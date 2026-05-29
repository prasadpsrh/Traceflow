// Tauri IPC commands, v2.
//
// The command surface stays roughly the same as v1 so the React frontend
// barely changes — but under the hood, writes go to the event log and reads
// project from it.

use crate::capture::{engine, monitor};
use crate::config::{CaptureCfg, ProjectConfig};
use crate::document::{render_to_file, RenderRequest, StepView};
use crate::events::{event::SessionPaths, log as event_log, EventKind, EventLog};
use crate::rules::engine::RulePack;
use crate::state::{ActiveSession, AppState, Session};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
        if guard.is_recording {
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
            input_hook: Arc::new(std::sync::Mutex::new(None)),
        });
        guard.capture_stop_flag = false;
        guard.is_recording = true;
        guard.step_count = 0;
        id
    };

    // Start input hooks (mouse + keyboard) on a dedicated OS thread.
    {
        let guard = state.lock().await;
        let state_for_hooks: SharedState = (*state.inner()).clone();
        let hook = crate::capture::input::start_input_hooks(state_for_hooks);
        if let Some(active) = &guard.active {
            if let Ok(mut slot) = active.input_hook.lock() {
                *slot = Some(hook);
            }
        }
    }

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
    guard.is_recording = false;

    // Append SessionEnd — borrow ends before the mutable update below.
    {
        let active = guard
            .active
            .as_ref()
            .ok_or_else(|| "no active session".to_string())?;
        active
            .log
            .append(EventKind::SessionEnd {
                reason: "user_stop".into(),
            })
            .map_err(|e| e.to_string())?;
    }

    // Stop input hooks.
    if let Some(active) = &guard.active {
        if let Ok(mut slot) = active.input_hook.lock() {
            if let Some(hook) = slot.take() {
                hook.stop();
            }
        }
    }

    // Stamp ended_at and return metadata.
    // active stays in state so export/verify/edit keep working after stop.
    let ended_at = chrono::Utc::now();
    let active = guard.active.as_mut().unwrap();
    active.meta.ended_at = Some(ended_at);
    Ok(active.meta.clone())
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
    guard
        .config
        .write(&AppState::config_path())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_config(state: State<'_, SharedState>) -> Result<ProjectConfig, String> {
    let guard = state.lock().await;
    Ok(guard.config.clone())
}

#[derive(Debug, Serialize)]
pub struct RulePackSummary {
    pub name: String,
    pub version: String,
    pub description: String,
    pub path: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct SaveRulePackRequest {
    pub pack: RulePack,
    pub filename: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToggleRulePackRequest {
    pub path: String,
    pub enabled: bool,
}

fn rule_pack_config_key(path: &std::path::Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Some(file_name) = canonical.file_name().and_then(|f| f.to_str()) {
        return file_name.to_string();
    }
    canonical.to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn toggle_rule_pack(
    req: ToggleRulePackRequest,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut guard = state.lock().await;
    let path = std::path::Path::new(&req.path);
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let file_name = canonical
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or_default()
        .to_string();
    let config_key = rule_pack_config_key(&canonical);

    if req.enabled {
        if !guard.config.rules.packs.contains(&config_key) {
            guard.config.rules.packs.push(config_key.clone());
        }
    } else {
        let canonical_str = canonical.to_string_lossy();
        guard.config.rules.packs.retain(|p| {
            p != &config_key && p != &file_name && p != canonical_str.as_ref()
        });
    }

    guard
        .config
        .write(&AppState::config_path())
        .map_err(|e| e.to_string())?;
    guard.rule_engine = if guard.config.capture.redact_pii {
        load_rule_engine(&guard.config)
    } else {
        None
    };
    Ok(())
}

#[tauri::command]
pub async fn list_rule_packs(
    state: State<'_, SharedState>,
) -> Result<Vec<RulePackSummary>, String> {
    let guard = state.lock().await;
    let enabled_packs = guard.config.rules.packs.clone();
    drop(guard);

    let mut summaries = Vec::new();
    let mut seen_paths = HashSet::new();

    for dir in crate::state::rule_pack_dirs() {
        if !dir.exists() {
            continue;
        }
        let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let canonical = path.canonicalize().unwrap_or(path.clone());
            if !seen_paths.insert(canonical.clone()) {
                continue;
            }
            let bytes = std::fs::read(&canonical).map_err(|e| e.to_string())?;
            let pack: RulePack = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let file_name = canonical
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or_default()
                .to_string();
            let canonical_str = canonical.to_string_lossy().to_string();
            let enabled = enabled_packs
                .iter()
                .any(|p| p == &file_name || p == &canonical_str);
            summaries.push(RulePackSummary {
                name: pack.name,
                version: pack.version,
                description: pack.description,
                path: canonical.to_string_lossy().into_owned(),
                enabled,
            });
        }
    }

    Ok(summaries)
}

#[tauri::command]
pub async fn save_rule_pack(
    req: SaveRulePackRequest,
    state: State<'_, SharedState>,
) -> Result<String, String> {
    let mut guard = state.lock().await;
    let dir = crate::state::user_rule_pack_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let filename = if let Some(name) = req.filename.clone() {
        name
    } else {
        format!("{}-{}.json", req.pack.name, req.pack.version)
    };

    let path = dir.join(&filename);
    let json = serde_json::to_string_pretty(&req.pack).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;

    if !guard.config.rules.packs.contains(&filename) {
        guard.config.rules.packs.push(filename.clone());
        guard
            .config
            .write(&AppState::config_path())
            .map_err(|e| e.to_string())?;
    }

    Ok(path.to_string_lossy().into_owned())
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

/// A brief summary of a past session — enough for the history panel.
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub step_count: usize,
    pub root: PathBuf,
}

#[tauri::command]
pub async fn list_sessions() -> Result<Vec<SessionSummary>, String> {
    let sessions_dir = AppState::data_root().join("sessions");
    if !sessions_dir.exists() {
        return Ok(vec![]);
    }

    let mut summaries: Vec<SessionSummary> = Vec::new();

    let entries = std::fs::read_dir(&sessions_dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let log_path = entry.path().join("events.ndjson");
        if !log_path.exists() {
            continue;
        }
        let records = match event_log::read_all(&log_path) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let mut title = "Untitled".to_string();
        let mut started_at = String::new();
        let mut ended_at: Option<String> = None;
        let mut step_count = 0usize;
        let mut session_id = String::new();

        for r in &records {
            session_id = r.session.to_string();
            match &r.body {
                crate::events::EventKind::SessionStart { title: t, .. } => {
                    title = t.clone();
                    started_at = r.at.to_rfc3339();
                }
                crate::events::EventKind::SessionEnd { .. } => {
                    ended_at = Some(r.at.to_rfc3339());
                }
                crate::events::EventKind::StepPromoted { .. } => {
                    step_count += 1;
                }
                _ => {}
            }
        }

        if session_id.is_empty() {
            continue;
        }
        summaries.push(SessionSummary {
            id: session_id,
            title,
            started_at,
            ended_at,
            step_count,
            root: entry.path(),
        });
    }

    // Most-recent first.
    summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(summaries)
}

/// Load a past session so it becomes the current "active" for export/verify.
#[tauri::command]
pub async fn load_session(
    root: PathBuf,
    state: State<'_, SharedState>,
) -> Result<SessionSummary, String> {
    let log_path = root.join("events.ndjson");
    if !log_path.exists() {
        return Err(format!("events.ndjson not found in {}", root.display()));
    }
    let records = event_log::read_all(&log_path).map_err(|e| e.to_string())?;

    let mut title = "Untitled".to_string();
    let mut started_at = String::new();
    let mut ended_at: Option<String> = None;
    let mut step_count = 0usize;
    let mut session_id_str = String::new();

    for r in &records {
        session_id_str = r.session.to_string();
        match &r.body {
            crate::events::EventKind::SessionStart { title: t, .. } => {
                title = t.clone();
                started_at = r.at.to_rfc3339();
            }
            crate::events::EventKind::SessionEnd { .. } => {
                ended_at = Some(r.at.to_rfc3339());
            }
            crate::events::EventKind::StepPromoted { .. } => step_count += 1,
            _ => {}
        }
    }

    // Open the log for appending (allows post-load edits/deletions).
    let log = crate::events::log::EventLog::open_for_append(&log_path)
        .map_err(|e| e.to_string())?;

    let frames_dir = root.join("frames");
    let session_id: uuid::Uuid = session_id_str.parse().map_err(|e: uuid::Error| e.to_string())?;

    let meta = crate::state::Session {
        id: session_id,
        title: title.clone(),
        started_at: records
            .first()
            .map(|r| r.at)
            .unwrap_or_else(chrono::Utc::now),
        ended_at: records.last().and_then(|r| {
            if matches!(&r.body, crate::events::EventKind::SessionEnd { .. }) {
                Some(r.at)
            } else {
                None
            }
        }),
        root: root.clone(),
    };

    let mut guard = state.lock().await;
    guard.active = Some(ActiveSession {
        meta,
        log: Arc::new(log),
        frames_dir,
        input_hook: Arc::new(std::sync::Mutex::new(None)),
    });

    Ok(SessionSummary {
        id: session_id_str,
        title,
        started_at,
        ended_at,
        step_count,
        root,
    })
}
