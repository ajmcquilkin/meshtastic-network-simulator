use std::time::Duration;

use simulation::engine::Engine;

pub mod graph;
pub mod simulation;
pub mod utils;

pub const NUM_NODES: usize = 1;
const IMAGE_NAME: &str = "meshtastic/device-simulator";
const IMAGE_TAG: &str = "2.2.0.9f6584b"; // Latest

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339(std::time::SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        // .chain(fern::log_file("output.log")?)
        .apply()?;

    let mut engine = Engine::new(NUM_NODES)?;

    match engine
        .create_host_container(IMAGE_NAME.into(), IMAGE_TAG.into())
        .await
    {
        Ok(_) => (),
        Err(e) => {
            log::error!("Failed to create host container: {}", e);
            return Err(e);
        }
    };

    // Let the nodes start up before configuration
    log::info!("Waiting for nodes to start up...");
    tokio::time::sleep(Duration::from_secs(4)).await;
    log::info!("Finished waiting for nodes to start up");

    match engine.connect_to_nodes().await {
        Ok(_) => (),
        Err(e) => {
            log::error!("Failed to connect to nodes: {}", e);
            return Err(e);
        }
    };

    println!("Waiting to remove container...");
    engine.wait_for_user();

    match engine.drop_node_connections().await {
        Ok(_) => (),
        Err(e) => {
            log::error!("Failed to drop node connections: {}", e);
            return Err(e);
        }
    }

    match engine.remove_host_container().await {
        Ok(_) => (),
        Err(e) => {
            log::error!("Failed to remove host container: {}", e);
            return Err(e);
        }
    };

    Ok(())
}
