//! Errors sent to the frontend as `{ code, message }`. The UI maps `code` to its own texts and
//! never shows `message` directly.

use serde::Serialize;
use stt_core::SttError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Stt(#[from] SttError),
    #[error("audio device: {0}")]
    AudioDevice(String),
    #[error("microphone access denied")]
    MicrophoneDenied,
    #[error("no input device")]
    NoInputDevice,
    #[error("download failed: {0}")]
    Download(String),
    #[error("checksum mismatch: {0}")]
    ChecksumMismatch(String),
    #[error("model not downloaded: {0}")]
    ModelMissing(String),
    #[error("file system: {0}")]
    Io(String),
    #[error("already listening")]
    AlreadyListening,
    #[error("internal: {0}")]
    Internal(String),
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Stt(e) => e.code(),
            Self::AudioDevice(_) => "audio_device",
            Self::MicrophoneDenied => "microphone_denied",
            Self::NoInputDevice => "no_input_device",
            Self::Download(_) => "download_failed",
            Self::ChecksumMismatch(_) => "checksum_mismatch",
            Self::ModelMissing(_) => "model_missing",
            Self::Io(_) => "io",
            Self::AlreadyListening => "already_listening",
            Self::Internal(_) => "internal",
            Self::NotImplemented(_) => "not_implemented",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
