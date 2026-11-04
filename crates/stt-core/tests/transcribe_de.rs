//! End-to-end: German sample → text. Needs a downloaded model, therefore ignored by default:
//!
//! ```bash
//! cargo run --release -p stt-core --example transcribe -- crates/stt-core/tests/data/de-sample.wav --model tiny-q --fetch
//! cargo test --release -p stt-core --test transcribe_de -- --ignored
//! ```
//! `STT_TEST_MODEL` picks the model (default `tiny-q`).

// Test code: helpers outside #[test] fns may unwrap too.
#![allow(clippy::unwrap_used)]

mod support;

use std::path::Path;
use stt_core::ModelId;
use stt_core::whisper::{Transcriber, best_device};

fn model() -> ModelId {
    std::env::var("STT_TEST_MODEL").ok().and_then(|s| ModelId::parse(&s)).unwrap_or(ModelId::TinyQ)
}

fn transcribe(language: Option<&str>) -> (String, Vec<stt_core::Segment>) {
    let id = model();
    assert!(support::model_present(id), "model {} missing – see the module docs", id.as_str());
    let audio =
        support::load_wav(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/de-sample.wav"))
            .unwrap();
    let mut t = Transcriber::load(support::load_model(id).unwrap(), &best_device()).unwrap();
    let segments = t.transcribe_all(&audio, language).unwrap();
    let text = segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
    (text.to_lowercase(), segments)
}

#[test]
#[ignore = "needs a downloaded model"]
fn german_sample_with_fixed_language() {
    let (text, segments) = transcribe(Some("de"));
    println!("{text}");
    for word in ["morgen", "heute", "tag", "sprach"] {
        assert!(text.contains(word), "'{word}' missing in: {text}");
    }
    assert!(segments.iter().all(|s| s.start_ms <= s.end_ms && s.end_ms <= 7_000), "{segments:?}");
}

#[test]
#[ignore = "needs a downloaded model"]
fn german_sample_with_auto_detection() {
    let (text, _) = transcribe(None);
    assert!(text.contains("morgen"), "{text}");
}
