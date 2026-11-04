//! Event names and payloads sent to the frontend (`listen()` in `tauri-backend.ts`).

use serde::Serialize;

pub const SEGMENT: &str = "stt://segment";
pub const STATUS: &str = "stt://status";
pub const DOWNLOAD_PROGRESS: &str = "stt://download-progress";
pub const LEVEL: &str = "stt://level";
pub const ERROR: &str = "stt://error";

/// `idle | loading | listening | transcribing | error` – mirrored by `SttStatus` in TypeScript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SttStatus {
    Idle,
    Loading,
    Listening,
    Transcribing,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model_id: String,
    pub received: u64,
    pub total: u64,
}
