//! Transcript export: the only way text leaves the app, and only on explicit user action.
//! The frontend sends the text and a suggested name; Rust shows the native save dialog and writes
//! exactly to the path the user picked. The webview has no file-system or dialog permissions.
//! Name sanitising taken from sonic-lab (`src-tauri/src/export.rs`).

use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// Upper bound for one export (a transcript is text – generous headroom).
pub const MAX_BYTES: usize = 32 * 1024 * 1024;

/// Keeps a safe file name: no directories, no control or reserved characters, sensible length.
pub fn sanitize_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    let result: String = trimmed.chars().take(120).collect();
    if result.is_empty() { "transcript.txt".into() } else { result }
}

fn filter_for(name: &str) -> Option<(&'static str, &'static [&'static str])> {
    match Path::new(name).extension().and_then(|e| e.to_str()) {
        Some("txt") => Some(("Text", &["txt"])),
        Some("srt") => Some(("SubRip-Untertitel", &["srt"])),
        _ => None,
    }
}

/// Shows the save dialog and writes the file (blocking). `None` = cancelled.
pub fn save_text(app: &AppHandle, suggested: &str, contents: &str) -> AppResult<Option<String>> {
    if contents.len() > MAX_BYTES {
        return Err(AppError::Io("export too large".into()));
    }
    let name = sanitize_file_name(suggested);
    let mut dialog = app.dialog().file().set_file_name(&name);
    if let Some((label, extensions)) = filter_for(&name) {
        dialog = dialog.add_filter(label, extensions);
    }
    let Some(choice) = dialog.blocking_save_file() else {
        return Ok(None);
    };
    let path: PathBuf = choice.into_path().map_err(|e| AppError::Io(e.to_string()))?;
    std::fs::write(&path, contents)?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_sanitised() {
        assert_eq!(sanitize_file_name("talk-typer-2026.srt"), "talk-typer-2026.srt");
        assert_eq!(sanitize_file_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_file_name("C:\\x\\a:b?.txt"), "a_b_.txt");
        assert_eq!(sanitize_file_name(" .. "), "transcript.txt");
        assert_eq!(sanitize_file_name("Notiz äöü 🎙.txt"), "Notiz äöü 🎙.txt");
    }

    #[test]
    fn filters_follow_the_extension() {
        assert_eq!(filter_for("a.srt").map(|f| f.1), Some(&["srt"][..]));
        assert!(filter_for("a.wav").is_none());
    }
}
