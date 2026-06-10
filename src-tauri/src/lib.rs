// Traceflow library root, v2.

use std::sync::Arc;
use tokio::sync::Mutex;

mod ai;
mod capture;
mod commands;
mod config;
mod document;
mod events;
mod ocr;
mod privacy;
mod replay;
mod rules;
mod state;

#[cfg(test)]
mod integration_tests;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .init();

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
            commands::list_rule_packs,
            commands::save_rule_pack,
            commands::toggle_rule_pack,
            commands::verify_session_chain,
            commands::list_sessions,
            commands::load_session,
            commands::rule_wizard_presets,
            commands::get_session_timeline,
            commands::rule_wizard_presets,
            commands::rule_pattern_test,
            commands::custom_rule_add,
            commands::custom_rule_remove,
            commands::custom_rules_get,
            commands::rule_pack_import,
            commands::rule_pack_export,
        ])
        .setup(|app| {
            log::info!("Traceflow v{} starting", env!("CARGO_PKG_VERSION"));
            let _ = app;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Traceflow");
}
