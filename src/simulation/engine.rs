use std::collections::HashMap;

use crate::graph::node::Node;
use crate::utils;
use bollard::container::{Config, RemoveContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::image::CreateImageOptions;
use bollard::service::PortBinding;
use bollard::Docker;
use futures_util::TryStreamExt;
use meshtastic::connections::helpers::generate_rand_id;
use meshtastic::connections::stream_api::StreamApi;
use meshtastic::protobufs;
use meshtastic::Message;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;

use super::rectangle::Rectangle;

pub const IMAGE_NAME: &str = "meshtastic/device-simulator";
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
    broadcast_send: broadcast::Sender<protobufs::MeshPacket>,
    broadcast_recv: broadcast::Receiver<protobufs::MeshPacket>,
}

// Private helper methods

impl Engine {
    fn initialize_nodes(num_nodes: usize, bounding_box: Rectangle) -> Vec<Node> {
        (0..num_nodes)
            .map(|id| Node::new(id as u32, bounding_box.get_random_contained_point()))
            .collect()
    }

    fn format_run_node_command(node: &Node) -> Vec<String> {
        vec![
            "sh".to_string(),
            "-c".to_string(),
            format!(
                "./meshtasticd_linux_amd64 -d /home/node{} -h {} -p {}",
                node.id, node.hw_id, node.tcp_port
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
            broadcast_send: send,
            broadcast_recv: recv,
        })
    }

    pub async fn create_host_container(
        &mut self,
        image_name: String,
    ) -> Result<(), utils::GenericError> {
        let create_image_options: CreateImageOptions<String> = CreateImageOptions {
            from_image: image_name.clone(),
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
            let exposed_port = format!("{}/tcp", node.tcp_port);
            exposed_ports.insert(exposed_port, empty.clone());

            port_bindings.insert(
                format!("{}/tcp", node.tcp_port).to_string(),
                Some(vec![PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(node.tcp_port.to_string()),
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

        println!("Host container id: {}", container_id);
        self.host_container_id = Some(container_id);
        Ok(())
    }

    async fn spawn_from_radio_listener(
        broadcast_send: broadcast::Sender<protobufs::MeshPacket>,
        decoded_listener: UnboundedReceiver<protobufs::FromRadio>,
        node_id: u32,
    ) {
        let mut decoded_listener = decoded_listener;

        while let Some(from_radio_packet) = decoded_listener.recv().await {
            let mesh_packet = match from_radio_packet.payload_variant {
                Some(protobufs::from_radio::PayloadVariant::Packet(p)) => p,
                _ => {
                    println!(
                        "[{}] Received non-mesh packet from radio, not forwarding: {:?}",
                        node_id, from_radio_packet
                    );
                    continue;
                }
            };

            println!(
                "[{}] Forwarding mesh packet with id {} from radio {} to pub_sub",
                node_id, mesh_packet.id, node_id
            );

            let forward_mesh_packet = match Engine::generate_forwarded_mesh_packet(mesh_packet) {
                Ok(packet) => packet,
                Err(e) => {
                    eprintln!(
                        "[{}] Error generating forwarded mesh packet: {}",
                        node_id, e
                    );
                    continue;
                }
            };

            match broadcast_send.send(forward_mesh_packet) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "[{}] Error sending message to pub sub channel: {}",
                        node_id, e
                    );
                }
            }
        }
    }

    async fn spawn_to_radio_listener(
        broadcast_recv: broadcast::Receiver<protobufs::MeshPacket>,
        to_radio_channel: UnboundedSender<Vec<u8>>,
        node_id: u32,
    ) {
        let mut broadcast_recv = broadcast_recv;

        while let Ok(mesh_packet) = broadcast_recv.recv().await {
            // Don't forward own messages to self

            if mesh_packet.from == node_id {
                continue;
            }

            println!(
                "[{}] Forwarding mesh_packet from pub_sub with id {} from radio {} to radio {}",
                node_id, mesh_packet.id, mesh_packet.from, node_id
            );

            let to_radio_packet = protobufs::ToRadio {
                payload_variant: Some(protobufs::to_radio::PayloadVariant::Packet(mesh_packet)),
            };

            let encoded_packet = to_radio_packet.encode_to_vec();

            if let Err(e) = to_radio_channel.send(encoded_packet) {
                eprintln!("[{}] Error sending message to radio: {}", node_id, e);
                continue;
            }
        }
    }

    pub async fn connect_to_nodes(&mut self) -> Result<(), utils::GenericError> {
        for node in self.nodes.iter_mut() {
            let tcp_stream = StreamApi::build_tcp_stream(node.get_full_tcp_address()).await?;
            let stream_api = StreamApi::new();

            let (decoded_listener, stream_api) = stream_api.connect(tcp_stream).await;

            let config_id = generate_rand_id();
            let stream_api = stream_api.configure(config_id).await?;

            let node_cancellation_token = CancellationToken::new();

            let cancellation_token = node_cancellation_token.clone();
            let broadcast_send = self.broadcast_send.clone();
            let node_id = node.hw_id;

            // Listen for packets from radio and forward them to pub/sub

            let decoded_listener_handle = tokio::spawn(async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        println!("Cancellation token cancelled, stopping from_radio worker");
                    }
                    _ = Engine::spawn_from_radio_listener(
                        broadcast_send,
                        decoded_listener,
                        node_id,
                    ) => {
                        println!("[{}] from_radio worker stopped", node_id);
                    }
                }
            });

            let cancellation_token = node_cancellation_token.clone();
            let broadcast_recv = self.broadcast_recv.resubscribe();
            let to_radio_channel = stream_api.get_write_input_sender()?;
            let node_id = node.hw_id;

            // Listen for packets from pub/sub or API and forward them to radio

            let message_relay_handle = tokio::spawn(async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        println!("Cancellation token cancelled, stopping message_relay worker");
                    }
                    _ = Engine::spawn_to_radio_listener(broadcast_recv, to_radio_channel, node_id) => {
                        println!("[{}] to_radio worker stopped", node_id);
                    }
                }
            });

            // Update node with handles

            node.stream_api = Some(stream_api);
            node.decoded_listener_handle = Some(decoded_listener_handle);
            node.message_relay_handle = Some(message_relay_handle);
            node.cancellation_token = Some(node_cancellation_token);
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

        println!("Removed host container: {}", container_id);

        Ok(())
    }

    pub fn get_nodes(&self) -> &Vec<Node> {
        &self.nodes
    }

    pub fn get_nodes_mut(&mut self) -> &mut Vec<Node> {
        &mut self.nodes
    }
}
