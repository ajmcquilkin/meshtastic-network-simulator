//! This example will run a non-interactive command inside the container using `docker exec`

use std::collections::HashMap;

use bollard::container::{Config, RemoveContainerOptions};
use bollard::Docker;

use bollard::exec::{CreateExecOptions, StartExecOptions};
use bollard::image::CreateImageOptions;
use bollard::service::PortBinding;
// use futures_util::stream::StreamExt;
use futures_util::TryStreamExt;

pub const NUM_NODES: usize = 3;
const IMAGE: &str = "meshtastic/device-simulator";

fn format_run_node_command(id: usize) -> Vec<String> {
    vec![
        "sh".to_string(),
        "-c".to_string(),
        format!(
            "./meshtasticd_linux_amd64 -d /home/node{} -h {} -p {}",
            0 + id,
            16 + id,
            4403 + id
        )
        .to_string(),
    ]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let docker = Docker::connect_with_socket_defaults().unwrap();

    let create_image_options: CreateImageOptions<String> = CreateImageOptions {
        from_image: IMAGE.to_string(),
        ..Default::default()
    };

    docker
        .create_image(Some(create_image_options), None, None)
        .try_collect::<Vec<_>>()
        .await?;

    let empty = HashMap::<(), ()>::new();
    let mut exposed_ports = HashMap::<String, _>::new();
    let mut port_bindings = HashMap::new();

    for id in 0..NUM_NODES {
        let exposed_port = format!("{}/tcp", id + 4403);
        exposed_ports.insert(exposed_port, empty.clone());

        port_bindings.insert(
            format!("{}/tcp", 4403 + id).to_string(),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some((4403 + id).to_string()),
            }]),
        );
    }

    let device_config: Config<String> = Config {
        image: Some(IMAGE.to_string()),
        tty: Some(true),
        exposed_ports: Some(exposed_ports),
        host_config: Some(bollard::service::HostConfig {
            port_bindings: Some(port_bindings),
            auto_remove: Some(true),
            ..Default::default()
        }),
        cmd: Some(format_run_node_command(0)),
        user: Some("mesh".to_string()),
        ..Default::default()
    };

    let container_id = docker
        .create_container::<String, String>(None, device_config)
        .await?
        .id;

    docker
        .start_container::<String>(&container_id, None)
        .await?;

    for id in 1..NUM_NODES {
        let exec = docker
            .create_exec(
                &container_id,
                CreateExecOptions {
                    cmd: Some(format_run_node_command(id)),
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

        docker.start_exec(&exec, Some(start_exec_options)).await?;
    }

    println!("Press \"Enter\" to remove container");
    let mut line = String::new();
    let _input = std::io::stdin()
        .read_line(&mut line)
        .expect("Failed to read line");

    docker
        .remove_container(
            &container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await?;

    Ok(())
}

// use std::time::Duration;

// use graph::node::Node;
// use meshtastic::connections::stream_api::{state::Configured, StreamApi};
// use simulation::engine::Engine;
// use tokio::{spawn, task::JoinHandle};

// pub mod graph;
// pub mod simulation;
// pub mod utils;

// pub const NUM_NODES: usize = 1;

// #[tokio::main]
// async fn main() -> Result<(), utils::GenericError> {
//     let mut engine = Engine::new().expect("Failed to connect to Docker daemon");

//     engine.initialize_nodes(NUM_NODES).await?;

//     // Allow containers to initialize
//     tokio::time::sleep(Duration::from_secs(10)).await;

//     let mut node_handles = vec![];

//     let _result: Result<(), String> = {
//         for node in engine.get_nodes().iter() {
//             let (handle, _) = connect_to_node(node).await?;
//             node_handles.push(handle);
//         }

//         engine.wait_for_user();

//         for handle in node_handles {
//             handle.abort();
//         }

//         Ok(())
//     };

//     engine.cleanup().await?;

//     Ok(())
// }

// async fn connect_to_node(node: &Node) -> Result<(JoinHandle<()>, StreamApi<Configured>), String> {
//     let address = Node::get_full_tcp_address(node);
//     let stream = StreamApi::build_tcp_stream(address.clone()).await?;

//     let stream_api = StreamApi::new();
//     let (decoded_listener, stream_api) = stream_api.connect(stream).await;

//     let id = node.id.clone();
//     let handle = spawn(async move {
//         let mut listener = decoded_listener;

//         while let Some(message) = listener.recv().await {
//             println!("[{}]: {:?}", id, message);
//         }
//     });

//     let stream_api = stream_api.configure(node.id).await?;
//     Ok((handle, stream_api))
// }
