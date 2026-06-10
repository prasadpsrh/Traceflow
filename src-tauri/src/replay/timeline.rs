//! Timeline index for session replay.

use crate::events::{event::EventKind, log as event_log};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A single entry in the timeline — one promoted step with its associated events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub timestamp_ms: i64,
    pub timestamp: DateTime<Utc>,
    pub step_index: usize,
    pub frame_path: PathBuf,
    pub frame_hash: String,
    pub window_title: Option<String>,
    pub description: String,
    /// Events that occurred within ±500ms of this step's promotion.
    pub events: Vec<TimelineEvent>,
}

/// A simplified event record for the UI (no full EventKind serialization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub seq: u64,
    pub at: DateTime<Utc>,
    pub kind: String,
    pub summary: String,
}

/// The full timeline index for a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineIndex {
    pub session_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: i64,
    pub total_events: usize,
    pub entries: Vec<TimelineEntry>,
}

/// Serializable view sent to the frontend.
pub type TimelineView = TimelineIndex;

const EVENT_WINDOW_MS: i64 = 500;

impl TimelineIndex {
    /// Build the index from a session's event log.
    pub fn build(events_log: &Path, frames_dir: &Path) -> Result<Self> {
        let records = event_log::read_all(events_log)
            .with_context(|| format!("reading {}", events_log.display()))?;

        let total_events = records.len();
        let mut session_id = String::new();
        let mut started_at: Option<DateTime<Utc>> = None;
        let mut ended_at: Option<DateTime<Utc>> = None;

        // First pass: collect step promotions and their timestamps.
        let mut steps: Vec<(i64, usize, String, Option<String>, PathBuf)> = Vec::new();
        let mut descriptions: BTreeMap<usize, String> = BTreeMap::new();

        for r in &records {
            session_id = r.session.to_string();
            match &r.body {
                EventKind::SessionStart { .. } => started_at = Some(r.at),
                EventKind::SessionEnd { .. } => ended_at = Some(r.at),
                EventKind::StepPromoted {
                    step_index,
                    frame_hash,
                    window_title,
                    ..
                } => {
                    let ts_ms = r.at.timestamp_millis();
                    let frame_path = frames_dir.join(format!("{frame_hash}.png"));
                    steps.push((
                        ts_ms,
                        *step_index,
                        frame_hash.clone(),
                        window_title.clone(),
                        frame_path,
                    ));
                }
                EventKind::AiDescription {
                    step_index, text, ..
                } => {
                    descriptions.insert(*step_index, text.clone());
                }
                EventKind::DescriptionEdited { step_index, text } => {
                    descriptions.insert(*step_index, text.clone());
                }
                EventKind::StepDeleted { step_index } => {
                    steps.retain(|(_, idx, _, _, _)| idx != step_index);
                    descriptions.remove(step_index);
                }
                _ => {}
            }
        }

        // Second pass: for each step, collect events within ±EVENT_WINDOW_MS.
        let mut entries: Vec<TimelineEntry> = Vec::with_capacity(steps.len());
        for (ts_ms, step_index, frame_hash, window_title, frame_path) in &steps {
            let mut nearby_events = Vec::new();
            for r in &records {
                let r_ms = r.at.timestamp_millis();
                if (r_ms - ts_ms).abs() <= EVENT_WINDOW_MS {
                    nearby_events.push(TimelineEvent {
                        seq: r.seq,
                        at: r.at,
                        kind: event_kind_name(&r.body),
                        summary: event_summary(&r.body),
                    });
                }
            }

            let description = descriptions
                .get(step_index)
                .cloned()
                .unwrap_or_else(|| format!("Step {}", step_index + 1));

            entries.push(TimelineEntry {
                timestamp_ms: *ts_ms,
                timestamp: DateTime::from_timestamp_millis(*ts_ms)
                    .unwrap_or_default(),
                step_index: *step_index,
                frame_path: frame_path.clone(),
                frame_hash: frame_hash.clone(),
                window_title: window_title.clone(),
                description,
                events: nearby_events,
            });
        }

        // Sort by timestamp.
        entries.sort_by_key(|e| e.timestamp_ms);

        let duration_ms = match (started_at, ended_at) {
            (Some(s), Some(e)) => e.timestamp_millis() - s.timestamp_millis(),
            _ => entries
                .last()
                .map(|e| e.timestamp_ms - entries.first().map(|f| f.timestamp_ms).unwrap_or(0))
                .unwrap_or(0),
        };

        Ok(Self {
            session_id,
            started_at,
            ended_at,
            duration_ms,
            total_events,
            entries,
        })
    }
}

fn event_kind_name(body: &EventKind) -> String {
    match body {
        EventKind::SessionStart { .. } => "session_start",
        EventKind::SessionEnd { .. } => "session_end",
        EventKind::StepPromoted { .. } => "step_promoted",
        EventKind::AiDescription { .. } => "ai_description",
        EventKind::DescriptionEdited { .. } => "description_edited",
        EventKind::StepDeleted { .. } => "step_deleted",
        EventKind::WindowFocusChanged { .. } => "window_focus",
        EventKind::MouseClick { .. } => "mouse_click",
        EventKind::KeyboardInput { .. } => "keyboard_input",
        EventKind::OcrResult { .. } => "ocr_result",
        EventKind::RedactionApplied { .. } => "redaction_applied",
        EventKind::FrameSampled { .. } => "frame_sampled",
        
    }
    .to_string()
}

fn event_summary(body: &EventKind) -> String {
    match body {
        EventKind::SessionStart { title, .. } => format!("Session started: {title}"),
        EventKind::SessionEnd { reason, .. } => format!("Session ended: {reason}"),
        EventKind::StepPromoted { step_index, .. } => format!("Step {} captured", step_index + 1),
        EventKind::AiDescription { step_index, .. } => {
            format!("AI description for step {}", step_index + 1)
        }
        EventKind::DescriptionEdited { step_index, .. } => {
            format!("Description edited for step {}", step_index + 1)
        }
        EventKind::StepDeleted { step_index } => format!("Step {} deleted", step_index + 1),
        EventKind::WindowFocusChanged { window_title, .. } => {
            format!("Window focus: {}", window_title.as_deref().unwrap_or("unknown"))
        }
        EventKind::MouseClick { x, y, button, .. } => format!("Click {button} at ({x}, {y})"),
        EventKind::OcrResult { frame_hash, .. } => {
            format!("OCR completed for {}", &frame_hash[..8])
        }
        EventKind::RedactionApplied {
            rule_name,
            match_count,
            ..
        } => format!("{match_count} matches redacted by {rule_name}"),
        _ => "Event".to_string(),
    }
}