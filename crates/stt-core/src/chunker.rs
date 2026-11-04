//! Groups 16 kHz audio into chunks for Whisper using the energy VAD:
//!
//! - silence before speech is dropped except for a short pre-roll;
//! - a pause (VAD hangover over) closes a chunk → [`ChunkKind::Final`];
//! - while speech is running, a growing copy is offered every few seconds → [`ChunkKind::Partial`]
//!   (preliminary text, replaced later);
//! - speech longer than 30 s is cut → [`ChunkKind::Split`]. The caller transcribes it and reports
//!   with [`Chunker::commit_split`] how much was covered by complete segments; the unfinished rest
//!   stays in the buffer and is transcribed again with the next chunk. That overlap follows the
//!   speech instead of a fixed 1–2 s and avoids duplicate text. Without any complete segment a
//!   fixed overlap ([`ChunkerConfig::split_keep`]) is kept.
//!
//! After a `Split` the chunker stops consuming input until `commit_split` – call
//! [`feed`](Chunker::feed) with an empty slice afterwards to continue.

use crate::vad::{Vad, VadConfig};

const SR: usize = crate::WHISPER_SAMPLE_RATE as usize;

/// Chunking settings (16 kHz samples).
#[derive(Debug, Clone, Copy)]
pub struct ChunkerConfig {
    pub vad: VadConfig,
    /// Longest chunk (Whisper window).
    pub max_len: usize,
    /// Shorter speech is dropped (clicks, coughs).
    pub min_speech: usize,
    /// Audio kept before the start of speech.
    pub preroll: usize,
    /// Interval for preliminary results; `None` = off.
    pub partial_every: Option<usize>,
    /// Overlap kept after a split without complete segments.
    pub split_keep: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            vad: VadConfig::default(),
            max_len: 30 * SR,
            min_speech: SR / 2,
            preroll: SR * 3 / 10,
            partial_every: Some(2 * SR),
            split_keep: SR * 3 / 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkKind {
    Final,
    Partial,
    Split,
}

/// A chunk of 16 kHz mono audio and its position in the recording.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub start_sample: u64,
    pub samples: Vec<f32>,
    pub kind: ChunkKind,
}

impl Chunk {
    pub fn start_ms(&self) -> u64 {
        self.start_sample * 1000 / SR as u64
    }
}

pub struct Chunker {
    cfg: ChunkerConfig,
    vad: Vad,
    /// Samples not yet classified by the VAD.
    pending: Vec<f32>,
    /// Current speech (or pre-roll) and the recording position of `buf[0]`.
    buf: Vec<f32>,
    buf_start: u64,
    in_speech: bool,
    /// Speech frames in `buf` (for `min_speech`).
    speech_samples: usize,
    since_partial: usize,
    split_pending: bool,
}

impl Chunker {
    pub fn new(cfg: ChunkerConfig) -> Self {
        Self {
            vad: Vad::new(cfg.vad),
            cfg,
            pending: Vec::new(),
            buf: Vec::new(),
            buf_start: 0,
            in_speech: false,
            speech_samples: 0,
            since_partial: 0,
            split_pending: false,
        }
    }

    /// Input level of the last frame (RMS, 0…1).
    pub fn level(&self) -> f32 {
        self.vad.level
    }

    /// Feeds 16 kHz mono samples and returns the chunks that became ready.
    pub fn feed(&mut self, samples: &[f32]) -> Vec<Chunk> {
        self.pending.extend_from_slice(samples);
        let n = self.vad.frame_len();
        let mut out = Vec::new();
        let mut used = 0;
        while !self.split_pending && self.pending.len() - used >= n {
            let frame = self.pending[used..used + n].to_vec();
            used += n;
            let speech = self.vad.classify(&frame);
            self.buf.extend_from_slice(&frame);
            if speech {
                if !self.in_speech {
                    self.in_speech = true;
                    self.since_partial = 0;
                }
                self.speech_samples += n;
                self.since_partial += n;
                if self.buf.len() >= self.cfg.max_len {
                    out.push(self.chunk(self.cfg.max_len, ChunkKind::Split));
                    self.split_pending = true;
                } else if self.cfg.partial_every.is_some_and(|every| {
                    self.since_partial >= every && self.speech_samples >= self.cfg.min_speech
                }) {
                    self.since_partial = 0;
                    out.push(self.chunk(self.buf.len(), ChunkKind::Partial));
                }
            } else {
                if self.in_speech {
                    self.in_speech = false;
                    if let Some(c) = self.take_final() {
                        out.push(c);
                    }
                }
                self.trim_to_preroll();
            }
        }
        self.pending.drain(..used);
        out
    }

    /// Reports how many samples of the last `Split` chunk are done (0 = unknown).
    pub fn commit_split(&mut self, consumed: usize) {
        if !self.split_pending {
            return;
        }
        let drop = if consumed > 0 { consumed } else { self.cfg.max_len - self.cfg.split_keep };
        let drop = drop.min(self.buf.len());
        self.buf.drain(..drop);
        self.buf_start += drop as u64;
        self.speech_samples = self.speech_samples.saturating_sub(drop);
        self.since_partial = 0;
        self.split_pending = false;
    }

    /// `true` while unconsumed input waits for [`commit_split`](Self::commit_split).
    pub fn is_split_pending(&self) -> bool {
        self.split_pending
    }

    /// Ends the recording: the remaining speech as a final chunk.
    pub fn finish(&mut self) -> Option<Chunk> {
        self.split_pending = false;
        let rest = std::mem::take(&mut self.pending);
        self.buf.extend_from_slice(&rest);
        let c = if self.in_speech || self.speech_samples > 0 { self.take_final() } else { None };
        self.in_speech = false;
        c
    }

    fn chunk(&self, len: usize, kind: ChunkKind) -> Chunk {
        Chunk { start_sample: self.buf_start, samples: self.buf[..len].to_vec(), kind }
    }

    fn take_final(&mut self) -> Option<Chunk> {
        let enough = self.speech_samples >= self.cfg.min_speech;
        let samples = std::mem::take(&mut self.buf);
        let start = self.buf_start;
        self.buf_start += samples.len() as u64;
        self.speech_samples = 0;
        enough.then_some(Chunk { start_sample: start, samples, kind: ChunkKind::Final })
    }

    fn trim_to_preroll(&mut self) {
        if self.buf.len() > self.cfg.preroll {
            let drop = self.buf.len() - self.cfg.preroll;
            self.buf.drain(..drop);
            self.buf_start += drop as u64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: usize = 480;

    fn cfg() -> ChunkerConfig {
        ChunkerConfig {
            vad: VadConfig { frame_len: FRAME, threshold: 0.1, hangover_frames: 2 },
            max_len: 100 * FRAME,
            min_speech: 4 * FRAME,
            preroll: FRAME,
            partial_every: None,
            split_keep: 10 * FRAME,
        }
    }

    fn loud(frames: usize) -> Vec<f32> {
        vec![0.5; frames * FRAME]
    }
    fn quiet(frames: usize) -> Vec<f32> {
        vec![0.0; frames * FRAME]
    }

    #[test]
    fn pause_closes_a_final_chunk_with_preroll() {
        let mut c = Chunker::new(cfg());
        assert!(c.feed(&quiet(5)).is_empty());
        assert!(c.feed(&loud(10)).is_empty());
        let out = c.feed(&quiet(5));
        assert_eq!(out.len(), 1);
        let chunk = &out[0];
        assert_eq!(chunk.kind, ChunkKind::Final);
        // pre-roll (1 frame) starts at frame 4; speech 10 + hangover 2 + the closing frame
        assert_eq!(chunk.start_sample, 4 * FRAME as u64);
        assert_eq!(chunk.samples.len(), (1 + 10 + 3) * FRAME);
    }

    #[test]
    fn short_blips_are_dropped() {
        let mut c = Chunker::new(cfg());
        let mut input = loud(1);
        input.extend(quiet(10));
        assert!(c.feed(&input).is_empty());
    }

    #[test]
    fn long_speech_is_split_and_continues_after_commit() {
        let mut c = Chunker::new(cfg());
        let out = c.feed(&loud(150));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, ChunkKind::Split);
        assert_eq!(out[0].samples.len(), 100 * FRAME);
        assert!(c.is_split_pending());
        // 60 frames covered by complete segments → the rest stays
        c.commit_split(60 * FRAME);
        assert!(c.feed(&[]).is_empty());
        let out = c.feed(&quiet(5));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, ChunkKind::Final);
        assert_eq!(out[0].start_sample, 60 * FRAME as u64);
        assert_eq!(out[0].samples.len(), (150 - 60 + 3) * FRAME);
    }

    #[test]
    fn split_without_segments_keeps_a_fixed_overlap() {
        let mut c = Chunker::new(cfg());
        c.feed(&loud(100));
        c.commit_split(0);
        let out = c.finish().unwrap();
        assert_eq!(out.start_sample, 90 * FRAME as u64);
        assert_eq!(out.samples.len(), 10 * FRAME);
    }

    #[test]
    fn partials_are_offered_while_speaking() {
        let mut c = Chunker::new(ChunkerConfig { partial_every: Some(5 * FRAME), ..cfg() });
        let out = c.feed(&loud(12));
        let kinds: Vec<_> = out.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, vec![ChunkKind::Partial, ChunkKind::Partial]);
        assert!(out[1].samples.len() > out[0].samples.len());
    }

    #[test]
    fn finish_returns_running_speech() {
        let mut c = Chunker::new(cfg());
        c.feed(&loud(8));
        let out = c.finish().unwrap();
        assert_eq!(out.kind, ChunkKind::Final);
        assert!(c.finish().is_none());
    }
}
