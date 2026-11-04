//! Minimal Whisper tokenizer: decoding only, read from a HuggingFace `tokenizer.json`.
//!
//! Whisper needs just two operations: token id → text (byte-level BPE, GPT-2 byte mapping) and
//! looking up special tokens by name (`<|de|>`, `<|transcribe|>`, …). Encoding text is never
//! needed, so the heavy `tokenizers` crate is not used (design decision, phase 2).

use crate::{SttError, SttResult};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct TokenizerJson {
    model: BpeModel,
    #[serde(default)]
    added_tokens: Vec<AddedToken>,
}

#[derive(Deserialize)]
struct BpeModel {
    vocab: HashMap<String, u32>,
}

#[derive(Deserialize)]
struct AddedToken {
    id: u32,
    content: String,
}

pub struct Tokenizer {
    /// id → raw bytes of the token (already mapped back from the GPT-2 byte alphabet).
    pieces: Vec<Option<Vec<u8>>>,
    /// Special/added tokens by content (`<|endoftext|>` …).
    special: HashMap<String, u32>,
    /// Ids of special tokens; skipped when decoding text.
    special_ids: Vec<bool>,
}

impl Tokenizer {
    pub fn from_json(bytes: &[u8]) -> SttResult<Self> {
        let json: TokenizerJson =
            serde_json::from_slice(bytes).map_err(|e| SttError::InvalidTokenizer(e.to_string()))?;
        let byte_of = unicode_to_byte();
        let max_id = json
            .model
            .vocab
            .values()
            .chain(json.added_tokens.iter().map(|t| &t.id))
            .copied()
            .max()
            .ok_or_else(|| SttError::InvalidTokenizer("empty vocabulary".into()))?;
        if max_id > 1_000_000 {
            return Err(SttError::InvalidTokenizer(format!("token id {max_id} too large")));
        }
        let len = max_id as usize + 1;
        let mut pieces = vec![None; len];
        for (token, &id) in &json.model.vocab {
            let bytes: Option<Vec<u8>> = token.chars().map(|c| byte_of.get(&c).copied()).collect();
            pieces[id as usize] = Some(bytes.unwrap_or_else(|| token.as_bytes().to_vec()));
        }
        let mut special = HashMap::new();
        let mut special_ids = vec![false; len];
        for t in json.added_tokens {
            special_ids[t.id as usize] = true;
            special.insert(t.content, t.id);
        }
        Ok(Self { pieces, special, special_ids })
    }

    /// Id of a special token such as `<|startoftranscript|>`.
    pub fn token_id(&self, token: &str) -> SttResult<u32> {
        self.special
            .get(token)
            .copied()
            .ok_or_else(|| SttError::InvalidTokenizer(format!("missing token {token}")))
    }

    /// Decodes text tokens; special tokens and unknown ids are skipped.
    pub fn decode(&self, ids: &[u32]) -> String {
        let mut bytes = Vec::new();
        for &id in ids {
            let i = id as usize;
            if self.special_ids.get(i).copied().unwrap_or(true) {
                continue;
            }
            if let Some(Some(piece)) = self.pieces.get(i) {
                bytes.extend_from_slice(piece);
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// Inverse of GPT-2's `bytes_to_unicode`: printable bytes map to themselves, the others to
/// U+0100 + n in byte order.
fn unicode_to_byte() -> HashMap<char, u8> {
    let mut map = HashMap::with_capacity(256);
    let mut n = 0u32;
    for b in 0u8..=255 {
        let printable = matches!(b, b'!'..=b'~' | 0xA1..=0xAC | 0xAE..=0xFF);
        let c = if printable {
            char::from(b)
        } else {
            let c = char::from_u32(256 + n).unwrap_or('\u{FFFD}');
            n += 1;
            c
        };
        map.insert(c, b);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
        "model": { "type": "BPE", "vocab": { "Hallo": 0, "Ġdu": 1, "Ã¤": 2, "!": 3 } },
        "added_tokens": [
            { "id": 4, "content": "<|endoftext|>", "special": true },
            { "id": 5, "content": "<|de|>", "special": true }
        ]
    }"#;

    #[test]
    fn decodes_byte_level_tokens() {
        let t = Tokenizer::from_json(JSON.as_bytes()).unwrap();
        // "Ġ" = space, "Ã¤" = UTF-8 bytes of "ä"
        assert_eq!(t.decode(&[0, 1, 2, 3]), "Hallo duä!");
    }

    #[test]
    fn skips_special_tokens_and_finds_them_by_name() {
        let t = Tokenizer::from_json(JSON.as_bytes()).unwrap();
        assert_eq!(t.decode(&[5, 0, 4]), "Hallo");
        assert_eq!(t.token_id("<|de|>").unwrap(), 5);
        assert!(t.token_id("<|xx|>").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(Tokenizer::from_json(b"{}").is_err());
        assert!(Tokenizer::from_json(b"not json").is_err());
    }

    #[test]
    fn byte_mapping_is_a_bijection() {
        let m = unicode_to_byte();
        assert_eq!(m.len(), 256);
        assert_eq!(m.get(&'Ġ'), Some(&b' '));
        assert_eq!(m.get(&'Ċ'), Some(&b'\n'));
    }
}
