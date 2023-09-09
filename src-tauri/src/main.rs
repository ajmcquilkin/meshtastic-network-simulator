#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;

use tauri::{async_runtime, Manager};

pub mod commands;
pub mod graph;
pub mod simulation;
pub mod state;
pub mod utils;

pub const NUM_NODES: u32 = 3;
const IMAGE_NAME: &str = "meshtastic/device-simulator";
const IMAGE_TAG: &str = "2.2.0.9f6584b"; // Latest

fn main() {
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
        .apply()
        .expect("Failed to start logger");

    log::info!("Logger started");

    log::info!("Application starting...");

    let initial_engine_state = state::EngineState {
        inner: Arc::new(async_runtime::Mutex::new(None)),
    };

    tauri::Builder::default()
        .setup(|app| {
            app.handle().manage(initial_engine_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::engine::initialize_engine,
            commands::engine::destroy_engine
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
