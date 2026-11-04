//! Browser binding of stt-core. Runs inside a Web Worker (`frontend/src/app/stt/stt.worker.ts`);
//! the worker owns one [`SttEngine`], feeds it microphone samples and posts the segments back.
//!
//! Build: `pnpm wasm` (wasm-pack, `--target web`, SIMD via `.cargo/config.toml`).
//! Single-threaded on purpose (no SharedArrayBuffer / COOP+COEP needed).

use std::collections::VecDeque;
use stt_core::audio::{Resampler16k, downmix};
use stt_core::chunker::{Chunk, ChunkKind, ChunkerConfig};
use stt_core::pipeline::Pipeline;
use stt_core::vad::VadConfig;
use stt_core::whisper::{Device, ModelBytes, Transcriber};
use stt_core::{MODELS, Segment, SttError};
use wasm_bindgen::prelude::*;

fn js_err(e: SttError) -> JsError {
    // "code: message" – the worker splits it into SttError { code, message }
    JsError::new(&format!("{}: {e}", e.code()))
}

fn to_json(segments: &[Segment]) -> Result<String, JsError> {
    serde_json::to_string(segments).map_err(|e| JsError::new(&format!("internal: {e}")))
}

/// JSON of the model catalogue (`ModelInfo[]`).
#[wasm_bindgen(js_name = listModels)]
pub fn list_models() -> Result<String, JsError> {
    serde_json::to_string(MODELS).map_err(|e| JsError::new(&format!("internal: {e}")))
}

struct Session {
    resampler: Resampler16k,
    pipeline: Pipeline,
    channels: usize,
    /// Chunks waiting for Whisper; the worker runs them one by one so it can report progress.
    queue: VecDeque<Chunk>,
}

#[wasm_bindgen]
pub struct SttEngine {
    transcriber: Transcriber,
    session: Option<Session>,
}

#[wasm_bindgen]
impl SttEngine {
    /// Loads a model from the downloaded files (copies them into WASM memory).
    #[wasm_bindgen(constructor)]
    pub fn new(
        weights: &[u8],
        config: &[u8],
        tokenizer: &[u8],
        quantized: bool,
    ) -> Result<SttEngine, JsError> {
        let bytes = ModelBytes {
            weights: weights.to_vec(),
            config: config.to_vec(),
            tokenizer: tokenizer.to_vec(),
            quantized,
        };
        let transcriber = Transcriber::load(bytes, &Device::Cpu).map_err(js_err)?;
        Ok(Self { transcriber, session: None })
    }

    /// Starts a recording.
    /// - `language`: `"auto"` or an ISO 639-1 code
    /// - `partial_every_ms`: interval for preliminary results (0 = off)
    /// - `max_chunk_ms`: longest chunk (≤ 30 000); shorter chunks = text sooner on slow CPUs
    /// - `pause_ms`: silence that closes a chunk
    pub fn start(
        &mut self,
        sample_rate: u32,
        channels: usize,
        language: &str,
        partial_every_ms: u32,
        max_chunk_ms: u32,
        pause_ms: u32,
    ) -> Result<(), JsError> {
        let language = (language != "auto").then(|| language.to_owned());
        let samples = |ms: u32| ms as usize * 16_000 / 1000;
        let base = ChunkerConfig::default();
        let cfg = ChunkerConfig {
            partial_every: (partial_every_ms > 0).then(|| samples(partial_every_ms)),
            max_len: samples(max_chunk_ms).clamp(samples(4_000), base.max_len),
            vad: VadConfig {
                hangover_frames: samples(pause_ms) / base.vad.frame_len.max(1),
                ..base.vad
            },
            ..base
        };
        self.session = Some(Session {
            resampler: Resampler16k::new(sample_rate).map_err(js_err)?,
            pipeline: Pipeline::new(cfg, language),
            channels: channels.max(1),
            queue: VecDeque::new(),
        });
        Ok(())
    }

    /// Feeds interleaved samples (cheap: resampling, VAD, chunking). Returns the number of chunks
    /// waiting for [`run_next`](Self::run_next).
    pub fn feed(&mut self, samples: &[f32]) -> Result<usize, JsError> {
        let s = self.session.as_mut().ok_or_else(|| js_err(SttError::NoModel))?;
        let mono = downmix(samples, s.channels).map_err(js_err)?;
        let resampled = s.resampler.push(&mono).map_err(js_err)?;
        s.queue.extend(s.pipeline.feed(&resampled));
        Ok(s.queue.len())
    }

    /// Kind of the next waiting chunk: `"final"`, `"split"`, `"partial"` or `""` (none).
    #[wasm_bindgen(js_name = nextKind)]
    pub fn next_kind(&self) -> String {
        let kind = self.session.as_ref().and_then(|s| s.queue.front()).map(|c| c.kind);
        match kind {
            Some(ChunkKind::Final) => "final",
            Some(ChunkKind::Split) => "split",
            Some(ChunkKind::Partial) => "partial",
            None => "",
        }
        .to_owned()
    }

    /// Transcribes the next waiting chunk (heavy) and returns its segments as JSON. A partial chunk
    /// is skipped when `allow_partial` is false or a newer chunk is already waiting.
    #[wasm_bindgen(js_name = runNext)]
    pub fn run_next(&mut self, allow_partial: bool) -> Result<String, JsError> {
        let s = self.session.as_mut().ok_or_else(|| js_err(SttError::NoModel))?;
        let Some(chunk) = s.queue.pop_front() else {
            return to_json(&[]);
        };
        if chunk.kind == ChunkKind::Partial && (!allow_partial || !s.queue.is_empty()) {
            return to_json(&[]);
        }
        let out = s.pipeline.transcribe(&mut self.transcriber, &chunk).map_err(js_err)?;
        // input held back after a split continues now
        s.queue.extend(s.pipeline.feed(&[]));
        to_json(&out)
    }

    /// Input level of the last frame (RMS, 0…1).
    pub fn level(&self) -> f32 {
        self.session.as_ref().map_or(0.0, |s| s.pipeline.level())
    }

    /// Ends the recording and returns the remaining segments as JSON.
    pub fn stop(&mut self) -> Result<String, JsError> {
        let Some(mut s) = self.session.take() else {
            return to_json(&[]);
        };
        let tail = s.resampler.flush().map_err(js_err)?;
        let mut out = Vec::new();
        s.queue.extend(s.pipeline.feed(&tail));
        while let Some(chunk) = s.queue.pop_front() {
            if chunk.kind != ChunkKind::Partial {
                out.extend(s.pipeline.transcribe(&mut self.transcriber, &chunk).map_err(js_err)?);
                s.queue.extend(s.pipeline.feed(&[]));
            }
        }
        out.extend(s.pipeline.flush(&mut self.transcriber).map_err(js_err)?);
        to_json(&out)
    }
}
