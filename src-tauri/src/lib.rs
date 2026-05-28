// Traceflow library root, v2.

use std::sync::Arc;
use tokio::sync::Mutex;

mod ai;
mod capture;
mod commands;
mod config;
mod document;
mod events;
mod privacy;
mod rules;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let shared_state = Arc::new(Mutex::new(AppState::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(shared_state)
        .invoke_handler(tauri::generate_handler![
            commands::list_monitors,
            commands::start_capture,
            commands::stop_capture,
            commands::get_session_steps,
            commands::delete_step,
            commands::update_step_description,
            commands::export_document,
            commands::get_settings,
            commands::update_settings,
            commands::get_config,
            commands::verify_session_chain,
        ])
        .setup(|app| {
            log::info!("Traceflow v{} starting", env!("CARGO_PKG_VERSION"));
            let _ = app;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Traceflow");
}
