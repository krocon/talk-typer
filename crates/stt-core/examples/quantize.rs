//! Quantizes official Whisper safetensors (`openai/whisper-*`) to Q8_0 GGUF for candle's
//! `quantized_model`. Reproducible: the result must match the SHA-256 in `src/models.rs`.
//! How the files get to the model repository: `docs/models.md`.
//!
//! ```bash
//! cargo run --release -p stt-core --example quantize -- models/base/weights.safetensors model-base-q80.gguf
//! cargo run --release -p stt-core --example quantize -- --all out/      # tiny, base, small
//! cargo run --release -p stt-core --example quantize -- --inspect model-base-q80.gguf
//! ```

#[path = "../tests/support/mod.rs"]
mod support;

use candle_core::quantized::gguf_file;
use std::path::Path;
use stt_core::MODELS;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [flag, file] if flag == "--inspect" => inspect(Path::new(file)),
        [flag, dir] if flag == "--all" => all(Path::new(dir)),
        [input, output] => one(Path::new(input), Path::new(output)),
        _ => {
            Err("usage: quantize <in.safetensors> <out.gguf> | --all <dir> | --inspect <file.gguf>"
                .to_owned())
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn one(input: &Path, output: &Path) -> Result<(), String> {
    let (q, f) = support::quantize(input, output).map_err(|e| e.to_string())?;
    let size = std::fs::metadata(output).map_err(|e| e.to_string())?.len();
    let hash = support::sha256_file(output)?;
    println!("{} tensors ({q} Q8_0, {f} F32) → {} ({size} bytes)", q + f, output.display());
    println!("sha256 {hash}");
    Ok(())
}

/// Builds every quantized model of the catalogue into `dir` and checks it against the catalogue.
fn all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for m in MODELS.iter().filter(|m| m.quantized) {
        let source = support::source_of(m.id).ok_or("no source model")?;
        support::fetch(source)?;
        let weights = m.files.iter().find(|f| f.role == "weights").ok_or("no weights")?;
        let name = weights.url.rsplit('/').next().ok_or("bad url")?;
        let out = dir.join(name);
        let input = support::models_dir().join(source.as_str()).join("weights.safetensors");
        one(&input, &out)?;
        let hash = support::sha256_file(&out)?;
        if hash != weights.sha256 {
            return Err(format!("{name}: hash differs from the catalogue ({})", weights.sha256));
        }
        println!("{name}: matches the catalogue ✓");
    }
    Ok(())
}

fn inspect(file: &Path) -> Result<(), String> {
    let mut f = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let content = gguf_file::Content::read(&mut f).map_err(|e| e.to_string())?;
    let mut names: Vec<_> = content.tensor_infos.iter().collect();
    names.sort_by(|a, b| a.0.cmp(b.0));
    for (name, info) in names {
        println!("{name}\t{:?}\t{:?}", info.ggml_dtype, info.shape.dims());
    }
    Ok(())
}
