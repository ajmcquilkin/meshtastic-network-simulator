use std::time::Duration;

use connections::stream_api::StreamApi;
use graph::node::Node;
use simulation::engine::Engine;
use tokio::{spawn, task::JoinHandle};

pub mod connections;
pub mod graph;
pub mod protobufs;
pub mod simulation;
pub mod utils;

pub const NUM_NODES: usize = 1;

#[tokio::main]
async fn main() -> Result<(), utils::GenericError> {
    let mut engine = Engine::new().expect("Failed to connect to Docker daemon");

    engine.initialize_nodes(NUM_NODES).await?;

    // Allow containers to initialize
    tokio::time::sleep(Duration::from_secs(10)).await;

    let mut node_handles = vec![];

    let _result: Result<(), String> = {
        for node in engine.get_nodes().iter() {
            let handle = connect_to_node(node).await?;
            node_handles.push(handle);
        }

        engine.wait_for_user();

        for handle in node_handles {
            handle.abort();
        }

        Ok(())
    };

    engine.cleanup().await?;

    Ok(())
}

async fn connect_to_node(node: &Node) -> Result<JoinHandle<()>, String> {
    let address = Node::get_full_tcp_address(node);
    let stream = StreamApi::build_tcp_stream(address.clone()).await?;

    let mut stream_api = StreamApi::new();
    let decoded_listener = stream_api.connect(stream).await;

    let id = node.id.clone();
    let handle = spawn(async move {
        let mut listener = decoded_listener;

        while let Some(message) = listener.recv().await {
            println!("[{}]: {:?}", id, message);
        }
    });

    stream_api.configure(node.id).await?;
    Ok(handle)
}
