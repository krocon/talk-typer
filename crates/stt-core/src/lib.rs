//! stt-core – platform-neutral, fully local speech-to-text with OpenAI Whisper.
//!
//! Pipeline: any sample rate / channel count → mono 16 kHz ([`audio`]) → energy VAD ([`vad`]) →
//! chunks of ≤ 30 s ([`chunker`]) → log-mel → Whisper encoder/decoder ([`whisper`], candle) →
//! [`Segment`]s with timestamps.
//!
//! The crate never touches the file system, the network or threads, so it builds for native targets
//! (Tauri desktop app) and for `wasm32-unknown-unknown` (browser Web Worker) alike. Models,
//! `tokenizer.json` and `config.json` are passed in as bytes by the caller.

pub mod audio;
pub mod chunker;
pub mod error;
pub mod mel;
pub mod models;
pub mod pipeline;
pub mod segment;
pub mod tokenizer;
pub mod vad;
pub mod whisper;

pub use error::{SttError, SttResult};
pub use models::{MODELS, ModelFile, ModelId, ModelInfo};
pub use segment::Segment;

/// Sample rate Whisper expects.
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;
