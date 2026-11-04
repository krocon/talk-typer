//! Model cache in the app data directory (`<app data>/models/<id>/`) and the download from
//! HuggingFace – the only network access of the app. Every file is streamed to `*.part`, checked
//! against size and SHA-256 from the catalogue (`stt_core::models`) and only then renamed.

use crate::error::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use stt_core::whisper::ModelBytes;
use stt_core::{ModelFile, ModelId, ModelInfo};

pub struct ModelStore {
    root: PathBuf,
}

impl ModelStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn dir(&self, id: ModelId) -> PathBuf {
        self.root.join(id.as_str())
    }

    fn path(&self, info: &ModelInfo, file: &ModelFile) -> PathBuf {
        let name = match file.role {
            "weights" if info.quantized => "weights.gguf".to_owned(),
            "weights" => "weights.safetensors".to_owned(),
            role => format!("{role}.json"),
        };
        self.dir(info.id).join(name)
    }

    fn file_ok(&self, info: &ModelInfo, file: &ModelFile) -> bool {
        std::fs::metadata(self.path(info, file)).is_ok_and(|m| m.len() == file.size)
    }

    /// All files present with the expected size (the hash was checked when downloading).
    pub fn is_downloaded(&self, id: ModelId) -> bool {
        let info = id.info();
        info.files.iter().all(|f| self.file_ok(info, f))
    }

    /// Downloads missing files. `progress(received, total)` is called at most ~10×/s.
    pub async fn download(&self, id: ModelId, progress: impl Fn(u64, u64)) -> AppResult<()> {
        let info = id.info();
        std::fs::create_dir_all(self.dir(id))?;
        let total: u64 = info.files.iter().map(|f| f.size).sum();
        let mut received = 0u64;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .user_agent(concat!("TalkTyper/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| AppError::Download(e.to_string()))?;
        for file in info.files {
            if self.file_ok(info, file) {
                received += file.size;
                continue;
            }
            let target = self.path(info, file);
            let done = received;
            let result = fetch(&client, file, &target, |n| progress(done + n, total)).await;
            if result.is_err() {
                let _ = std::fs::remove_file(target.with_extension("part"));
            }
            result?;
            received += file.size;
        }
        progress(total, total);
        Ok(())
    }

    /// Reads a downloaded model (blocking – call from `spawn_blocking`).
    pub fn load(&self, id: ModelId) -> AppResult<ModelBytes> {
        if !self.is_downloaded(id) {
            return Err(AppError::ModelMissing(id.as_str().to_owned()));
        }
        let info = id.info();
        let read = |role: &str| -> AppResult<Vec<u8>> {
            let file = info
                .files
                .iter()
                .find(|f| f.role == role)
                .ok_or_else(|| AppError::Internal(format!("no {role} file")))?;
            Ok(std::fs::read(self.path(info, file))?)
        };
        Ok(ModelBytes {
            weights: read("weights")?,
            config: read("config")?,
            tokenizer: read("tokenizer")?,
            quantized: info.quantized,
        })
    }
}

async fn fetch(
    client: &reqwest::Client,
    file: &ModelFile,
    target: &Path,
    progress: impl Fn(u64),
) -> AppResult<()> {
    let mut resp = client
        .get(file.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| AppError::Download(e.to_string()))?;
    let part = target.with_extension("part");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&part)?);
    let mut hasher = Sha256::new();
    let mut n = 0u64;
    let mut last = Instant::now();
    while let Some(chunk) = resp.chunk().await.map_err(|e| AppError::Download(e.to_string()))? {
        n += chunk.len() as u64;
        if n > file.size {
            return Err(AppError::ChecksumMismatch(format!(
                "{} is larger than expected",
                file.url
            )));
        }
        hasher.update(&chunk);
        out.write_all(&chunk)?;
        if last.elapsed() >= Duration::from_millis(100) {
            last = Instant::now();
            progress(n);
        }
    }
    out.flush()?;
    drop(out);
    verify(n, &hasher.finalize(), file)?;
    std::fs::rename(&part, target)?;
    Ok(())
}

fn verify(size: u64, digest: &[u8], file: &ModelFile) -> AppResult<()> {
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if size != file.size || hex != file.sha256 {
        return Err(AppError::ChecksumMismatch(format!("{} ({size} bytes, {hex})", file.url)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_checks_size_and_hash() {
        let file = ModelFile {
            role: "config",
            url: "https://huggingface.co/x",
            // sha256("abc")
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            size: 3,
        };
        let digest = Sha256::digest(b"abc");
        assert!(verify(3, &digest, &file).is_ok());
        assert!(verify(4, &digest, &file).is_err());
        assert!(verify(3, &Sha256::digest(b"abd"), &file).is_err());
    }

    #[test]
    fn missing_or_wrong_size_files_are_not_downloaded() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ModelStore::new(tmp.path().to_path_buf());
        assert!(!store.is_downloaded(ModelId::TinyQ));
        let info = ModelId::TinyQ.info();
        std::fs::create_dir_all(store.dir(ModelId::TinyQ)).unwrap();
        for f in info.files {
            std::fs::write(store.path(info, f), b"short").unwrap();
        }
        assert!(!store.is_downloaded(ModelId::TinyQ));
        assert!(matches!(store.load(ModelId::TinyQ), Err(AppError::ModelMissing(_))));
    }
}
