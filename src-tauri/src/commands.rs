//! Thin command layer. Every command is `async`; blocking work runs in `spawn_blocking`.

use crate::AppState;
use crate::devices::{self, InputDevice};
use crate::error::{AppError, AppResult};
use crate::events::{self, DownloadProgress, SttStatus};
use serde::Serialize;
use stt_core::{MODELS, ModelId, ModelInfo};
use tauri::{AppHandle, Emitter, Manager, State};

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::Internal(e.to_string()))?
}

fn parse_model(id: &str) -> AppResult<ModelId> {
    ModelId::parse(id).ok_or_else(|| stt_core::SttError::UnknownModel(id.to_owned()).into())
}

/// Catalogue entry plus local state (`ModelInfo` in TypeScript).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    #[serde(flatten)]
    info: ModelInfo,
    downloaded: bool,
    loaded: bool,
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> AppResult<Vec<ModelEntry>> {
    let loaded = state.engine.loaded_model();
    Ok(MODELS
        .iter()
        .map(|m| ModelEntry {
            info: *m,
            downloaded: state.models.is_downloaded(m.id),
            loaded: loaded == Some(m.id),
        })
        .collect())
}

#[tauri::command]
pub async fn download_model(app: AppHandle, model_id: String) -> AppResult<()> {
    let id = parse_model(&model_id)?;
    let state = app.state::<AppState>();
    if state.models.is_downloaded(id) {
        return Ok(());
    }
    let _ = app.emit(events::STATUS, SttStatus::Loading);
    let emitter = app.clone();
    state
        .models
        .download(id, move |received, total| {
            let p = DownloadProgress { model_id: model_id.clone(), received, total };
            let _ = emitter.emit(events::DOWNLOAD_PROGRESS, p);
        })
        .await
}

#[tauri::command]
pub async fn load_model(app: AppHandle, model_id: String) -> AppResult<()> {
    let id = parse_model(&model_id)?;
    if app.state::<AppState>().engine.loaded_model() == Some(id) {
        return Ok(());
    }
    let _ = app.emit(events::STATUS, SttStatus::Loading);
    blocking(move || {
        let state = app.state::<AppState>();
        let bytes = state.models.load(id)?;
        state.engine.load(id, bytes)
        // no "idle" here: the frontend goes on with start_listening right away
    })
    .await
}

#[tauri::command]
pub async fn start_listening(app: AppHandle, language: String) -> AppResult<()> {
    let language = (language != "auto").then_some(language);
    blocking(move || app.state::<AppState>().engine.start(app.clone(), language)).await
}

#[tauri::command]
pub async fn stop_listening(app: AppHandle) -> AppResult<()> {
    blocking(move || app.state::<AppState>().engine.stop()).await
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
    state.engine.set_device(device_id)
}

#[cfg(test)]
mod tests {
    /// Commands must never block the async runtime: every `#[tauri::command]` is `async`.
    #[test]
    fn command_rules() {
        let src = include_str!("commands.rs");
        let mut lines = src.lines();
        while let Some(line) = lines.next() {
            if line.trim() == "#[tauri::command]" {
                let next = lines.next().unwrap_or_default();
                assert!(next.contains("pub async fn"), "not async: {next}");
            }
        }
    }
}
