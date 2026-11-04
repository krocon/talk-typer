//! Input devices via cpal.

use crate::error::{AppError, AppResult};
use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDevice {
    /// Stable id (cpal `DeviceId`), used by `set_input_device`.
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn list_input_devices() -> AppResult<Vec<InputDevice>> {
    let host = cpal::default_host();
    let default_id = host.default_input_device().and_then(|d| d.id().ok()).map(|id| id.to_string());
    let devices = host.input_devices().map_err(|e| AppError::AudioDevice(e.to_string()))?;
    Ok(devices
        .filter_map(|d| {
            let id = d.id().ok()?.to_string();
            let name =
                d.description().map(|desc| desc.name().to_owned()).unwrap_or_else(|_| id.clone());
            let is_default = default_id.as_deref() == Some(id.as_str());
            Some(InputDevice { id, name, is_default })
        })
        .collect())
}
