use std::collections::HashMap;

use bollard::container::{Config, RemoveContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::image::CreateImageOptions;
use bollard::service::PortBinding;
use bollard::Docker;
use futures_util::TryStreamExt;
use meshtastic::connections::helpers::generate_rand_id;
use meshtastic::connections::stream_api::StreamApi;

use crate::graph::node::Node;
use crate::utils;

use super::rectangle::Rectangle;

pub const IMAGE_NAME: &str = "meshtastic/device-simulator";
pub const SIMULATION_WIDTH: u32 = 100;
pub const SIMULATION_HEIGHT: u32 = 100;
pub const HW_ID_OFFSET: u32 = 16;
pub const TCP_PORT_OFFSET: u32 = 4403;

#[derive(Debug)]
pub struct Engine {
    docker_client: Docker,
    host_container_id: Option<String>,
    #[allow(dead_code)]
    simulation_bounds: Rectangle,
    nodes: Vec<Node>,
}

// Private helper methods

impl Engine {
    fn initialize_nodes(num_nodes: usize, bounding_box: Rectangle) -> Vec<Node> {
        let nodes = (0..num_nodes)
            .map(|id| Node::new(id as u32, bounding_box.get_random_contained_point()))
            .collect();

        println!("Initialized nodes: {:?}", nodes);

        nodes
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

        Ok(Engine {
            docker_client,
            host_container_id: None,
            nodes,
            simulation_bounds,
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

    pub async fn connect_to_nodes(&mut self) -> Result<(), utils::GenericError> {
        for node in self.nodes.iter_mut() {
            let tcp_stream = StreamApi::build_tcp_stream(node.get_full_tcp_address()).await?;
            let stream_api = StreamApi::new();

            let (decoded_listener, stream_api) = stream_api.connect(tcp_stream).await;

            let config_id = generate_rand_id();
            let stream_api = stream_api.configure(config_id).await?;

            let id = node.id.clone();
            let handle = tokio::spawn(async move {
                let mut decoded_listener = decoded_listener;
                while let Some(message) = decoded_listener.recv().await {
                    println!("[{}]: {:?}", id, message);
                }
                println!("Node {} listener stopped", id);
            });

            node.stream_api = Some(stream_api);
            node.decoded_listener_handle = Some(handle);
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
