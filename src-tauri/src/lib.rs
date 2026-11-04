//! Talk Typer – Tauri desktop shell.
//!
//! Whisper runs natively (stt-core, candle with Metal on macOS); audio is captured with cpal.
//! The frontend talks to this crate only through the commands in [`commands`] and the events in
//! [`events`]. The only network access is the model download from HuggingFace.

pub mod capture;
pub mod commands;
pub mod devices;
pub mod engine;
pub mod error;
pub mod events;
pub mod model_store;

use engine::Engine;
use model_store::ModelStore;
use tauri::Manager;

pub struct AppState {
    pub models: ModelStore,
    pub engine: Engine,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let models = ModelStore::new(app.path().app_data_dir()?.join("models"));
            app.manage(AppState { models, engine: Engine::default() });
            Ok(())
        })
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
