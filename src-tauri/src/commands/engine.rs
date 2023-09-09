use crate::{simulation::engine::Engine, state, IMAGE_NAME, IMAGE_TAG};

#[tauri::command]
// TODO replace this with better error handler flow (any...?)
pub async fn initialize_engine(
    engine_state: tauri::State<'_, state::EngineState>,
    num_nodes: u32,
) -> Result<(), String> {
    let mut engine = match Engine::new(num_nodes) {
        Ok(engine) => engine,
        Err(e) => {
            log::error!("Could not initialize engine: {}", e);
            return Err(e.to_string());
        }
    };

    if let Err(e) = engine
        .create_host_container(IMAGE_NAME.into(), IMAGE_TAG.into())
        .await
    {
        log::error!("Could not create host container: {}", e);
        return Err(e.to_string());
    }

    if let Err(e) = engine.connect_to_nodes().await {
        log::error!("Could not connect to node host containers: {}", e);
        return Err(e.to_string());
    }

    {
        let mut engine_state_guard = engine_state.inner.lock().await;
        *engine_state_guard = Some(engine);
    }

    Ok(())
}

#[tauri::command]
pub async fn destroy_engine(
    engine_state: tauri::State<'_, state::EngineState>,
) -> Result<(), String> {
    let mut engine_guard = engine_state.inner.lock().await;

    let engine = match engine_guard.as_mut() {
        Some(en) => en,
        None => {
            log::error!("Engine not initialized");
            return Err("Engine not initialized".into());
        }
    };

    if let Err(e) = engine.drop_node_connections().await {
        log::error!("Failed to drop node connections: {}", e);
        return Err(e.to_string());
    }

    if let Err(e) = engine.remove_host_container().await {
        log::error!("Failed ot remove engine host container: {}", e);
        return Err(e.to_string());
    }

    Ok(())
}
