use std::collections::HashMap;

use bollard::container::{Config, RemoveContainerOptions};
use bollard::exec::{CreateExecOptions, CreateExecResults, StartExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::service::{ContainerCreateResponse, PortBinding};
use bollard::Docker;
use futures_util::TryStreamExt;

use crate::graph::node::Node;
use crate::utils;

use super::rectangle::Rectangle;

pub const IMAGE_NAME: &str = "meshtastic/device-simulator";
pub const SIMULATION_WIDTH: u32 = 100;
pub const SIMULATION_HEIGHT: u32 = 100;

#[derive(Debug)]
pub struct Engine {
    pub docker_client: Docker,
    pub primary_node_id: Option<String>,
    pub simulation_bounds: Rectangle,
    pub nodes: Vec<Node>,
}

impl Engine {
    pub fn new() -> Result<Self, utils::GenericError> {
        let docker_client = Docker::connect_with_socket_defaults()?;

        Ok(Engine {
            docker_client,
            primary_node_id: None,
            simulation_bounds: Rectangle {
                width: SIMULATION_WIDTH,
                height: SIMULATION_HEIGHT,
                ..Default::default() // Only care about width and height
            },
            nodes: vec![],
        })
    }

    async fn fetch_docker_image(
        docker_client: &Docker,
        image_name: &str,
    ) -> Result<(), utils::GenericError> {
        docker_client
            .create_image(
                Some(CreateImageOptions {
                    from_image: image_name,
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await?;

        Ok(())
    }

    async fn create_container_for_node(
        docker_client: &Docker,
        image_name: &str,
        node: &Node,
    ) -> Result<String, utils::GenericError> {
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{}/tcp", node.tcp_port).to_string(),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(node.tcp_port.to_string()),
            }]),
        );

        let container_config = Config {
            image: Some(image_name),
            tty: Some(true),
            host_config: Some(bollard::service::HostConfig {
                port_bindings: Some(port_bindings),
                ..Default::default()
            }),
            ..Default::default()
        };

        let ContainerCreateResponse { id, .. } = docker_client
            .create_container::<&str, &str>(None, container_config)
            .await?;
        println!("Created container with id \"{}\"", id);

        docker_client.start_container::<String>(&id, None).await?;
        println!("Started container with id \"{}\"", id);

        Ok(id)
    }

    async fn start_node_firmware(
        docker_client: &Docker,
        node: &Node,
        container_id: &String,
    ) -> Result<(), utils::GenericError> {
        let CreateExecResults { id: exec } = docker_client
            .create_exec(
                &container_id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(vec![
                        "/bin/sh",
                        "-c",
                        format!(
                        "./meshtasticd_linux_amd64 -d /home/node{} -h {} -p {} > /home/out_{}.log",
                        node.id, node.hw_id,node.tcp_port, node.id
                    )
                        .as_str(),
                    ]),
                    user: Some("root"),
                    ..Default::default()
                },
            )
            .await?;

        // Start exec and detach container
        let start_exec_result = docker_client
            .start_exec(
                &exec,
                Some(StartExecOptions {
                    detach: true,
                    ..Default::default()
                }),
            )
            .await?;

        // Ensure container started in detached mode
        if let StartExecResults::Detached = start_exec_result {
            println!("Started exec in detached mode: {:?}", start_exec_result);
        } else {
            // TODO make this a Result Err()
            eprintln!("Exec started in attached mode, but detached mode was requested");
        }

        Ok(())
    }

    pub async fn initialize_nodes(&mut self, num_nodes: usize) -> Result<(), utils::GenericError> {
        Engine::fetch_docker_image(&self.docker_client, IMAGE_NAME).await?;

        for i in 0..num_nodes {
            let mut node = Node::new(
                i as u32,
                self.simulation_bounds.get_random_contained_point(),
            );

            let container_id =
                match Engine::create_container_for_node(&self.docker_client, IMAGE_NAME, &node)
                    .await
                {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Failed to create container for node {}: {}", node.id, e);
                        continue;
                    }
                };

            match Engine::start_node_firmware(&self.docker_client, &node, &container_id).await {
                Ok(_) => println!("Started firmware for node {}", node.id),
                Err(e) => {
                    eprintln!("Failed to start firmware for node {}: {}", node.id, e);
                    continue;
                }
            }

            node.docker_container_id = Some(container_id);
            self.nodes.push(node);
        }

        println!("Nodes: {:?}", self.nodes);

        Ok(())
    }

    pub fn wait_for_user(&mut self) {
        // Wait for user to press "Enter" to continue
        println!("Press \"Enter\" to remove all containers");
        let mut line = String::new();
        let _input = std::io::stdin()
            .read_line(&mut line)
            .expect("Failed to read line");
    }

    async fn remove_node_container(
        docker_client: &Docker,
        node: &mut Node,
    ) -> Result<(), utils::GenericError> {
        let id = node.docker_container_id.take().ok_or(
            format!(
                "Node {} doesn't have associated Docker container id",
                node.id
            )
            .to_string(),
        )?;

        docker_client
            .remove_container(
                &id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| e.to_string())?;

        println!("Removed container with id \"{}\"", id);

        Ok(())
    }

    pub async fn cleanup(&mut self) -> Result<(), utils::GenericError> {
        for node in self.nodes.iter_mut() {
            match Engine::remove_node_container(&self.docker_client, node).await {
                Ok(_) => println!("Removed container for node {}", node.id),
                Err(e) => eprintln!("Failed to remove container for node {}: {}", node.id, e),
            }
        }

        Ok(())
    }
}
