//! Debug tool: transcribes one window of a WAV file and prints the raw window result.
//!
//! ```bash
//! cargo run --release -p stt-core --example window -- file.wav <start_ms> <len_ms> [model] [language]
//! ```

#[path = "../tests/support/mod.rs"]
mod support;

use stt_core::ModelId;
use stt_core::whisper::{Transcriber, best_device};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 3 {
        eprintln!("usage: window <file.wav> <start_ms> <len_ms> [model] [language]");
        std::process::exit(2);
    }
    let ms = |s: &str| s.parse::<usize>().unwrap_or(0) * 16;
    let id = a.get(3).and_then(|m| ModelId::parse(m)).unwrap_or(ModelId::TinyQ);
    let lang = a.get(4).map(String::as_str).filter(|l| *l != "auto");
    let run = || -> Result<(), String> {
        let audio = support::load_wav(std::path::Path::new(&a[0]))?;
        let start = ms(&a[1]).min(audio.len());
        let end = (start + ms(&a[2])).min(audio.len());
        let mut t = Transcriber::load(support::load_model(id)?, &best_device())
            .map_err(|e| e.to_string())?;
        let w = t
            .transcribe_window(&audio[start..end], (start / 16) as u64, lang)
            .map_err(|e| e.to_string())?;
        println!(
            "language {} · no_speech {:.3} · consumed {} ms",
            w.language,
            w.no_speech_prob,
            w.consumed / 16
        );
        for s in &w.segments {
            println!("  {:>6}–{:>6} {}", s.start_ms, s.end_ms, s.text);
        }
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
