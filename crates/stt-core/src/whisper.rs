//! Whisper with candle: load from bytes, log-mel, encoder, greedy decoding with timestamps.
//!
//! Follows the official candle examples (`candle-examples/examples/whisper`,
//! `candle-wasm-examples/whisper`) with these differences:
//! - own log-mel ([`crate::mel`], no threads → runs in WASM) and own tokenizer ([`crate::tokenizer`]);
//! - greedy decoding only (no temperature fallback, no `rand`);
//! - logits post-processing on the CPU as plain `Vec<f32>` (one transfer per token);
//! - the encoder runs once per window and is shared by language detection and decoding;
//! - timestamp rules as in OpenAI's `ApplyTimestampRules` (incl. "penultimate" for short sequences).

use crate::mel;
use crate::tokenizer::Tokenizer;
use crate::{Segment, SttError, SttResult, WHISPER_SAMPLE_RATE};
pub use candle_core::Device;
use candle_core::{DType, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, Config};

/// Samples in a 30 s window.
pub const WINDOW_SAMPLES: usize = m::N_SAMPLES;
/// Frames of a 30 s window (3000).
const WINDOW_FRAMES: usize = m::N_FRAMES;
/// Seconds per timestamp token.
const TIMESTAMP_STEP_MS: u64 = 20;
/// The first timestamp may be at most 1 s into the window (OpenAI default).
const MAX_INITIAL_TIMESTAMP_INDEX: u32 = 50;

/// Whisper's languages, in token order.
pub const LANGUAGES: &[&str] = &[
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it",
    "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur",
    "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn",
    "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si",
    "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo",
    "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
    "ha", "ba", "jw", "su",
];

/// Model files as downloaded from the model host.
pub struct ModelBytes {
    pub weights: Vec<u8>,
    pub config: Vec<u8>,
    pub tokenizer: Vec<u8>,
    pub quantized: bool,
}

/// Best available device: Metal (feature `metal`), CUDA (feature `cuda`), else CPU.
pub fn best_device() -> Device {
    #[cfg(feature = "metal")]
    if let Ok(d) = Device::new_metal(0) {
        return d;
    }
    #[cfg(feature = "cuda")]
    if let Ok(d) = Device::new_cuda(0) {
        return d;
    }
    Device::Cpu
}

enum Model {
    Normal(m::model::Whisper),
    Quantized(m::quantized_model::Whisper),
}

impl Model {
    fn encode(&mut self, mel: &Tensor) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.encoder.forward(mel, true),
            Self::Quantized(m) => m.encoder.forward(mel, true),
        }
    }

    fn decode(&mut self, tokens: &Tensor, xa: &Tensor, flush: bool) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.forward(tokens, xa, flush),
            Self::Quantized(m) => m.decoder.forward(tokens, xa, flush),
        }
    }

    fn final_linear(&self, x: &Tensor) -> candle_core::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.final_linear(x),
            Self::Quantized(m) => m.decoder.final_linear(x),
        }
    }
}

struct SpecialTokens {
    sot: u32,
    transcribe: u32,
    eot: u32,
    no_speech: u32,
    no_timestamps: u32,
    languages: Vec<u32>,
}

/// Result of one ≤ 30 s window.
#[derive(Debug, Clone)]
pub struct WindowResult {
    pub segments: Vec<Segment>,
    /// Language used for decoding (given or detected).
    pub language: &'static str,
    /// Samples of the window covered by complete (timestamp-closed) segments; for seeking.
    pub consumed: usize,
    pub no_speech_prob: f32,
}

pub struct Transcriber {
    model: Model,
    config: Config,
    tokenizer: Tokenizer,
    filters: Vec<f32>,
    device: Device,
    tokens: SpecialTokens,
    /// Additive mask (0 / -inf) over the vocabulary: config `suppress_tokens` + `<|notimestamps|>`.
    suppress: Vec<f32>,
}

impl Transcriber {
    pub fn load(bytes: ModelBytes, device: &Device) -> SttResult<Self> {
        let config: Config = serde_json::from_slice(&bytes.config)
            .map_err(|e| SttError::InvalidModel(format!("config.json: {e}")))?;
        let tokenizer = Tokenizer::from_json(&bytes.tokenizer)?;
        let invalid = |e: candle_core::Error| SttError::InvalidModel(e.to_string());
        let model = if bytes.quantized {
            let vb = candle_transformers::quantized_var_builder::VarBuilder::from_gguf_buffer(
                &bytes.weights,
                device,
            )
            .map_err(invalid)?;
            Model::Quantized(
                m::quantized_model::Whisper::load(&vb, config.clone()).map_err(invalid)?,
            )
        } else {
            let vb = VarBuilder::from_buffered_safetensors(bytes.weights, m::DTYPE, device)
                .map_err(invalid)?;
            Model::Normal(m::model::Whisper::load(&vb, config.clone()).map_err(invalid)?)
        };

        let no_speech = m::NO_SPEECH_TOKENS
            .iter()
            .find_map(|t| tokenizer.token_id(t).ok())
            .ok_or_else(|| SttError::InvalidTokenizer("no no-speech token".into()))?;
        let languages = LANGUAGES
            .iter()
            .map(|l| tokenizer.token_id(&format!("<|{l}|>")))
            .collect::<SttResult<Vec<_>>>()?;
        let tokens = SpecialTokens {
            sot: tokenizer.token_id(m::SOT_TOKEN)?,
            transcribe: tokenizer.token_id(m::TRANSCRIBE_TOKEN)?,
            eot: tokenizer.token_id(m::EOT_TOKEN)?,
            no_speech,
            no_timestamps: tokenizer.token_id(m::NO_TIMESTAMPS_TOKEN)?,
            languages,
        };
        let mut suppress = vec![0f32; config.vocab_size];
        for &t in config.suppress_tokens.iter().chain(std::iter::once(&tokens.no_timestamps)) {
            if let Some(v) = suppress.get_mut(t as usize) {
                *v = f32::NEG_INFINITY;
            }
        }
        let filters = mel::mel_filters(config.num_mel_bins);
        Ok(Self { model, config, tokenizer, filters, device: device.clone(), tokens, suppress })
    }

    pub fn mel_bins(&self) -> usize {
        self.config.num_mel_bins
    }

    /// Transcribes any length of 16 kHz mono audio in 30 s windows, seeking to the last complete
    /// segment like OpenAI's `transcribe()`. `language`: `None` = detect once from the start.
    pub fn transcribe_all(
        &mut self,
        samples: &[f32],
        language: Option<&str>,
    ) -> SttResult<Vec<Segment>> {
        let mut segments = Vec::new();
        let mut language = language.map(str::to_owned);
        let mut seek = 0usize;
        let min_tail = WHISPER_SAMPLE_RATE as usize / 5;
        while samples.len().saturating_sub(seek) > min_tail {
            let end = (seek + WINDOW_SAMPLES).min(samples.len());
            let offset_ms = seek as u64 * 1000 / WHISPER_SAMPLE_RATE as u64;
            let w = self.transcribe_window(&samples[seek..end], offset_ms, language.as_deref())?;
            language.get_or_insert_with(|| w.language.to_owned());
            segments.extend(w.segments);
            let full = end - seek == WINDOW_SAMPLES;
            // Seek to the end of the last closed segment; at least 1 s to avoid loops.
            let step = if full && w.consumed > 0 { w.consumed } else { end - seek };
            seek += step.max(WHISPER_SAMPLE_RATE as usize);
        }
        Ok(segments)
    }

    /// Transcribes one window of at most 30 s; `offset_ms` is added to all times.
    pub fn transcribe_window(
        &mut self,
        samples: &[f32],
        offset_ms: u64,
        language: Option<&str>,
    ) -> SttResult<WindowResult> {
        let samples = &samples[..samples.len().min(WINDOW_SAMPLES)];
        let n_mels = self.config.num_mel_bins;
        let mut padded = samples.to_vec();
        padded.resize(WINDOW_SAMPLES, 0.0);
        let mel = mel::log_mel(&padded, &self.filters, n_mels);
        let mel = Tensor::from_vec(mel, (1, n_mels, WINDOW_FRAMES), &self.device)?;
        let features = self.model.encode(&mel)?;

        let lang_idx = match language {
            Some(code) => LANGUAGES
                .iter()
                .position(|l| *l == code)
                .ok_or_else(|| SttError::InvalidAudio(format!("unknown language {code}")))?,
            None => self.detect_language(&features)?,
        };
        let lang_token = self.tokens.languages[lang_idx];

        let (tokens, no_speech_prob, avg_logprob) = self.decode_greedy(&features, lang_token)?;
        let duration_ms = samples.len() as u64 * 1000 / WHISPER_SAMPLE_RATE as u64;
        let ts_begin = self.tokens.no_timestamps + 1;

        let mut segments = Vec::new();
        let mut consumed_ms = 0u64;
        if !(no_speech_prob > m::NO_SPEECH_THRESHOLD as f32
            && avg_logprob < m::LOGPROB_THRESHOLD as f32)
        {
            let mut start: Option<u64> = None;
            let mut text_tokens: Vec<u32> = Vec::new();
            for &t in &tokens {
                if t >= ts_begin {
                    // Whisper may place a timestamp in the padding after the audio: clamp
                    let ms = ((t - ts_begin) as u64 * TIMESTAMP_STEP_MS).min(duration_ms);
                    if text_tokens.is_empty() {
                        start = Some(ms);
                    } else {
                        self.push_segment(
                            &mut segments,
                            &text_tokens,
                            start.unwrap_or(0),
                            ms,
                            offset_ms,
                        );
                        text_tokens.clear();
                        start = None;
                        consumed_ms = ms;
                    }
                } else if t < self.tokens.eot {
                    text_tokens.push(t);
                }
            }
            if !text_tokens.is_empty() {
                let s = start.unwrap_or(consumed_ms);
                self.push_segment(&mut segments, &text_tokens, s, duration_ms.max(s), offset_ms);
            }
        }
        let consumed = (consumed_ms.min(duration_ms) * WHISPER_SAMPLE_RATE as u64 / 1000) as usize;
        Ok(WindowResult { segments, language: LANGUAGES[lang_idx], consumed, no_speech_prob })
    }

    fn push_segment(&self, out: &mut Vec<Segment>, tokens: &[u32], s: u64, e: u64, off: u64) {
        let text = self.tokenizer.decode(tokens).trim().to_owned();
        if !text.is_empty() {
            out.push(Segment { text, start_ms: off + s, end_ms: off + e, is_final: true });
        }
    }

    /// Index into [`LANGUAGES`] of the most likely language.
    fn detect_language(&mut self, features: &Tensor) -> SttResult<usize> {
        let tokens = Tensor::new(&[[self.tokens.sot]], &self.device)?;
        let ys = self.model.decode(&tokens, features, true)?;
        let logits: Vec<f32> = self.model.final_linear(&ys.i(..1)?)?.i(0)?.i(0)?.to_vec1()?;
        self.tokens
            .languages
            .iter()
            .enumerate()
            .filter_map(|(i, &t)| logits.get(t as usize).map(|l| (i, *l)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
            .ok_or_else(|| SttError::Inference("language detection failed".into()))
    }

    /// Greedy decoding with timestamps. Returns sampled tokens (after the prompt),
    /// the no-speech probability and the average log-probability.
    fn decode_greedy(&mut self, features: &Tensor, lang: u32) -> SttResult<(Vec<u32>, f32, f32)> {
        let mut tokens = vec![self.tokens.sot, lang, self.tokens.transcribe];
        let sample_begin = tokens.len();
        let max_len = self.config.max_target_positions / 2;
        let mut sum_logprob = 0f32;
        let mut no_speech_prob = 0f32;
        for i in 0..max_len {
            let input = Tensor::new(tokens.as_slice(), &self.device)?.unsqueeze(0)?;
            let ys = self.model.decode(&input, features, i == 0)?;
            if i == 0 {
                let first: Vec<f32> =
                    self.model.final_linear(&ys.i(..1)?)?.i(0)?.i(0)?.to_vec1()?;
                no_speech_prob = softmax_at(&first, self.tokens.no_speech as usize);
            }
            let (_, seq_len, _) = ys.dims3()?;
            let mut logits: Vec<f32> = self
                .model
                .final_linear(&ys.i((..1, seq_len - 1..))?)?
                .i(0)?
                .i(0)?
                .to_dtype(DType::F32)?
                .to_vec1()?;
            for (l, s) in logits.iter_mut().zip(&self.suppress) {
                *l += s;
            }
            self.apply_timestamp_rules(&mut logits, &tokens[sample_begin..]);
            let (next, _) = logits
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .ok_or_else(|| SttError::Inference("empty logits".into()))?;
            let next = next as u32;
            tokens.push(next);
            if next == self.tokens.eot || tokens.len() > self.config.max_target_positions {
                break;
            }
            if let Some(keep) = repetition_cut(&tokens[sample_begin..]) {
                // Greedy decoding got stuck in a loop: keep one repetition, end the window.
                tokens.truncate(sample_begin + keep);
                break;
            }
            sum_logprob += log_softmax_at(&logits, next as usize);
        }
        let sampled = tokens.split_off(sample_begin);
        let avg = sum_logprob / sampled.len().max(1) as f32;
        Ok((sampled, no_speech_prob, avg))
    }

    /// OpenAI `ApplyTimestampRules` on a logits vector.
    fn apply_timestamp_rules(&self, logits: &mut [f32], sampled: &[u32]) {
        let ts_begin = (self.tokens.no_timestamps + 1) as usize;
        let eot = self.tokens.eot as usize;
        let vocab = logits.len();
        if ts_begin >= vocab {
            return;
        }
        let is_ts = |t: u32| t as usize >= ts_begin;
        let mask = |logits: &mut [f32], range: std::ops::Range<usize>| {
            for v in &mut logits[range.start.min(vocab)..range.end.min(vocab)] {
                *v = f32::NEG_INFINITY;
            }
        };

        let last_ts = sampled.last().is_some_and(|&t| is_ts(t));
        let penultimate_ts = sampled.len() < 2 || is_ts(sampled[sampled.len() - 2]);
        if last_ts {
            if penultimate_ts {
                mask(logits, ts_begin..vocab); // two timestamps in a row: text must follow
            } else {
                mask(logits, 0..eot); // a timestamp must close the segment (or end of text)
            }
        }
        if let Some(&last) = sampled.iter().rev().find(|&&t| is_ts(t)) {
            // timestamps never go backwards
            let limit = if last_ts && !penultimate_ts { last } else { last + 1 } as usize;
            mask(logits, ts_begin..limit);
        }
        if sampled.is_empty() {
            mask(logits, 0..ts_begin); // the first token is a timestamp …
            let last_allowed = ts_begin + MAX_INITIAL_TIMESTAMP_INDEX as usize;
            mask(logits, last_allowed + 1..vocab); // … within the first second
        }
        // If timestamps are more likely in total than any single text token, force a timestamp.
        let lse = log_sum_exp(logits);
        let ts_logprob = log_sum_exp(&logits[ts_begin..]) - lse;
        let max_text = logits[..ts_begin].iter().copied().fold(f32::NEG_INFINITY, f32::max) - lse;
        if ts_logprob > max_text {
            mask(logits, 0..ts_begin);
        }
    }
}

/// Repetition guard (stands in for OpenAI's temperature fallback, which needs sampling): if the
/// sampled tokens end with the same n-gram (n ≤ 16) four times in a row, returns the length to
/// keep – everything up to and including the first of these repetitions.
fn repetition_cut(sampled: &[u32]) -> Option<usize> {
    const REPEATS: usize = 4;
    for n in 1..=16 {
        if sampled.len() < n * REPEATS {
            break;
        }
        let tail = &sampled[sampled.len() - n..];
        let all_same = (1..REPEATS).all(|r| {
            let start = sampled.len() - n * (r + 1);
            &sampled[start..start + n] == tail
        });
        if all_same {
            return Some(sampled.len() - n * (REPEATS - 1));
        }
    }
    None
}

fn log_sum_exp(v: &[f32]) -> f32 {
    let max = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if max == f32::NEG_INFINITY {
        return max;
    }
    max + v.iter().map(|x| (x - max).exp()).sum::<f32>().ln()
}

fn log_softmax_at(v: &[f32], i: usize) -> f32 {
    v.get(i).map_or(f32::NEG_INFINITY, |x| x - log_sum_exp(v))
}

fn softmax_at(v: &[f32], i: usize) -> f32 {
    log_softmax_at(v, i).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn softmax_helpers() {
        let v = [0.0f32, 0.0];
        assert!((softmax_at(&v, 0) - 0.5).abs() < 1e-6);
        assert!((log_sum_exp(&[f32::NEG_INFINITY, 0.0])).abs() < 1e-6);
        assert_eq!(log_sum_exp(&[f32::NEG_INFINITY]), f32::NEG_INFINITY);
    }

    #[test]
    fn repetition_guard_keeps_one_repetition() {
        assert_eq!(repetition_cut(&[1, 2, 3]), None);
        assert_eq!(repetition_cut(&[9, 5, 5, 5, 5]), Some(2));
        assert_eq!(repetition_cut(&[9, 1, 2, 1, 2, 1, 2, 1, 2]), Some(3));
        // three repeats are still allowed (counting, lists)
        assert_eq!(repetition_cut(&[1, 2, 1, 2, 1, 2]), None);
    }

    #[test]
    fn languages_contain_german_and_are_unique() {
        assert!(LANGUAGES.contains(&"de"));
        let mut v = LANGUAGES.to_vec();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), LANGUAGES.len());
    }
}
