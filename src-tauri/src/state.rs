// Shared application state, v2.
//
// State is much thinner now: the event log is the source of truth for what
// happened during a session. State just tracks "what's currently running."

use crate::config::ProjectConfig;
use crate::events::EventLog;
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
    pub active: Option<ActiveSession>,
    /// Set to true to ask the capture loop to stop.
    pub capture_stop_flag: bool,
    /// Cached step count so the UI status bar can show it cheaply.
    pub step_count: usize,
}

impl AppState {
    pub fn new() -> Self {
        let config = ProjectConfig::load_or_default(&Self::config_path());
        Self {
            config,
            active: None,
            capture_stop_flag: false,
            step_count: 0,
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
