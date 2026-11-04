//! Streaming transcription: chunker + Whisper. Platform-neutral and single-threaded; the host
//! decides where it runs (desktop: own thread, browser: Web Worker).
//!
//! ```text
//! let mut chunks = pipeline.feed(&samples_16k);          // cheap: VAD + chunking
//! while !chunks.is_empty() {
//!     for chunk in &chunks {
//!         emit(pipeline.transcribe(&mut t, chunk)?);     // heavy: Whisper
//!     }
//!     chunks = pipeline.feed(&[]);                       // input held back after a split
//! }
//! ```
//! [`Pipeline::push`] runs exactly this loop.

use crate::chunker::{Chunk, ChunkKind, Chunker, ChunkerConfig};
use crate::whisper::Transcriber;
use crate::{Segment, SttResult, WHISPER_SAMPLE_RATE};

pub struct Pipeline {
    chunker: Chunker,
    /// Fixed language, or the one detected from the first final chunk with text.
    language: Option<String>,
}

impl Pipeline {
    /// `language`: `None` = detect automatically.
    pub fn new(cfg: ChunkerConfig, language: Option<String>) -> Self {
        Self { chunker: Chunker::new(cfg), language }
    }

    pub fn level(&self) -> f32 {
        self.chunker.level()
    }

    /// Feeds 16 kHz mono samples; returns the chunks to transcribe now.
    pub fn feed(&mut self, samples: &[f32]) -> Vec<Chunk> {
        self.chunker.feed(samples)
    }

    /// The remaining speech when recording stops.
    pub fn finish(&mut self) -> Option<Chunk> {
        self.chunker.finish()
    }

    /// Transcribes one chunk. Partial chunks give one preliminary segment (`is_final: false`);
    /// after a split the chunker is told how far the complete segments reached and more chunks may
    /// follow – they are returned by the next [`feed`](Self::feed) (an empty slice is fine).
    pub fn transcribe(&mut self, t: &mut Transcriber, chunk: &Chunk) -> SttResult<Vec<Segment>> {
        let offset_ms = chunk.start_ms();
        let w = t.transcribe_window(&chunk.samples, offset_ms, self.language.as_deref())?;
        if self.language.is_none() && chunk.kind != ChunkKind::Partial && !w.segments.is_empty() {
            self.language = Some(w.language.to_owned());
        }
        Ok(match chunk.kind {
            ChunkKind::Final => w.segments,
            ChunkKind::Partial => {
                let text = w.segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
                match (w.segments.first(), w.segments.last()) {
                    (Some(first), Some(last)) => vec![Segment {
                        text,
                        start_ms: first.start_ms,
                        end_ms: last.end_ms,
                        is_final: false,
                    }],
                    _ => Vec::new(),
                }
            }
            ChunkKind::Split => {
                if w.consumed > 0 {
                    // keep complete segments; the unfinished rest is transcribed again next time
                    let done_ms = offset_ms + w.consumed as u64 * 1000 / WHISPER_SAMPLE_RATE as u64;
                    self.chunker.commit_split(w.consumed);
                    w.segments.into_iter().filter(|s| s.end_ms <= done_ms).collect()
                } else {
                    // no closing timestamp: take the text as it is and continue without overlap –
                    // a word cut at the boundary is better than losing the whole chunk
                    self.chunker.commit_split(chunk.samples.len());
                    w.segments
                }
            }
        })
    }

    /// Feeds samples and transcribes everything that becomes ready. `before` is called before each
    /// (heavy) transcription, e.g. to report a "transcribing" status.
    pub fn push(
        &mut self,
        t: &mut Transcriber,
        samples: &[f32],
        mut before: impl FnMut(&Chunk),
    ) -> SttResult<Vec<Segment>> {
        let mut out = Vec::new();
        let mut chunks = self.feed(samples);
        while !chunks.is_empty() {
            for chunk in &chunks {
                before(chunk);
                out.extend(self.transcribe(t, chunk)?);
            }
            chunks = self.feed(&[]);
        }
        Ok(out)
    }

    /// Stops: transcribes the remaining speech.
    pub fn flush(&mut self, t: &mut Transcriber) -> SttResult<Vec<Segment>> {
        match self.finish() {
            Some(chunk) => self.transcribe(t, &chunk),
            None => Ok(Vec::new()),
        }
    }

    /// Language in use (fixed or detected so far).
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
}
