//! Thin command layer. Every command is `async`; blocking work runs in `spawn_blocking`.

use crate::AppState;
use crate::devices::{self, InputDevice};
use crate::error::{AppError, AppResult};
use stt_core::{MODELS, ModelInfo};
use tauri::State;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn list_models() -> AppResult<Vec<ModelInfo>> {
    Ok(MODELS.to_vec())
}

#[tauri::command]
pub async fn download_model(model_id: String) -> AppResult<()> {
    let _ = model_id;
    Err(AppError::NotImplemented("download_model (phase 3)"))
}

#[tauri::command]
pub async fn load_model(model_id: String) -> AppResult<()> {
    let _ = model_id;
    Err(AppError::NotImplemented("load_model (phase 3)"))
}

#[tauri::command]
pub async fn start_listening(language: String) -> AppResult<()> {
    let _ = language;
    Err(AppError::NotImplemented("start_listening (phase 3)"))
}

#[tauri::command]
pub async fn stop_listening() -> AppResult<()> {
    Ok(())
}

#[tauri::command]
pub async fn list_input_devices() -> AppResult<Vec<InputDevice>> {
    blocking(devices::list_input_devices).await
}

#[tauri::command]
pub async fn set_input_device(
    state: State<'_, AppState>,
    device_id: Option<String>,
) -> AppResult<()> {
    let mut selected = state.input_device.lock().map_err(|e| AppError::Internal(e.to_string()))?;
    *selected = device_id;
    Ok(())
}
