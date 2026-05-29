// The event schema.
//
// Events are deliberately **generic**: they describe what happened in
// neutral terms, not what it *means* in any particular industry. Downstream
// renderers and rule packs decide what's important.
//
// The schema is forward-compatible: unknown variants in an older binary
// will be skipped, not crashed on.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// One event in the log. Always wrapped in `EventRecord` when written.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    /// Session started.
    SessionStart {
        title: String,
        capture_settings: serde_json::Value,
        app_version: String,
        host_os: String,
    },

    /// A frame was promoted to a step (passed change-detection + stability).
    StepPromoted {
        step_index: usize,
        /// SHA-256 of the PNG bytes; the file lives at `frames/{hash}.png`.
        frame_hash: String,
        width: u32,
        height: u32,
        window_title: Option<String>,
        window_class: Option<String>,
        app_name: Option<String>,
    },

    /// A frame was captured but not promoted (kept for forensics, optional).
    FrameSampled {
        frame_hash: Option<String>,
        diff_score: f32,
    },

    /// The OS reported a window-focus change.
    WindowFocusChanged {
        window_title: Option<String>,
        window_class: Option<String>,
        app_name: Option<String>,
    },

    /// Mouse click event (when input hooks are wired up — phase 2).
    MouseClick { x: i32, y: i32, button: String },

    /// Keyboard event (sanitized — never raw keystrokes for password fields).
    KeyboardInput { virtual_key: String },

    /// OCR pass produced text for a captured frame.
    OcrResult {
        frame_hash: String,
        text_length: usize,
        /// Bounding boxes of detected text regions (x, y, w, h, text-hash).
        regions: Vec<OcrRegion>,
    },

    /// Redaction was applied to a frame or to text.
    RedactionApplied {
        frame_hash: Option<String>,
        rule_name: String,
        match_count: usize,
        action: String, // "blur" | "black" | "drop"
    },

    /// AI-generated description for a step.
    AiDescription {
        step_index: usize,
        text: String,
        model: String,
        language: String,
    },

    /// User edited a step description in the UI.
    DescriptionEdited { step_index: usize, text: String },

    /// User deleted a step.
    StepDeleted { step_index: usize },

    /// Session ended.
    SessionEnd { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRegion {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub text_hash: String,
}

/// The actual record written to disk. Wraps an `EventKind` with chain metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    /// Monotonic sequence number within the session, starting at 0.
    pub seq: u64,
    /// Wall-clock time the event was recorded.
    pub at: DateTime<Utc>,
    /// Session this event belongs to.
    pub session: Uuid,
    /// Hex SHA-256 of the previous record (all-zeros for seq=0).
    pub prev: String,
    /// Hex SHA-256 of THIS record's canonical body (excluding `hash` itself).
    pub hash: String,
    /// The payload.
    pub body: EventKind,
}

/// Location on disk for a session's data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPaths {
    pub root: PathBuf,
    pub events_log: PathBuf,
    pub frames_dir: PathBuf,
    pub manifest: PathBuf,
}

impl SessionPaths {
    pub fn under(root: PathBuf) -> Self {
        let events_log = root.join("events.ndjson");
        let frames_dir = root.join("frames");
        let manifest = root.join("manifest.json");
        Self {
            root,
            events_log,
            frames_dir,
            manifest,
        }
    }
}
