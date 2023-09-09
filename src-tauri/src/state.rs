use std::sync::Arc;

use tauri::async_runtime;

use crate::simulation::engine::Engine;

pub type EngineStateInner = Arc<async_runtime::Mutex<Option<Engine>>>;

#[derive(Debug)]
pub struct EngineState {
    pub inner: EngineStateInner,
}
