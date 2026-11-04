//! Streaming pipeline with a real model (ignored by default, see `transcribe_de.rs`):
//! audio arrives in 100 ms blocks like from a microphone.

// Test code: helpers outside #[test] fns may unwrap too.
#![allow(clippy::unwrap_used)]

mod support;

use std::path::Path;
use stt_core::chunker::ChunkerConfig;
use stt_core::pipeline::Pipeline;
use stt_core::vad::VadConfig;
use stt_core::whisper::{Transcriber, best_device};
use stt_core::{ModelId, Segment};

fn model() -> ModelId {
    std::env::var("STT_TEST_MODEL").ok().and_then(|s| ModelId::parse(&s)).unwrap_or(ModelId::TinyQ)
}

fn sample() -> Vec<f32> {
    support::load_wav(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/de-sample.wav"))
        .unwrap()
}

fn stream(audio: &[f32], partials: bool) -> (Vec<Segment>, Vec<Segment>) {
    let cfg = ChunkerConfig { partial_every: partials.then_some(32_000), ..Default::default() };
    stream_with(audio, cfg)
}

fn stream_with(audio: &[f32], cfg: ChunkerConfig) -> (Vec<Segment>, Vec<Segment>) {
    let id = model();
    assert!(support::model_present(id), "model {} missing", id.as_str());
    let mut t = Transcriber::load(support::load_model(id).unwrap(), &best_device()).unwrap();
    let mut p = Pipeline::new(cfg, Some("de".into()));
    let mut all = Vec::new();
    for block in audio.chunks(1_600) {
        all.extend(
            p.push(&mut t, block, |c| {
                if c.kind != stt_core::chunker::ChunkKind::Partial {
                    eprintln!(
                        "chunk {:?} @{} ms, {} ms",
                        c.kind,
                        c.start_ms(),
                        c.samples.len() / 16
                    )
                }
            })
            .unwrap(),
        );
    }
    all.extend(p.flush(&mut t).unwrap());
    let (finals, partials) = all.into_iter().partition(|s| s.is_final);
    (finals, partials)
}

fn text(segments: &[Segment]) -> String {
    segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ").to_lowercase()
}

#[test]
#[ignore = "needs a downloaded model"]
fn pauses_close_chunks() {
    let s = sample();
    let mut audio = Vec::new();
    for _ in 0..3 {
        audio.extend_from_slice(&s);
        audio.extend(std::iter::repeat_n(0.0, 16_000));
    }
    let (finals, partials) = stream(&audio, true);
    let t = text(&finals);
    println!("{t}");
    assert_eq!(t.matches("morgen").count(), 3, "{t}");
    assert!(!partials.is_empty(), "no preliminary results");
    assert!(finals.windows(2).all(|w| w[0].end_ms <= w[1].start_ms + 500), "{finals:?}");
}

/// `de-long.wav`: 38 s of varied German, short pauses only between sentences.
#[test]
#[ignore = "needs a downloaded model"]
fn long_recording_keeps_every_sentence_once() {
    let (finals, _) = stream(&long_audio(), true);
    check_long(
        &finals,
        &["lokal", "cloud", "speicher", "abschnitt", "kopieren", "prozessor", "schluss"],
    );
}

/// The browser's settings (`stt.worker.ts`): 12 s chunks, 300 ms pause, no partials.
/// Splits happen inside sentences here – no text may get lost.
#[test]
#[ignore = "needs a downloaded model"]
fn long_recording_with_browser_chunking_loses_nothing() {
    let base = ChunkerConfig::default();
    let cfg = ChunkerConfig {
        partial_every: None,
        max_len: 12 * 16_000,
        vad: VadConfig { hangover_frames: 10, ..base.vad },
        ..base
    };
    let (finals, _) = stream_with(&long_audio(), cfg);
    check_long(&finals, &["lokal", "cloud", "kopieren", "prozessor", "schluss"]);
}

fn long_audio() -> Vec<f32> {
    support::load_wav(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/de-long.wav"))
        .unwrap()
}

/// Every sentence once: `words` are ones even `tiny` gets right – the tests are about chunking,
/// not recognition quality.
fn check_long(finals: &[Segment], words: &[&str]) {
    for f in finals {
        println!("{:>6}–{:>6} {}", f.start_ms, f.end_ms, f.text);
    }
    let t = text(finals);
    for &word in words {
        let n = t.split(|c: char| !c.is_alphanumeric()).filter(|w| *w == word).count();
        assert_eq!(n, 1, "'{word}' not exactly once in: {t}");
    }
    assert!(finals.windows(2).all(|w| w[0].start_ms <= w[1].start_ms), "{finals:?}");
    assert!(finals.last().unwrap().end_ms > 35_000, "{finals:?}");
}
