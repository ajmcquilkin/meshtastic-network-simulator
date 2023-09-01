use simulation::engine::Engine;

pub mod graph;
pub mod simulation;
pub mod utils;

pub const NUM_NODES: usize = 3;
const IMAGE_NAME: &str = "meshtastic/device-simulator";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let mut engine = Engine::new(NUM_NODES)?;

    match engine.create_host_container(IMAGE_NAME.into()).await {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to create host container: {}", e);
            return Err(e);
        }
    };

    match engine.connect_to_nodes().await {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to connect to nodes: {}", e);
            return Err(e);
        }
    };

    println!("Waiting to remove container...");
    engine.wait_for_user();

    match engine.drop_node_connections().await {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to drop node connections: {}", e);
            return Err(e);
        }
    }

    match engine.remove_host_container().await {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to remove host container: {}", e);
            return Err(e);
        }
    };

    Ok(())
}
