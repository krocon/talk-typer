//! Helpers shared by `examples/transcribe.rs` and the integration tests (dev only: file system,
//! `curl` for downloads). Not part of the library – stt-core itself never touches files.
#![allow(dead_code)]

use candle_core::quantized::{GgmlDType, QTensor, gguf_file};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use stt_core::audio::{Resampler16k, downmix};
use stt_core::whisper::ModelBytes;
use stt_core::{ModelId, ModelInfo};

/// `$STT_MODELS_DIR` or `<repo>/models` (git-ignored).
pub fn models_dir() -> PathBuf {
    std::env::var_os("STT_MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models"))
}

fn file_name(info: &ModelInfo, role: &str) -> String {
    match role {
        "weights" if info.quantized => "weights.gguf".into(),
        "weights" => "weights.safetensors".into(),
        other => format!("{other}.json"),
    }
}

pub fn model_present(id: ModelId) -> bool {
    let info = id.info();
    let dir = models_dir().join(id.as_str());
    info.files.iter().all(|f| dir.join(file_name(info, f.role)).is_file())
}

/// Unquantized model a quantized one is made from.
pub fn source_of(id: ModelId) -> Option<ModelId> {
    match id {
        ModelId::TinyQ => Some(ModelId::Tiny),
        ModelId::BaseQ => Some(ModelId::Base),
        ModelId::SmallQ => Some(ModelId::Small),
        _ => None,
    }
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

/// Downloads the model files with `curl` and checks their SHA-256 (dev tool; the app uses its own
/// downloader). Quantized weights that cannot be downloaded (own repository not filled yet) are
/// made locally from the OpenAI weights – reproducibly, so the hash still has to match.
pub fn fetch(id: ModelId) -> Result<(), String> {
    let info = id.info();
    let dir = models_dir().join(id.as_str());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for f in info.files {
        let target = dir.join(file_name(info, f.role));
        if target.is_file() {
            continue;
        }
        let part = target.with_extension("part");
        if !download(f.url, &part) {
            let source = source_of(id).filter(|_| f.role == "weights");
            let Some(source) = source else {
                return Err(format!("download failed: {}", f.url));
            };
            eprintln!("not downloadable – quantizing {} locally", source.as_str());
            fetch(source)?;
            let input = models_dir().join(source.as_str()).join("weights.safetensors");
            quantize(&input, &part).map_err(|e| e.to_string())?;
        }
        let hash = sha256_file(&part)?;
        if hash != f.sha256 {
            let _ = std::fs::remove_file(&part);
            return Err(format!("SHA-256 mismatch for {}: {hash}", f.url));
        }
        std::fs::rename(&part, &target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn download(url: &str, to: &Path) -> bool {
    eprintln!("downloading {url} …");
    Command::new("curl")
        .args(["-fL", "--progress-bar", "-o"])
        .arg(to)
        .arg(url)
        .status()
        .is_ok_and(|s| s.success())
}

/// Quantizes OpenAI safetensors to GGUF like candle's `tensor-tools quantize`: 2-D tensors whose
/// row length is a multiple of the block size become Q8_0, the rest stays F32. Tensors are written
/// sorted by name, so the output is reproducible (same input → same SHA-256).
pub fn quantize(input: &Path, output: &Path) -> candle_core::Result<(usize, usize)> {
    let dtype = GgmlDType::Q8_0;
    let mut tensors: Vec<_> =
        candle_core::safetensors::load(input, &candle_core::Device::Cpu)?.into_iter().collect();
    tensors.sort_by(|a, b| a.0.cmp(&b.0));
    let mut quantized = Vec::with_capacity(tensors.len());
    let (mut q, mut f) = (0, 0);
    for (name, tensor) in tensors {
        let tensor = tensor.to_dtype(candle_core::DType::F32)?;
        let to_q8 = tensor.rank() == 2 && tensor.dim(1)? % dtype.block_size() == 0;
        if to_q8 {
            q += 1
        } else {
            f += 1
        }
        let target = if to_q8 { dtype } else { GgmlDType::F32 };
        quantized.push((name, QTensor::quantize(&tensor, target)?));
    }
    let refs: Vec<(&str, &QTensor)> = quantized.iter().map(|(n, t)| (n.as_str(), t)).collect();
    let mut out = std::fs::File::create(output)?;
    gguf_file::write(&mut out, &[], &refs)?;
    Ok((q, f))
}

pub fn load_model(id: ModelId) -> Result<ModelBytes, String> {
    let info = id.info();
    let dir = models_dir().join(id.as_str());
    let read = |role: &str| {
        let p = dir.join(file_name(info, role));
        std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    Ok(ModelBytes {
        weights: read("weights")?,
        config: read("config")?,
        tokenizer: read("tokenizer")?,
        quantized: info.quantized,
    })
}

/// Reads a WAV file (PCM int or float, any rate/channels) as 16 kHz mono.
pub fn load_wav(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => {
            reader.samples::<f32>().collect::<Result<_, _>>().map_err(|e| e.to_string())?
        }
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        }
    };
    let mono = downmix(&samples, spec.channels as usize).map_err(|e| e.to_string())?;
    let mut rs = Resampler16k::new(spec.sample_rate).map_err(|e| e.to_string())?;
    let mut out = rs.push(&mono).map_err(|e| e.to_string())?;
    out.extend(rs.flush().map_err(|e| e.to_string())?);
    Ok(out)
}
