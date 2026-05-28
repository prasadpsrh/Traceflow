// Shared application state, v2.
//
// State is much thinner now: the event log is the source of truth for what
// happened during a session. State just tracks "what's currently running."

use crate::config::ProjectConfig;
use crate::events::EventLog;
use crate::rules::engine::RuleEngine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// A capture session — one run of "Record → Stop".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub root: PathBuf,
}

#[derive(Clone)]
pub struct ActiveSession {
    pub meta: Session,
    pub log: Arc<EventLog>,
    pub frames_dir: PathBuf,
    /// Input hook thread handle — stopped when the session ends.
    /// Wrapped in Arc<Mutex<Option>> so ActiveSession can be Clone.
    pub input_hook: Arc<std::sync::Mutex<Option<crate::capture::input::InputHookHandle>>>,
}

impl std::fmt::Debug for ActiveSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveSession")
            .field("meta", &self.meta)
            .field("frames_dir", &self.frames_dir)
            .finish()
    }
}

#[derive(Debug)]
pub struct AppState {
    pub config: ProjectConfig,
    /// Most recent session — retained after recording stops so export/verify/edit
    /// keep working without requiring the capture loop to be running.
    /// Replaced (not cleared) when a new session starts.
    pub active: Option<ActiveSession>,
    /// True only while the capture loop is actively running.
    /// Distinct from `active.is_some()` so we can query post-stop sessions.
    pub is_recording: bool,
    /// Set to true to ask the capture loop to exit on its next tick.
    pub capture_stop_flag: bool,
    /// Cached step count so the UI status bar can show it cheaply.
    pub step_count: usize,
    /// Compiled rule engine loaded from config.rules.packs at startup.
    /// None if redact_pii = false or no packs are configured.
    pub rule_engine: Option<Arc<RuleEngine>>,
}

impl AppState {
    pub fn new() -> Self {
        let config = ProjectConfig::load_or_default(&Self::config_path());
        let rule_engine = if config.capture.redact_pii {
            load_rule_engine(&config)
        } else {
            None
        };
        Self {
            config,
            active: None,
            is_recording: false,
            capture_stop_flag: false,
            step_count: 0,
            rule_engine,
        }
    }

    pub fn data_root() -> PathBuf {
        dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Traceflow")
    }

    pub fn config_path() -> PathBuf {
        Self::data_root().join("traceflow.config.toml")
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Load and compile all rule packs listed in `config.rules.packs`.
/// Returns `None` if no packs are configured or all fail to load.
fn load_rule_engine(config: &ProjectConfig) -> Option<Arc<RuleEngine>> {
    if config.rules.packs.is_empty() {
        return None;
    }
    let mut engine = RuleEngine::new();
    let search_dirs = rule_pack_dirs();
    let mut loaded = 0usize;

    for pack_name in &config.rules.packs {
        let path = std::path::Path::new(pack_name);
        let candidates: Vec<std::path::PathBuf> = if path.is_absolute() {
            vec![path.to_path_buf()]
        } else {
            search_dirs.iter().map(|d| d.join(pack_name)).collect()
        };

        let loaded_this = candidates.iter().find(|c| c.exists()).and_then(|p| {
            engine.load_pack_file(p).ok()?;
            Some(())
        });

        if loaded_this.is_some() {
            loaded += 1;
            log::info!("Loaded rule pack: {pack_name}");
        } else {
            log::warn!("Rule pack '{pack_name}' not found in search paths");
        }
    }

    if loaded > 0 {
        log::info!("Rule engine ready: {} rules across {loaded} packs", engine.rule_count());
        Some(Arc::new(engine))
    } else {
        None
    }
}

fn rule_pack_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("rule_packs"));
            if let Some(gp) = parent.parent() {
                dirs.push(gp.join("rule_packs"));
            }
        }
    }
    dirs.push(std::path::PathBuf::from("src-tauri/rule_packs"));
    dirs.push(std::path::PathBuf::from("rule_packs"));
    dirs
}
