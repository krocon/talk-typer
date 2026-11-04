//! Browser binding of stt-core. Runs inside a Web Worker (`frontend/src/app/stt/stt.worker.ts`);
//! the worker owns one [`SttEngine`], feeds it microphone samples and posts the segments back.
//!
//! Build: `wasm-pack build crates/stt-wasm --target web` (SIMD via `.cargo/config.toml`).

use stt_core::audio::{Resampler16k, downmix};
use stt_core::vad::{Vad, VadConfig};
use stt_core::whisper::Device;
use stt_core::whisper::{ModelBytes, Transcriber};
use stt_core::{MODELS, SttError};
use wasm_bindgen::prelude::*;

fn js_err(e: SttError) -> JsError {
    JsError::new(&format!("{}: {e}", e.code()))
}

/// JSON of the model catalogue (`ModelInfo[]`).
#[wasm_bindgen(js_name = listModels)]
pub fn list_models() -> Result<String, JsError> {
    serde_json::to_string(MODELS).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub struct SttEngine {
    transcriber: Transcriber,
    resampler: Option<Resampler16k>,
    vad: Vad,
    language: Option<String>,
}

#[wasm_bindgen]
impl SttEngine {
    /// Loads a model from the downloaded bytes.
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
        Ok(Self {
            transcriber,
            resampler: None,
            vad: Vad::new(VadConfig::default()),
            language: None,
        })
    }

    /// Starts a recording. `language`: `"auto"` or an ISO 639-1 code.
    pub fn start(&mut self, sample_rate: u32, language: &str) -> Result<(), JsError> {
        self.resampler = Some(Resampler16k::new(sample_rate).map_err(js_err)?);
        self.vad = Vad::new(VadConfig::default());
        self.language = (language != "auto").then(|| language.to_owned());
        Ok(())
    }

    /// Feeds interleaved samples; returns the segments finished so far as JSON (`Segment[]`).
    pub fn push(&mut self, samples: &[f32], channels: usize) -> Result<String, JsError> {
        let rs = self.resampler.as_mut().ok_or_else(|| js_err(SttError::NoModel))?;
        let mono = downmix(samples, channels).map_err(js_err)?;
        let resampled = rs.push(&mono).map_err(js_err)?;
        let _speech = self.vad.push(&resampled);
        // Chunking and decoding follow in phases 2–4.
        Ok("[]".to_owned())
    }

    /// Current input level (RMS, 0…1) for the level meter.
    pub fn level(&self) -> f32 {
        self.vad.level
    }

    /// Ends the recording and returns the remaining segments as JSON.
    pub fn stop(&mut self) -> Result<String, JsError> {
        self.resampler = None;
        let _ = (&mut self.transcriber, &self.language);
        Ok("[]".to_owned())
    }
}
