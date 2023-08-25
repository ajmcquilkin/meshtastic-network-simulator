use simulation_engine::SimulationEngine;

pub mod graph;
pub mod simulation_engine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let mut engine = SimulationEngine::new().expect("Failed to connect to Docker daemon");

    engine.init().await?;
    engine.run().await;
    engine.cleanup().await?;

    Ok(())
}
