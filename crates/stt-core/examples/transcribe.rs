//! Transcribes a WAV file offline.
//!
//! ```bash
//! cargo run --release --example transcribe -- sample.wav [--model base] [--language de|auto] [--fetch]
//! ```
//! Models live in `<repo>/models/<id>/` (or `$STT_MODELS_DIR`); `--fetch` downloads a missing one.

#[path = "../tests/support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::time::Instant;
use stt_core::whisper::{Transcriber, best_device};
use stt_core::{ModelId, WHISPER_SAMPLE_RATE};

struct Args {
    wav: PathBuf,
    model: ModelId,
    language: Option<String>,
    fetch: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut wav = None;
    let mut model = ModelId::Base;
    let mut language = None;
    let mut fetch = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--model" => {
                let v = it.next().ok_or("--model needs a value")?;
                model = ModelId::parse(&v).ok_or_else(|| format!("unknown model {v}"))?;
            }
            "--language" => {
                let v = it.next().ok_or("--language needs a value")?;
                language = (v != "auto").then_some(v);
            }
            "--fetch" => fetch = true,
            "-h" | "--help" => return Err(String::new()),
            other => wav = Some(PathBuf::from(other)),
        }
    }
    let wav = wav.ok_or("missing WAV file")?;
    Ok(Args { wav, model, language, fetch })
}

fn main() {
    if let Err(e) = run() {
        if !e.is_empty() {
            eprintln!("error: {e}");
        }
        eprintln!(
            "usage: transcribe <file.wav> [--model tiny|tiny-q|base|small] \
             [--language de|en|…|auto] [--fetch]"
        );
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if !support::model_present(args.model) {
        if !args.fetch {
            return Err(format!(
                "model {} not found in {} – add --fetch to download it",
                args.model.as_str(),
                support::models_dir().display()
            ));
        }
        support::fetch(args.model)?;
    }
    let audio = support::load_wav(&args.wav)?;
    let audio_s = audio.len() as f64 / WHISPER_SAMPLE_RATE as f64;

    let device = best_device();
    let t = Instant::now();
    let mut transcriber =
        Transcriber::load(support::load_model(args.model)?, &device).map_err(|e| e.to_string())?;
    eprintln!("model {} on {:?} loaded in {:.2?}", args.model.as_str(), device, t.elapsed());

    let t = Instant::now();
    let segments =
        transcriber.transcribe_all(&audio, args.language.as_deref()).map_err(|e| e.to_string())?;
    let took = t.elapsed().as_secs_f64();
    for s in &segments {
        println!(
            "[{:>7.2}s → {:>7.2}s] {}",
            s.start_ms as f64 / 1e3,
            s.end_ms as f64 / 1e3,
            s.text
        );
    }
    eprintln!("{audio_s:.1} s audio in {took:.2} s (real-time factor {:.2})", took / audio_s);
    Ok(())
}
