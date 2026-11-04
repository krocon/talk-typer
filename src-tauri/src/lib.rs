//! Talk Typer – Tauri desktop shell.
//!
//! Whisper runs natively (stt-core, candle with Metal on macOS); audio is captured with cpal.
//! The frontend talks to this crate only through the commands in [`commands`] and the events in
//! [`events`]. The only network access is the model download from HuggingFace.

pub mod commands;
pub mod devices;
pub mod error;
pub mod events;

use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    /// Selected cpal input device id; `None` = system default.
    pub input_device: Mutex<Option<String>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_models,
            commands::download_model,
            commands::load_model,
            commands::start_listening,
            commands::stop_listening,
            commands::list_input_devices,
            commands::set_input_device,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("Talk Typer could not start: {e}");
        std::process::exit(1);
    }
}
