use std::collections::HashMap;

use crate::graph::node::Node;
use crate::utils;
use bollard::container::{Config, RemoveContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::image::CreateImageOptions;
use bollard::service::PortBinding;
use bollard::Docker;
use futures_util::TryStreamExt;
use meshtastic::api::StreamApi;
use meshtastic::protobufs;
use meshtastic::types::{EncodedToRadioPacket, EncodedToRadioPacketWithHeader};
use meshtastic::utils::{
    format_data_packet, generate_rand_id, stream::build_tcp_stream, strip_data_packet_header,
};
use meshtastic::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;

use super::rectangle::Rectangle;

pub const SIMULATION_WIDTH: u32 = 100;
pub const SIMULATION_HEIGHT: u32 = 100;
pub const HW_ID_OFFSET: u32 = 16;
pub const TCP_PORT_OFFSET: u32 = 4403;

pub struct Engine {
    docker_client: Docker,
    host_container_id: Option<String>,
    #[allow(dead_code)]
    simulation_bounds: Rectangle,
    nodes: Vec<Node>,
    pubsub_push_channel: broadcast::Sender<EncodedToRadioPacket>,
    pubsub_recv_channel: broadcast::Receiver<EncodedToRadioPacket>,
}

// Private helper methods

impl Engine {
    fn initialize_nodes(num_nodes: usize, bounding_box: Rectangle) -> Vec<Node> {
        (0..num_nodes)
            .map(|id| {
                let id = id as u32;

                let docker_tcp_port: u32 = 2 * id + TCP_PORT_OFFSET;
                let client_tcp_port: u32 = 2 * id + TCP_PORT_OFFSET + 1;

                Node::new(
                    id,
                    id + HW_ID_OFFSET,
                    docker_tcp_port,
                    client_tcp_port,
                    bounding_box.get_random_contained_point(),
                )
            })
            .collect()
    }

    fn format_run_node_command(node: &Node) -> Vec<String> {
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "./meshtasticd_linux_amd64 -d /home/node{} -h {} -p {}",
                node.id, node.hw_id, node.docker_tcp_port
            )
            .to_string(),
        ]
    }

    fn generate_forwarded_mesh_packet(
        incoming_packet: protobufs::MeshPacket,
    ) -> Result<protobufs::MeshPacket, String> {
        let mut outgoing_packet = incoming_packet;

        match outgoing_packet.payload_variant {
            Some(protobufs::mesh_packet::PayloadVariant::Decoded(ref mut data)) => {
                data.portnum = protobufs::PortNum::SimulatorApp.into();
            }
            _ => {
                return Err("Received invalid mesh packet, skipping".to_string());
            }
        }

        Ok(outgoing_packet)
    }
}

// Public engine API

impl Engine {
    pub fn new(num_nodes: usize) -> Result<Self, utils::GenericError> {
        let docker_client = Docker::connect_with_socket_defaults()?;
        let simulation_bounds = Rectangle {
            width: SIMULATION_WIDTH,
            height: SIMULATION_HEIGHT,
            ..Default::default() // Only care about width and height
        };

        let nodes = Engine::initialize_nodes(num_nodes, simulation_bounds.clone());
        let (send, recv) = broadcast::channel(64);

        Ok(Engine {
            docker_client,
            host_container_id: None,
            nodes,
            simulation_bounds,
            pubsub_push_channel: send,
            pubsub_recv_channel: recv,
        })
    }

    pub async fn create_host_container(
        &mut self,
        image_name: String,
        tag: String,
    ) -> Result<(), utils::GenericError> {
        let create_image_options: CreateImageOptions<String> = CreateImageOptions {
            from_image: image_name.clone(),
            tag,
            ..Default::default()
        };

        self.docker_client
            .create_image(Some(create_image_options), None, None)
            .try_collect::<Vec<_>>()
            .await?;

        let empty = HashMap::<(), ()>::new();
        let mut exposed_ports = HashMap::<String, _>::new();
        let mut port_bindings = HashMap::new();

        for node in self.nodes.iter() {
            let exposed_port = format!("{}/tcp", node.docker_tcp_port);
            exposed_ports.insert(exposed_port, empty.clone());

            port_bindings.insert(
                format!("{}/tcp", node.docker_tcp_port).to_string(),
                Some(vec![PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(node.docker_tcp_port.to_string()),
                }]),
            );
        }

        let device_config: Config<String> = Config {
            image: Some(image_name),
            tty: Some(true),
            exposed_ports: Some(exposed_ports),
            host_config: Some(bollard::service::HostConfig {
                port_bindings: Some(port_bindings),
                auto_remove: Some(true),
                ..Default::default()
            }),
            cmd: Some(Self::format_run_node_command(
                self.nodes.get(0).ok_or("Could not find first node")?,
            )),
            user: Some("mesh".to_string()),
            ..Default::default()
        };

        let container_id = self
            .docker_client
            .create_container::<String, String>(None, device_config)
            .await?
            .id;

        self.docker_client
            .start_container::<String>(&container_id, None)
            .await?;

        for node in self.nodes.iter().filter(|n| n.id != 0) {
            let exec = self
                .docker_client
                .create_exec(
                    &container_id,
                    CreateExecOptions {
                        cmd: Some(Self::format_run_node_command(node)),
                        user: Some("mesh".to_string()),
                        ..Default::default()
                    },
                )
                .await?
                .id;

            let start_exec_options = StartExecOptions {
                detach: true,
                ..Default::default()
            };

            self.docker_client
                .start_exec(&exec, Some(start_exec_options))
                .await?;
        }

        log::info!("Host container id: {}", container_id);
        self.host_container_id = Some(container_id);
        Ok(())
    }

    async fn spawn_client_forward_worker(
        to_client_recv_channel: broadcast::Receiver<EncodedToRadioPacketWithHeader>,
        client_listener: tokio::net::TcpListener,
        stream_api_to_radio_sender: UnboundedSender<EncodedToRadioPacket>,
    ) {
        let to_client_recv_channel = to_client_recv_channel;
        let stream_api_to_radio_sender = stream_api_to_radio_sender;

        loop {
            let (socket, addr) = client_listener.accept().await.expect("ERROR");
            log::info!("Client connected from {}", addr);

            let (mut from_client_half, mut to_client_half) = tokio::io::split(socket);

            // Forward data from client to radio

            let stream_api_to_radio_sender = stream_api_to_radio_sender.clone();

            let from_client_handle = tokio::spawn(async move {
                loop {
                    let mut buf = [0u8; 1024];
                    let read_bytes = from_client_half.read(&mut buf).await.expect("ERROR");

                    if read_bytes == 0 {
                        continue;
                    }

                    let data: EncodedToRadioPacketWithHeader = buf[..read_bytes].to_vec().into();

                    let stripped_data = match strip_data_packet_header(data) {
                        Ok(data) => data,
                        Err(e) => {
                            log::error!("Error stripping header from data: {}", e);
                            continue;
                        }
                    };

                    log::debug!("Received data from client: {:?}", stripped_data);

                    match &stream_api_to_radio_sender.send(stripped_data) {
                        Ok(_) => {
                            log::debug!("Successfully sent data to from_client channel");
                        }
                        Err(e) => {
                            log::error!("Error sending data to from_client channel: {}", e);
                        }
                    };
                }
            });

            // Forward data from radio to client

            let mut to_client_recv_channel = to_client_recv_channel.resubscribe();

            let to_client_handle = tokio::spawn(async move {
                while let Ok(data) = to_client_recv_channel.recv().await {
                    log::trace!("Sending data to client: {:?}", data);
                    let bytes_sent = to_client_half.write(data.data()).await.expect("ERROR");
                    log::trace!("Sent {} bytes to client", bytes_sent);
                }
            });

            // Wait for handles to join before listening for next connection

            match from_client_handle.await {
                Ok(_) => {
                    log::debug!("from_client_handle joined");
                }
                Err(e) => {
                    log::error!("from_client_handle failed to join: {}", e);
                }
            }

            match to_client_handle.await {
                Ok(_) => {
                    log::debug!("to_client_handle joined");
                }
                Err(e) => {
                    log::error!("to_client_handle failed to join: {}", e);
                }
            }
        }
    }

    /// Listens for packets coming from a radio connection
    /// and forwards all mesh packets to pub/sub.
    async fn spawn_from_radio_worker(
        pubsub_push_channel: broadcast::Sender<EncodedToRadioPacket>,
        decoded_listener: UnboundedReceiver<protobufs::FromRadio>,
        node_id: u32,
        to_client_send_channel: broadcast::Sender<EncodedToRadioPacketWithHeader>,
    ) {
        let mut decoded_listener = decoded_listener;

        while let Some(from_radio_packet) = decoded_listener.recv().await {
            // Immediately forward all packets to client connection

            log::info!(
                "[{}] Forwarding packet from radio {} to client: {:?}",
                node_id,
                node_id,
                from_radio_packet
            );

            let encoded_packet: EncodedToRadioPacket =
                from_radio_packet.clone().encode_to_vec().into();
            let formatted_packet = format_data_packet(encoded_packet);

            match to_client_send_channel.send(formatted_packet) {
                Ok(_) => {}
                Err(e) => {
                    log::error!(
                        "[{}] Error sending message to client channel: {}",
                        node_id,
                        e
                    );
                }
            }

            // Forward packet to other radios if it is a mesh packet

            let mesh_packet = match from_radio_packet.payload_variant {
                Some(protobufs::from_radio::PayloadVariant::Packet(p)) => p,
                _ => {
                    log::debug!(
                        "[{}] Received non-mesh packet from radio, not forwarding...",
                        node_id,
                    );
                    continue;
                }
            };

            log::info!(
                "[{}] Forwarding mesh packet with id {} from radio {} to pub_sub",
                node_id,
                mesh_packet.id,
                node_id
            );

            let forward_mesh_packet =
                match Engine::generate_forwarded_mesh_packet(mesh_packet.clone()) {
                    Ok(packet) => packet,
                    Err(e) => {
                        log::error!(
                            "[{}] Error generating forwarded mesh packet: {}",
                            node_id,
                            e
                        );
                        continue;
                    }
                };

            let to_radio_packet = protobufs::ToRadio {
                payload_variant: Some(protobufs::to_radio::PayloadVariant::Packet(
                    forward_mesh_packet,
                )),
            };

            let encoded_packet: EncodedToRadioPacket = to_radio_packet.encode_to_vec().into();

            match pubsub_push_channel.send(encoded_packet) {
                Ok(_) => {}
                Err(e) => {
                    log::error!(
                        "[{}] Error sending message to pub sub channel: {}",
                        node_id,
                        e
                    );
                }
            }
        }
    }

    /// Listens for packets coming from pub/sub and forwards them to
    /// the connected radio.
    async fn spawn_to_radio_worker(
        pubsub_recv_channel: broadcast::Receiver<EncodedToRadioPacket>,
        stream_api_to_radio_sender: UnboundedSender<EncodedToRadioPacket>,
        node_id: u32,
    ) {
        let mut pubsub_recv_channel = pubsub_recv_channel;

        while let Ok(encoded_packet) = pubsub_recv_channel.recv().await {
            if let Err(e) = stream_api_to_radio_sender.send(encoded_packet) {
                eprintln!("[{}] Error sending message to radio: {}", node_id, e);
                continue;
            }
        }
    }

    pub async fn connect_to_nodes(&mut self) -> Result<(), utils::GenericError> {
        for node in self.nodes.iter_mut() {
            // Configure node stream_api connection

            let tcp_stream = build_tcp_stream(node.get_full_docker_tcp_address()).await?;
            let stream_api = StreamApi::new();

            let (decoded_listener, stream_api) = stream_api.connect(tcp_stream).await;

            let config_id = generate_rand_id();
            let stream_api = stream_api.configure(config_id).await?;

            let node_cancellation_token = CancellationToken::new();

            // Configure client/radio forwarding

            log::info!(
                "Configuring node {} as client interface on port {}",
                node.hw_id,
                node.client_tcp_port
            );

            let client_listener =
                tokio::net::TcpListener::bind(node.get_full_client_tcp_address()).await?;

            // Channel to allow the connected radio to forward data to the client
            let (to_client_send_channel, to_client_recv_channel) =
                broadcast::channel::<EncodedToRadioPacketWithHeader>(64);

            // Listen for connections from a client and proxy data to/from the radio

            let cancellation_token = node_cancellation_token.clone();
            let stream_api_to_radio_sender = stream_api.write_input_sender();
            let node_id = node.hw_id;

            let client_forwarding_handle = tokio::spawn(async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        log::debug!("Cancellation token cancelled, stopping client_forwarding worker");
                    }
                    _ = Engine::spawn_client_forward_worker(
                        to_client_recv_channel,
                        client_listener,
                        stream_api_to_radio_sender
                    ) => {
                        log::debug!("[{}] client_forwarding worker stopped", node_id);
                    }
                }
            });

            // Listen for packets from radio and forward them to pub/sub

            let cancellation_token = node_cancellation_token.clone();
            let pubsub_push_channel = self.pubsub_push_channel.clone();
            let node_id = node.hw_id;
            let to_client_send_channel = to_client_send_channel.clone();

            let decoded_listener_handle = tokio::spawn(async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        log::debug!("Cancellation token cancelled, stopping from_radio worker");
                    }
                    _ = Engine::spawn_from_radio_worker(
                        pubsub_push_channel,
                        decoded_listener,
                        node_id,
                        to_client_send_channel
                    ) => {
                        log::debug!("[{}] from_radio worker stopped", node_id);
                    }
                }
            });

            let cancellation_token = node_cancellation_token.clone();
            let pubsub_recv_channel = self.pubsub_recv_channel.resubscribe();
            let stream_api_to_radio_sender = stream_api.write_input_sender();
            let node_id = node.hw_id;

            // Listen for packets from pub/sub or API and forward them to radio

            let message_relay_handle = tokio::spawn(async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        log::debug!("Cancellation token cancelled, stopping message_relay worker");
                    }
                    _ = Engine::spawn_to_radio_worker(pubsub_recv_channel, stream_api_to_radio_sender, node_id) => {
                        log::debug!("[{}] to_radio worker stopped", node_id);
                    }
                }
            });

            // Update node with handles

            node.stream_api = Some(stream_api);
            node.cancellation_token = Some(node_cancellation_token);

            node.decoded_listener_handle = Some(decoded_listener_handle);
            node.message_relay_handle = Some(message_relay_handle);
            node.client_forwarding_handle = Some(client_forwarding_handle);
        }

        Ok(())
    }

    /// Wait for user to press "Enter" to continue
    pub fn wait_for_user(&mut self) {
        println!("Press \"Enter\" to continue");
        let mut line = String::new();
        let _input = std::io::stdin()
            .read_line(&mut line)
            .expect("Failed to read line");
    }

    pub async fn drop_node_connections(&mut self) -> Result<(), utils::GenericError> {
        for node in self.nodes.iter_mut() {
            node.disconnect().await?;
        }

        Ok(())
    }

    pub async fn remove_host_container(&mut self) -> Result<(), utils::GenericError> {
        let container_id = self.host_container_id.take().ok_or(
            "Engine does not have a host container id, cannot remove container".to_string(),
        )?;

        self.docker_client
            .remove_container(
                &container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;

        log::info!("Removed host container: {}", container_id);

        Ok(())
    }
}
