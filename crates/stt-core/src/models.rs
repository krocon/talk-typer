//! Catalogue of the supported (multilingual) Whisper models.
//!
//! Only data – downloading and caching are done by the hosts (Tauri: `reqwest` + app data dir,
//! browser: Cache Storage). Sizes are approximate and only for display.

use serde::Serialize;

/// Model identifier as used across Rust and TypeScript (`"base"`, `"tiny-q"`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelId {
    Tiny,
    TinyQ,
    Base,
    BaseQ,
    Small,
    SmallQ,
}

impl ModelId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::TinyQ => "tiny-q",
            Self::Base => "base",
            Self::BaseQ => "base-q",
            Self::Small => "small",
            Self::SmallQ => "small-q",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        MODELS.iter().map(|m| m.id).find(|id| id.as_str() == s)
    }

    pub fn info(self) -> &'static ModelInfo {
        // MODELS lists every variant exactly once (test `catalogue_is_complete`).
        let idx = MODELS.iter().position(|m| m.id == self).unwrap_or(0);
        &MODELS[idx]
    }
}

/// A file the model needs.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFile {
    /// Logical role: `weights`, `config` or `tokenizer`.
    pub role: &'static str,
    /// Full download URL (HuggingFace).
    pub url: &'static str,
    /// Expected SHA-256 (hex) – a download that does not match is discarded.
    pub sha256: &'static str,
    /// Exact size in bytes (progress when the server sends no length; sanity check).
    pub size: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: ModelId,
    pub label: &'static str,
    pub quantized: bool,
    /// Approximate download size in MB.
    pub size_mb: u32,
    pub mel_bins: usize,
    pub files: &'static [ModelFile],
}

const HF: &str = "https://huggingface.co";

/// Official OpenAI weights, pinned to a commit.
macro_rules! openai {
    ($size:literal, $rev:literal, $file:literal) => {
        concat!("https://huggingface.co/openai/whisper-", $size, "/resolve/", $rev, "/", $file)
    };
}

/// Q8_0 GGUF made from the OpenAI weights with `examples/quantize.rs` (reproducible, see
/// `docs/models.md`) and hosted in the project's own HuggingFace repository.
macro_rules! talk_typer {
    ($file:literal) => {
        concat!("https://huggingface.co/krocon/talk-typer-models/resolve/main/", $file)
    };
}

/// Supported models: safetensors from `openai/whisper-*`; quantized variants = own GGUF +
/// OpenAI's `config.json`/`tokenizer.json`. Labels are shown in the UI.
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: ModelId::Tiny,
        label: "Tiny",
        quantized: false,
        size_mb: 154,
        mel_bins: 80,
        files: &[
            ModelFile {
                role: "weights",
                url: openai!(
                    "tiny",
                    "169d4a4341b33bc18d8881c4b69c2e104e1cc0af",
                    "model.safetensors"
                ),
                sha256: "7ebd0e69e78190ffe1438491fa05cc1f5c1aa3a4c4db3bc1723adbb551ea2395",
                size: 151061672,
            },
            ModelFile {
                role: "config",
                url: openai!("tiny", "169d4a4341b33bc18d8881c4b69c2e104e1cc0af", "config.json"),
                sha256: "ffdccec4f3211f4c63310f2b7098f309fe70f3952cedc5e4d11e43f5b2379b98",
                size: 1983,
            },
            ModelFile {
                role: "tokenizer",
                url: openai!("tiny", "169d4a4341b33bc18d8881c4b69c2e104e1cc0af", "tokenizer.json"),
                sha256: "27fc476bfe7f17299480be2273fc0608e4d5a99aba2ab5dec5374b4482d1a566",
                size: 2480466,
            },
        ],
    },
    ModelInfo {
        id: ModelId::TinyQ,
        label: "Tiny (quantisiert)",
        quantized: true,
        size_mb: 44,
        mel_bins: 80,
        files: &[
            ModelFile {
                role: "weights",
                url: talk_typer!("model-tiny-q80.gguf"),
                sha256: "0ea766dc1f27360f9910bc85091ceaec19e60691437c21559a2aed07b778b462",
                size: 41841632,
            },
            ModelFile {
                role: "config",
                url: openai!("tiny", "169d4a4341b33bc18d8881c4b69c2e104e1cc0af", "config.json"),
                sha256: "ffdccec4f3211f4c63310f2b7098f309fe70f3952cedc5e4d11e43f5b2379b98",
                size: 1983,
            },
            ModelFile {
                role: "tokenizer",
                url: openai!("tiny", "169d4a4341b33bc18d8881c4b69c2e104e1cc0af", "tokenizer.json"),
                sha256: "27fc476bfe7f17299480be2273fc0608e4d5a99aba2ab5dec5374b4482d1a566",
                size: 2480466,
            },
        ],
    },
    ModelInfo {
        id: ModelId::Base,
        label: "Base",
        quantized: false,
        size_mb: 293,
        mel_bins: 80,
        files: &[
            ModelFile {
                role: "weights",
                url: openai!(
                    "base",
                    "e37978b90ca9030d5170a5c07aadb050351a65bb",
                    "model.safetensors"
                ),
                sha256: "07cadb9f25677c8d50df603e66a98fbd842cce45047139baeb16e6219a1e807b",
                size: 290403936,
            },
            ModelFile {
                role: "config",
                url: openai!("base", "e37978b90ca9030d5170a5c07aadb050351a65bb", "config.json"),
                sha256: "a153c53883a6799b6f056b4a8d1a515c9926d03994682ba88a7616618d7da0c1",
                size: 1983,
            },
            ModelFile {
                role: "tokenizer",
                url: openai!("base", "e37978b90ca9030d5170a5c07aadb050351a65bb", "tokenizer.json"),
                sha256: "27fc476bfe7f17299480be2273fc0608e4d5a99aba2ab5dec5374b4482d1a566",
                size: 2480466,
            },
        ],
    },
    ModelInfo {
        id: ModelId::BaseQ,
        label: "Base (quantisiert)",
        quantized: true,
        size_mb: 83,
        mel_bins: 80,
        files: &[
            ModelFile {
                role: "weights",
                url: talk_typer!("model-base-q80.gguf"),
                sha256: "4cebd9ca1fc5774919e92ce92ac24b8e75e9ed1a2f35522bf85ee59e5f4364b7",
                size: 80092192,
            },
            ModelFile {
                role: "config",
                url: openai!("base", "e37978b90ca9030d5170a5c07aadb050351a65bb", "config.json"),
                sha256: "a153c53883a6799b6f056b4a8d1a515c9926d03994682ba88a7616618d7da0c1",
                size: 1983,
            },
            ModelFile {
                role: "tokenizer",
                url: openai!("base", "e37978b90ca9030d5170a5c07aadb050351a65bb", "tokenizer.json"),
                sha256: "27fc476bfe7f17299480be2273fc0608e4d5a99aba2ab5dec5374b4482d1a566",
                size: 2480466,
            },
        ],
    },
    ModelInfo {
        id: ModelId::Small,
        label: "Small",
        quantized: false,
        size_mb: 969,
        mel_bins: 80,
        files: &[
            ModelFile {
                role: "weights",
                url: openai!(
                    "small",
                    "973afd24965f72e36ca33b3055d56a652f456b4d",
                    "model.safetensors"
                ),
                sha256: "1d7734884874f1a1513ed9aa760a4f8e97aaa02fd6d93a3a85d27b2ae9ca596b",
                size: 966995080,
            },
            ModelFile {
                role: "config",
                url: openai!("small", "973afd24965f72e36ca33b3055d56a652f456b4d", "config.json"),
                sha256: "e6a2b489da1b5aed65a8eb8d1e7466fa867ad5643a8bc138ba708bd56b2875c4",
                size: 1967,
            },
            ModelFile {
                role: "tokenizer",
                url: openai!("small", "973afd24965f72e36ca33b3055d56a652f456b4d", "tokenizer.json"),
                sha256: "27fc476bfe7f17299480be2273fc0608e4d5a99aba2ab5dec5374b4482d1a566",
                size: 2480466,
            },
        ],
    },
    ModelInfo {
        id: ModelId::SmallQ,
        label: "Small (quantisiert)",
        quantized: true,
        size_mb: 266,
        mel_bins: 80,
        files: &[
            ModelFile {
                role: "weights",
                url: talk_typer!("model-small-q80.gguf"),
                sha256: "38394e67d1e577634ad2bd4be9c3e7e0d81db0d7a7a854bc880d5cced7b05781",
                size: 263419200,
            },
            ModelFile {
                role: "config",
                url: openai!("small", "973afd24965f72e36ca33b3055d56a652f456b4d", "config.json"),
                sha256: "e6a2b489da1b5aed65a8eb8d1e7466fa867ad5643a8bc138ba708bd56b2875c4",
                size: 1967,
            },
            ModelFile {
                role: "tokenizer",
                url: openai!("small", "973afd24965f72e36ca33b3055d56a652f456b4d", "tokenizer.json"),
                sha256: "27fc476bfe7f17299480be2273fc0608e4d5a99aba2ab5dec5374b4482d1a566",
                size: 2480466,
            },
        ],
    },
];

/// Base URL of the model host – the only network destination of the app.
pub const MODEL_HOST: &str = HF;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_complete() {
        for id in [
            ModelId::Tiny,
            ModelId::TinyQ,
            ModelId::Base,
            ModelId::BaseQ,
            ModelId::Small,
            ModelId::SmallQ,
        ] {
            assert_eq!(MODELS.iter().filter(|m| m.id == id).count(), 1, "{id:?}");
            assert_eq!(ModelId::parse(id.as_str()), Some(id));
            assert_eq!(id.info().id, id);
        }
    }

    #[test]
    fn all_urls_point_to_the_model_host() {
        for m in MODELS {
            for f in m.files {
                assert!(f.url.starts_with(MODEL_HOST), "{}", f.url);
            }
        }
    }

    #[test]
    fn every_file_is_pinned() {
        for m in MODELS {
            assert_eq!(m.files.len(), 3);
            for f in m.files {
                assert_eq!(f.sha256.len(), 64, "{}", f.url);
                assert!(f.sha256.bytes().all(|b| b.is_ascii_hexdigit()), "{}", f.url);
                assert!(f.size > 0, "{}", f.url);
                assert!(
                    !f.url.contains("/resolve/main/") || f.url.contains("krocon/"),
                    "{}",
                    f.url
                );
            }
            let total: u64 = m.files.iter().map(|f| f.size).sum();
            assert_eq!(m.size_mb as u64, (total + 500_000) / 1_000_000, "{:?}", m.id);
        }
    }
}
