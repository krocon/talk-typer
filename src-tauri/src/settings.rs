//! Persisted user settings (`<app data>/settings.json` via tauri-plugin-store, Rust-side only).
//! Values are validated: unknown models or malformed entries fall back to the defaults.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use stt_core::ModelId;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const FILE: &str = "settings.json";
const KEY: &str = "settings";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub model_id: Option<String>,
    pub language: Option<String>,
    pub device_id: Option<String>,
}

impl Settings {
    /// Drops values that cannot be valid.
    pub fn sanitized(self) -> Self {
        let short = |s: Option<String>, max: usize| s.filter(|v| !v.is_empty() && v.len() <= max);
        Self {
            model_id: self.model_id.filter(|m| ModelId::parse(m).is_some()),
            language: short(self.language, 8).filter(|l| l.chars().all(|c| c.is_ascii_lowercase())),
            device_id: short(self.device_id, 512),
        }
    }
}

fn store_err(e: impl std::fmt::Display) -> AppError {
    AppError::Io(e.to_string())
}

pub fn load(app: &AppHandle) -> AppResult<Settings> {
    let store = app.store(FILE).map_err(store_err)?;
    Ok(store
        .get(KEY)
        .and_then(|v| serde_json::from_value::<Settings>(v).ok())
        .unwrap_or_default()
        .sanitized())
}

pub fn save(app: &AppHandle, settings: Settings) -> AppResult<()> {
    let store = app.store(FILE).map_err(store_err)?;
    let value = serde_json::to_value(settings.sanitized()).map_err(store_err)?;
    store.set(KEY, value);
    store.save().map_err(store_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_values_are_dropped() {
        let s = Settings {
            model_id: Some("gpt-9".into()),
            language: Some("DE<script>".into()),
            device_id: Some(String::new()),
        }
        .sanitized();
        assert_eq!(s, Settings::default());
        let ok = Settings {
            model_id: Some("large-v3-turbo".into()),
            language: Some("de".into()),
            device_id: Some("CoreAudio:42".into()),
        };
        assert_eq!(ok.clone().sanitized(), ok);
    }
}
