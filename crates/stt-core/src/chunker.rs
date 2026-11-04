//! Groups VAD-marked audio into chunks for Whisper: ≤ 30 s, closed by a pause,
//! long speech split with an overlap. Implemented in phase 3; the types are fixed now.

/// Chunking settings (16 kHz samples).
#[derive(Debug, Clone, Copy)]
pub struct ChunkerConfig {
    pub max_len: usize,
    pub overlap: usize,
    /// Minimum speech length; shorter blips are dropped.
    pub min_len: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self { max_len: 30 * 16_000, overlap: 16_000 + 8_000, min_len: 4_000 }
    }
}

/// A chunk of 16 kHz mono audio and its offset in the recording.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub start_sample: u64,
    pub samples: Vec<f32>,
    /// `false` while speech is still running (preliminary transcription).
    pub is_final: bool,
}
