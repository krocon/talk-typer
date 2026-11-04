//! Error type of stt-core. Codes are stable `snake_case` strings for the frontend.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SttError {
    #[error("invalid audio: {0}")]
    InvalidAudio(String),
    #[error("unknown model: {0}")]
    UnknownModel(String),
    #[error("invalid model data: {0}")]
    InvalidModel(String),
    #[error("invalid tokenizer: {0}")]
    InvalidTokenizer(String),
    #[error("no model loaded")]
    NoModel,
    #[error("resampling failed: {0}")]
    Resample(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

impl SttError {
    /// Machine-readable code (`snake_case`), mirrored in the frontend.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidAudio(_) => "invalid_audio",
            Self::UnknownModel(_) => "unknown_model",
            Self::InvalidModel(_) => "invalid_model",
            Self::InvalidTokenizer(_) => "invalid_tokenizer",
            Self::NoModel => "no_model",
            Self::Resample(_) => "resample",
            Self::Inference(_) => "inference",
            Self::NotImplemented(_) => "not_implemented",
        }
    }
}

impl From<candle_core::Error> for SttError {
    fn from(e: candle_core::Error) -> Self {
        Self::Inference(e.to_string())
    }
}

pub type SttResult<T> = Result<T, SttError>;
