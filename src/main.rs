use simulation::engine::Engine;

pub mod graph;
pub mod simulation;
pub mod utils;

pub const NUM_NODES: usize = 3;
const IMAGE_NAME: &str = "meshtastic/device-simulator";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let mut engine = Engine::new(NUM_NODES)?;
    engine.create_host_container(IMAGE_NAME.into()).await?;
    engine.connect_to_nodes().await?;

    println!("Waiting to remove container...");
    engine.wait_for_user();

    engine.drop_node_connections().await?;
    engine.remove_host_container().await?;

    Ok(())
}
