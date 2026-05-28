// Tauri IPC commands, v2.
//
// The command surface stays roughly the same as v1 so the React frontend
// barely changes — but under the hood, writes go to the event log and reads
// project from it.

use crate::capture::{engine, monitor};
use crate::config::{CaptureCfg, ProjectConfig};
use crate::document::{render_to_file, RenderRequest, StepView};
use crate::events::{event::SessionPaths, log as event_log, EventKind, EventLog};
use crate::state::{ActiveSession, AppState, Session};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use uuid::Uuid;

pub type SharedState = Arc<Mutex<AppState>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

#[tauri::command]
pub async fn list_monitors() -> Result<Vec<MonitorInfo>, String> {
    monitor::list_monitors().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_capture(
    title: String,
    state: State<'_, SharedState>,
    app: tauri::AppHandle,
) -> Result<Uuid, String> {
    // Set up the session: directories + fresh event log.
    let session_id = {
        let mut guard = state.lock().await;
        if guard.active.is_some() {
            return Err("a capture session is already active".into());
        }
        let id = Uuid::new_v4();
        let root = AppState::data_root().join("sessions").join(id.to_string());
        let paths = SessionPaths::under(root.clone());
        std::fs::create_dir_all(&paths.frames_dir).map_err(|e| e.to_string())?;

        let log = EventLog::create(&paths.events_log, id).map_err(|e| e.to_string())?;
        // First event: SessionStart with the resolved capture settings.
        let settings_json = serde_json::to_value(&guard.config.capture)
            .map_err(|e| e.to_string())?;
        log.append(EventKind::SessionStart {
            title: title.clone(),
            capture_settings: settings_json,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            host_os: std::env::consts::OS.to_string(),
        })
        .map_err(|e| e.to_string())?;

        let now = chrono::Utc::now();
        let meta = Session {
            id,
            title,
            started_at: now,
            ended_at: None,
            root,
        };
        guard.active = Some(ActiveSession {
            meta,
            log: Arc::new(log),
            frames_dir: paths.frames_dir,
        });
        guard.capture_stop_flag = false;
        guard.step_count = 0;
        id
    };

    // Launch the capture loop on a background task.
    let state_clone: SharedState = (*state.inner()).clone();
    let app_handle = app.clone();
    tokio::spawn(async move {
        if let Err(e) = engine::run_capture_loop(state_clone, app_handle).await {
            log::error!("capture loop failed: {e}");
        }
    });

    Ok(session_id)
}

#[tauri::command]
pub async fn stop_capture(state: State<'_, SharedState>) -> Result<Session, String> {
    let mut guard = state.lock().await;
    guard.capture_stop_flag = true;
    let active = guard
        .active
        .take()
        .ok_or_else(|| "no active session".to_string())?;
    // Emit SessionEnd
    active
        .log
        .append(EventKind::SessionEnd {
            reason: "user_stop".into(),
        })
        .map_err(|e| e.to_string())?;
    let mut meta = active.meta;
    meta.ended_at = Some(chrono::Utc::now());
    Ok(meta)
}

#[tauri::command]
pub async fn get_session_steps(
    state: State<'_, SharedState>,
) -> Result<Vec<StepView>, String> {
    let guard = state.lock().await;
    if let Some(active) = &guard.active {
        let log_path = active.log.path().to_path_buf();
        let frames_dir = active.frames_dir.clone();
        drop(guard);
        let recs = event_log::read_all(&log_path).map_err(|e| e.to_string())?;
        // Project: reuse the same projection used by render_to_file.
        Ok(crate::document::render::project_steps_for_ui(&recs, &frames_dir))
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn delete_step(
    step_index: usize,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let guard = state.lock().await;
    if let Some(active) = &guard.active {
        active
            .log
            .append(EventKind::StepDeleted { step_index })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn update_step_description(
    step_index: usize,
    description: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let guard = state.lock().await;
    if let Some(active) = &guard.active {
        active
            .log
            .append(EventKind::DescriptionEdited {
                step_index,
                text: description,
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub output_path: PathBuf,
    pub title: String,
    pub author: Option<String>,
}

#[tauri::command]
pub async fn export_document(
    req: ExportRequest,
    state: State<'_, SharedState>,
) -> Result<PathBuf, String> {
    let (events_log, frames_dir, cfg) = {
        let guard = state.lock().await;
        let active = guard.active.as_ref().ok_or("no active session")?;
        (
            active.log.path().to_path_buf(),
            active.frames_dir.clone(),
            guard.config.clone(),
        )
    };
    let render_req = RenderRequest {
        events_log,
        frames_dir,
        output_path: req.output_path,
        title: req.title,
        author: req.author,
    };
    render_to_file(&render_req, &cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, SharedState>) -> Result<CaptureCfg, String> {
    let guard = state.lock().await;
    Ok(guard.config.capture.clone())
}

#[tauri::command]
pub async fn update_settings(
    settings: CaptureCfg,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut guard = state.lock().await;
    guard.config.capture = settings;
    // Persist
    let _ = guard.config.write(&AppState::config_path());
    Ok(())
}

#[tauri::command]
pub async fn get_config(state: State<'_, SharedState>) -> Result<ProjectConfig, String> {
    let guard = state.lock().await;
    Ok(guard.config.clone())
}

#[tauri::command]
pub async fn verify_session_chain(
    state: State<'_, SharedState>,
) -> Result<VerifyReport, String> {
    let guard = state.lock().await;
    let active = guard.active.as_ref().ok_or("no active session")?;
    let log_path = active.log.path().to_path_buf();
    drop(guard);
    let recs = event_log::read_all(&log_path).map_err(|e| e.to_string())?;
    match crate::events::chain::verify_chain(recs.iter()) {
        Ok(n) => Ok(VerifyReport {
            ok: true,
            verified_events: n,
            message: format!("chain intact — {n} events verified"),
        }),
        Err(e) => Ok(VerifyReport {
            ok: false,
            verified_events: 0,
            message: e.to_string(),
        }),
    }
}

#[derive(Debug, Serialize)]
pub struct VerifyReport {
    pub ok: bool,
    pub verified_events: u64,
    pub message: String,
}
