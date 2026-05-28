// Per-project configuration.
//
// `traceflow.config.toml` sits in the project root and tells Traceflow which
// rule packs to activate, which templates to use for export, and how to
// brand outputs. Everything is overridable; nothing is required.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub project: ProjectMeta,
    #[serde(default)]
    pub capture: CaptureCfg,
    #[serde(default)]
    pub rules: RulesCfg,
    #[serde(default)]
    pub templates: TemplatesCfg,
    #[serde(default)]
    pub branding: BrandingCfg,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureCfg {
    pub poll_fps: u32,
    pub change_threshold: f32,
    pub stability_frames: u32,
    pub ai_describe: bool,
    pub redact_pii: bool,
    pub language: String,
    /// If true, persist every sampled frame (not just promoted ones).
    /// Costs disk space, enables full forensic replay.
    pub keep_all_frames: bool,
    /// Zero-based index of the monitor to capture. 0 = primary.
    #[serde(default)]
    pub monitor_index: u32,
}

impl Default for CaptureCfg {
    fn default() -> Self {
        Self {
            poll_fps: 8,
            change_threshold: 0.04,
            stability_frames: 1,
            ai_describe: true,
            redact_pii: true,
            language: "en".into(),
            keep_all_frames: false,
            monitor_index: 0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RulesCfg {
    /// Rule pack files to load, relative to the project's rule_packs dir
    /// or absolute paths.
    #[serde(default)]
    pub packs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatesCfg {
    /// Template file (relative to templates dir or absolute) to use for .docx.
    pub docx_spec: String,
    /// Jinja-style template for Markdown export.
    pub markdown: String,
    /// Jinja-style template for HTML export.
    pub html: String,
}

impl Default for TemplatesCfg {
    fn default() -> Self {
        Self {
            docx_spec: "default.docx.json".into(),
            markdown: "default.md.j2".into(),
            html: "default.html.j2".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingCfg {
    pub accent_color: String,
    pub footer: String,
    pub logo_path: Option<PathBuf>,
}

impl Default for BrandingCfg {
    fn default() -> Self {
        Self {
            accent_color: "#c33c1f".into(),
            footer: "Generated with Traceflow".into(),
            logo_path: None,
        }
    }
}

impl ProjectConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let s = std::str::from_utf8(&bytes)
            .with_context(|| format!("invalid UTF-8 in {}", path.display()))?;
        let cfg: ProjectConfig =
            toml::from_str(s).with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
    }

    pub fn load_or_default(path: &Path) -> Self {
        Self::load(path).unwrap_or_else(|e| {
            log::info!(
                "no project config at {} ({e}); using defaults",
                path.display()
            );
            Self::default()
        })
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let s = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(path, s).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            project: ProjectMeta::default(),
            capture: CaptureCfg::default(),
            rules: RulesCfg {
                packs: vec!["general_pii.json".into(), "secrets.json".into()],
            },
            templates: TemplatesCfg::default(),
            branding: BrandingCfg::default(),
        }
    }
}
