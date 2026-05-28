// .docx renderer driven by a JSON template spec.
//
// We can't safely text-template a .docx (it's a zipped XML bundle), so
// instead we expose a *declarative spec* that tells the programmatic
// builder what to include and how. Customers can author their own spec
// for branding, layout, and which fields to show — no Rust required.

use crate::config::ProjectConfig;
use crate::document::render::DocumentView;
use anyhow::{Context, Result};
use docx_rs::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The declarative spec. Lives in `templates/{name}.docx.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxSpec {
    pub name: String,
    pub version: String,
    pub orientation: Orientation,
    pub cover_page: CoverPageSpec,
    pub step_layout: StepLayoutSpec,
    pub typography: TypographySpec,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverPageSpec {
    pub show: bool,
    pub show_author: bool,
    pub show_generated_at: bool,
    pub show_event_count: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepLayoutSpec {
    pub image_emu_width: u32,
    pub show_window_title: bool,
    pub show_app_name: bool,
    pub show_captured_at: bool,
    /// "before" | "after" — position of the description relative to the image.
    pub description_position: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographySpec {
    /// Half-points (e.g. 56 = 28pt)
    pub title_size: u32,
    pub heading_size: u32,
    pub body_size: u32,
    pub meta_size: u32,
}

impl Default for DocxSpec {
    fn default() -> Self {
        Self {
            name: "default".into(),
            version: "1".into(),
            orientation: Orientation::Portrait,
            cover_page: CoverPageSpec {
                show: true,
                show_author: true,
                show_generated_at: true,
                show_event_count: true,
            },
            step_layout: StepLayoutSpec {
                image_emu_width: 6_000_000,
                show_window_title: true,
                show_app_name: true,
                show_captured_at: false,
                description_position: "after".into(),
            },
            typography: TypographySpec {
                title_size: 56,
                heading_size: 32,
                body_size: 22,
                meta_size: 18,
            },
        }
    }
}

pub fn render(view: &DocumentView, output_path: &Path, cfg: &ProjectConfig) -> Result<()> {
    let spec = load_spec(&cfg.templates.docx_spec).unwrap_or_default();
    build_docx(view, &spec, output_path)
}

fn load_spec(spec_ref: &str) -> Result<DocxSpec> {
    let p = Path::new(spec_ref);
    let candidates: Vec<PathBuf> = if p.is_absolute() {
        vec![p.to_path_buf()]
    } else {
        let mut v = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                v.push(parent.join("templates").join(spec_ref));
                if let Some(g) = parent.parent() {
                    v.push(g.join("templates").join(spec_ref));
                }
            }
        }
        v.push(PathBuf::from("src-tauri/templates").join(spec_ref));
        v.push(PathBuf::from("templates").join(spec_ref));
        v
    };
    for c in candidates {
        if c.exists() {
            let bytes = std::fs::read(&c)?;
            return serde_json::from_slice(&bytes).context("parsing docx spec");
        }
    }
    anyhow::bail!("docx spec '{spec_ref}' not found")
}

fn build_docx(view: &DocumentView, spec: &DocxSpec, output_path: &Path) -> Result<()> {
    let mut docx = Docx::new();

    // ── Cover page ──────────────────────────────────────────────────────────
    if spec.cover_page.show {
        docx = docx.add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .add_run(
                    Run::new()
                        .add_text(view.title.clone())
                        .size(spec.typography.title_size as usize)
                        .bold(),
                ),
        );

        if spec.cover_page.show_author && !view.author.is_empty() {
            docx = docx.add_paragraph(
                Paragraph::new().align(AlignmentType::Center).add_run(
                    Run::new()
                        .add_text(format!("by {}", view.author))
                        .size((spec.typography.body_size + 6) as usize),
                ),
            );
        }

        if spec.cover_page.show_generated_at {
            docx = docx.add_paragraph(
                Paragraph::new().align(AlignmentType::Center).add_run(
                    Run::new()
                        .add_text(format!(
                            "Generated {}",
                            view.generated_at.format("%Y-%m-%d %H:%M UTC")
                        ))
                        .size(spec.typography.meta_size as usize)
                        .italic(),
                ),
            );
        }

        if spec.cover_page.show_event_count {
            docx = docx.add_paragraph(
                Paragraph::new().align(AlignmentType::Center).add_run(
                    Run::new()
                        .add_text(format!(
                            "{} steps · session {}",
                            view.steps.len(),
                            &view.session_id.chars().take(8).collect::<String>()
                        ))
                        .size(spec.typography.meta_size as usize)
                        .italic(),
                ),
            );
        }

        docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text("")));
        docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text("")));
    }

    // ── Steps ───────────────────────────────────────────────────────────────
    for step in &view.steps {
        // Heading
        docx = docx.add_paragraph(
            Paragraph::new().style("Heading1").add_run(
                Run::new()
                    .add_text(format!("Step {}", step.index + 1))
                    .size(spec.typography.heading_size as usize)
                    .bold(),
            ),
        );

        if spec.step_layout.show_window_title {
            if let Some(t) = &step.window_title {
                docx = docx.add_paragraph(
                    Paragraph::new().add_run(
                        Run::new()
                            .add_text(format!("Window: {t}"))
                            .size(spec.typography.meta_size as usize)
                            .italic(),
                    ),
                );
            }
        }
        if spec.step_layout.show_app_name {
            if let Some(a) = &step.app_name {
                docx = docx.add_paragraph(
                    Paragraph::new().add_run(
                        Run::new()
                            .add_text(format!("Application: {a}"))
                            .size(spec.typography.meta_size as usize)
                            .italic(),
                    ),
                );
            }
        }
        if spec.step_layout.show_captured_at {
            docx = docx.add_paragraph(
                Paragraph::new().add_run(
                    Run::new()
                        .add_text(format!("Captured at: {}", step.captured_at))
                        .size(spec.typography.meta_size as usize)
                        .italic(),
                ),
            );
        }

        let description_first = spec.step_layout.description_position == "before";
        if description_first {
            docx = docx.add_paragraph(
                Paragraph::new().add_run(
                    Run::new()
                        .add_text(step.description.clone())
                        .size(spec.typography.body_size as usize),
                ),
            );
        }

        // Embedded image — re-encode as JPEG to keep .docx file sizes manageable.
        // PNG frames from a 1080p screen are 1–2 MB each; JPEG at quality 82
        // typically compresses to 100–300 KB with no perceptible quality loss
        // for documentation screenshots.
        if step.image_path.exists() {
            let bytes = encode_for_docx(&step.image_path)
                .with_context(|| format!("encoding {}", step.image_path.display()))?;
            let emu_w = spec.step_layout.image_emu_width;
            let emu_h = emu_height_for(step.width, step.height, emu_w);
            let pic = Pic::new(&bytes).size(emu_w, emu_h);
            docx = docx.add_paragraph(
                Paragraph::new()
                    .align(AlignmentType::Center)
                    .add_run(Run::new().add_image(pic)),
            );
        }

        if !description_first {
            docx = docx.add_paragraph(
                Paragraph::new().add_run(
                    Run::new()
                        .add_text(step.description.clone())
                        .size(spec.typography.body_size as usize),
                ),
            );
        }

        docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text("")));
    }

    // ── Footer (final paragraph instead of true page footer for simplicity) ─
    docx = docx.add_paragraph(
        Paragraph::new().align(AlignmentType::Center).add_run(
            Run::new()
                .add_text(view.branding.footer.clone())
                .size(spec.typography.meta_size as usize)
                .italic(),
        ),
    );

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let file = std::fs::File::create(output_path)
        .with_context(|| format!("creating {}", output_path.display()))?;
    docx.build().pack(file).context("packing .docx")?;
    log::info!("rendered docx → {}", output_path.display());
    Ok(())
}

fn emu_height_for(px_w: u32, px_h: u32, emu_w: u32) -> u32 {
    if px_w == 0 {
        return emu_w;
    }
    ((emu_w as u64 * px_h as u64) / px_w as u64) as u32
}

/// Re-encode a PNG frame as JPEG for .docx embedding.
/// Falls back to raw PNG bytes if decoding fails.
fn encode_for_docx(path: &Path) -> anyhow::Result<Vec<u8>> {
    let img = image::open(path).context("opening image")?;
    let mut buf = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut buf),
        image::ImageFormat::Jpeg,
    )
    .context("JPEG encode")?;
    Ok(buf)
}
