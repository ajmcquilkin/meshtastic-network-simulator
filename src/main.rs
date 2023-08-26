use simulation::engine::Engine;

pub mod graph;
pub mod simulation;
pub mod utils;

#[tokio::main]
async fn main() -> Result<(), utils::GenericError> {
    let mut engine = Engine::new().expect("Failed to connect to Docker daemon");

    engine.initialize_nodes(4).await?;
    engine.wait_for_user();
    engine.cleanup().await?;

    Ok(())
}
