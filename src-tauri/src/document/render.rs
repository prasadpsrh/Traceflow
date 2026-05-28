// Render orchestrator.
//
// Reads the event log, projects it into a sequence of `StepView`s (one per
// promoted step), then dispatches to the right renderer based on output
// extension. Renderers receive the same data — the only difference is
// formatting.

use crate::config::ProjectConfig;
use crate::document::{docx_renderer, text_renderer};
use crate::events::{event::EventKind, log as event_log, EventRecord};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One "step" as understood by templates and the .docx renderer.
/// This is a *derived view* over the event log; the log itself remains
/// the source of truth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepView {
    pub index: usize,
    pub captured_at: DateTime<Utc>,
    /// Absolute filesystem path to the screenshot PNG.
    pub image_path: PathBuf,
    /// Most recent description (AI or user-edited).
    pub description: String,
    pub window_title: Option<String>,
    pub app_name: Option<String>,
    pub width: u32,
    pub height: u32,
}

/// Top-level "document view" passed to text templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentView {
    pub title: String,
    pub author: String,
    pub generated_at: DateTime<Utc>,
    pub session_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub steps: Vec<StepView>,
    pub branding: BrandingView,
    /// Total events in the log — useful for audit summaries in templates.
    pub event_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingView {
    pub accent_color: String,
    pub footer: String,
    pub logo_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenderRequest {
    /// Path to the session's events.ndjson.
    pub events_log: PathBuf,
    /// Directory containing the frame PNGs (frames/{hash}.png).
    pub frames_dir: PathBuf,
    /// Output file. Extension drives the renderer: .docx, .md, .html, .json, .txt.
    pub output_path: PathBuf,
    /// Display title.
    pub title: String,
    pub author: Option<String>,
}

/// Entry point. Builds the projection, picks a renderer, writes the file.
pub fn render_to_file(req: &RenderRequest, cfg: &ProjectConfig) -> Result<PathBuf> {
    let records = event_log::read_all(&req.events_log)
        .with_context(|| format!("reading {}", req.events_log.display()))?;
    let view = project(records, req, cfg)?;
    let ext = req
        .output_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "docx" => docx_renderer::render(&view, &req.output_path, cfg)?,
        "md" | "markdown" => text_renderer::render(&view, &req.output_path, &cfg.templates.markdown)?,
        "html" | "htm" => text_renderer::render(&view, &req.output_path, &cfg.templates.html)?,
        "json" => {
            let s = serde_json::to_string_pretty(&view).context("serializing view")?;
            std::fs::write(&req.output_path, s)
                .with_context(|| format!("writing {}", req.output_path.display()))?;
        }
        other => anyhow::bail!("unsupported output extension: .{other}"),
    }
    Ok(req.output_path.clone())
}

/// Project the event log into a renderable document view.
/// Folds StepPromoted → AiDescription → DescriptionEdited → StepDeleted in order.
fn project(
    records: Vec<EventRecord>,
    req: &RenderRequest,
    cfg: &ProjectConfig,
) -> Result<DocumentView> {
    let event_count = records.len() as u64;
    let mut started_at: Option<DateTime<Utc>> = None;
    let mut ended_at: Option<DateTime<Utc>> = None;
    let mut session_id = String::new();
    let mut steps: std::collections::BTreeMap<usize, StepView> =
        std::collections::BTreeMap::new();

    for r in &records {
        session_id = r.session.to_string();
        match &r.body {
            EventKind::SessionStart { .. } => started_at = Some(r.at),
            EventKind::SessionEnd { .. } => ended_at = Some(r.at),
            EventKind::StepPromoted {
                step_index,
                frame_hash,
                width,
                height,
                window_title,
                app_name,
                ..
            } => {
                let image_path = req.frames_dir.join(format!("{frame_hash}.png"));
                steps.insert(
                    *step_index,
                    StepView {
                        index: *step_index,
                        captured_at: r.at,
                        image_path,
                        description: format!("Step {}", step_index + 1),
                        window_title: window_title.clone(),
                        app_name: app_name.clone(),
                        width: *width,
                        height: *height,
                    },
                );
            }
            EventKind::AiDescription {
                step_index, text, ..
            } => {
                if let Some(s) = steps.get_mut(step_index) {
                    s.description = text.clone();
                }
            }
            EventKind::DescriptionEdited { step_index, text } => {
                if let Some(s) = steps.get_mut(step_index) {
                    s.description = text.clone();
                }
            }
            EventKind::StepDeleted { step_index } => {
                steps.remove(step_index);
            }
            _ => {}
        }
    }

    let mut steps_vec: Vec<StepView> = steps.into_values().collect();
    // Reindex contiguously after deletions
    for (new_idx, s) in steps_vec.iter_mut().enumerate() {
        s.index = new_idx;
    }

    Ok(DocumentView {
        title: req.title.clone(),
        author: req.author.clone().unwrap_or_else(|| cfg.project.author.clone()),
        generated_at: Utc::now(),
        session_id,
        started_at,
        ended_at,
        steps: steps_vec,
        branding: BrandingView {
            accent_color: cfg.branding.accent_color.clone(),
            footer: cfg.branding.footer.clone(),
            logo_path: cfg.branding.logo_path.clone(),
        },
        event_count,
    })
}

/// Same projection logic as `render_to_file`, exposed for direct UI use.
pub fn project_steps_for_ui(records: &[crate::events::EventRecord], frames_dir: &Path) -> Vec<StepView> {
    use crate::events::event::EventKind;
    let mut steps: std::collections::BTreeMap<usize, StepView> = std::collections::BTreeMap::new();
    for r in records {
        match &r.body {
            EventKind::StepPromoted {
                step_index,
                frame_hash,
                width,
                height,
                window_title,
                app_name,
                ..
            } => {
                steps.insert(
                    *step_index,
                    StepView {
                        index: *step_index,
                        captured_at: r.at,
                        image_path: frames_dir.join(format!("{frame_hash}.png")),
                        description: format!("Step {}", step_index + 1),
                        window_title: window_title.clone(),
                        app_name: app_name.clone(),
                        width: *width,
                        height: *height,
                    },
                );
            }
            EventKind::AiDescription { step_index, text, .. }
            | EventKind::DescriptionEdited { step_index, text } => {
                if let Some(s) = steps.get_mut(step_index) {
                    s.description = text.clone();
                }
            }
            EventKind::StepDeleted { step_index } => {
                steps.remove(step_index);
            }
            _ => {}
        }
    }
    let mut v: Vec<StepView> = steps.into_values().collect();
    for (i, s) in v.iter_mut().enumerate() {
        s.index = i;
    }
    v
}

/// Quickly compute a default save path for an export.
pub fn default_output_path(root: &Path, title: &str, ext: &str) -> PathBuf {
    let safe: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    root.join(format!("{safe}.{ext}"))
}
