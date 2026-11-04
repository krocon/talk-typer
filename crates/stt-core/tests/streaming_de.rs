//! Streaming pipeline with a real model (ignored by default, see `transcribe_de.rs`):
//! audio arrives in 100 ms blocks like from a microphone.

// Test code: helpers outside #[test] fns may unwrap too.
#![allow(clippy::unwrap_used)]

mod support;

use std::path::Path;
use stt_core::chunker::ChunkerConfig;
use stt_core::pipeline::Pipeline;
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
    let id = model();
    assert!(support::model_present(id), "model {} missing", id.as_str());
    let mut t = Transcriber::load(support::load_model(id).unwrap(), &best_device()).unwrap();
    let cfg = ChunkerConfig { partial_every: partials.then_some(32_000), ..Default::default() };
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
    let audio =
        support::load_wav(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/de-long.wav"))
            .unwrap();
    let (finals, _) = stream(&audio, true);
    for f in &finals {
        println!("{:>6}–{:>6} {}", f.start_ms, f.end_ms, f.text);
    }
    let t = text(&finals);
    // words even `tiny` gets right – the test is about chunking, not recognition quality
    for word in ["lokal", "cloud", "speicher", "abschnitt", "kopieren", "prozessor", "schluss"] {
        let n = t.split(|c: char| !c.is_alphanumeric()).filter(|w| *w == word).count();
        assert_eq!(n, 1, "'{word}' not exactly once in: {t}");
    }
    assert!(finals.windows(2).all(|w| w[0].start_ms <= w[1].start_ms), "{finals:?}");
    assert!(finals.last().unwrap().end_ms > 35_000, "{finals:?}");
}
