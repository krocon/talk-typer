//! Transcription output.

use serde::{Deserialize, Serialize};

/// One piece of recognised text. Times are relative to the start of the recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    /// `false` for a preliminary result of a chunk that is still growing.
    pub is_final: bool,
}
