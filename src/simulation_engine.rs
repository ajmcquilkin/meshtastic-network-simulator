use std::collections::HashMap;
use std::time::Duration;

use bollard::container::{Config, RemoveContainerOptions};
use bollard::Docker;

use bollard::exec::{CreateExecOptions, CreateExecResults, StartExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::service::{ContainerCreateResponse, PortBinding};
use futures_util::TryStreamExt;

pub const IMAGE_NAME: &str = "meshtastic/device-simulator";

#[derive(Debug)]
pub struct SimulationEngine {
    pub docker_client: Docker,
    pub primary_node_id: Option<String>,
}

impl SimulationEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + 'static>> {
        let docker_client = Docker::connect_with_socket_defaults()?;

        Ok(SimulationEngine {
            docker_client,
            primary_node_id: None,
        })
    }

    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + 'static>> {
        // Pull meshtastic simulator image from docker hub
        self.docker_client
            .create_image(
                Some(CreateImageOptions {
                    from_image: IMAGE_NAME,
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await?;

        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "4403/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some("4403".to_string()),
            }]),
        );

        let alpine_config = Config {
            image: Some(IMAGE_NAME),
            tty: Some(true),
            host_config: Some(bollard::service::HostConfig {
                port_bindings: Some(port_bindings),
                ..Default::default()
            }),
            ..Default::default()
        };

        let ContainerCreateResponse { id, .. } = self
            .docker_client
            .create_container::<&str, &str>(None, alpine_config)
            .await?;

        self.primary_node_id = Some(id.clone());
        println!("Created container with id \"{}\"", id);

        self.docker_client
            .start_container::<String>(&id, None)
            .await?;

        let CreateExecResults { id: exec } = self
            .docker_client
            .create_exec(
                &id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(vec![
                        "/bin/sh",
                        "-c",
                        format!(
                        "./meshtasticd_linux_amd64 -d /home/node{} -h {} -p {} > /home/out_{}.log",
                        1, 17, 4403, 1
                    )
                        .as_str(),
                    ]),
                    user: Some("root"),
                    ..Default::default()
                },
            )
            .await?;

        // Start exec and detach container
        let start_exec_result = self
            .docker_client
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
            panic!("Exec started in attached mode, but detached mode was requested");
        }

        Ok(())
    }

    pub async fn run(&mut self) {
        // Wait an arbitrary amount of time to see container in Docker Desktop
        println!("Pausing to check container logs in Docker Desktop");
        tokio::time::sleep(Duration::from_secs(20)).await;
    }

    pub async fn cleanup(&mut self) -> Result<(), String> {
        let id = self
            .primary_node_id
            .take()
            .ok_or("No set primary node id".to_string())?;

        self.docker_client
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
}
