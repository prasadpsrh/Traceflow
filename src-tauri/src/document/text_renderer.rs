// Text-format renderer.
//
// Loads a MiniJinja template by path or short name, evaluates it against
// the `DocumentView`, and writes the result. Works for any text format —
// Markdown, HTML, plain text, custom JSON shapes, even RST or AsciiDoc
// if a user authors a template for it.
//
// Built-in template lookup order:
//   1. Absolute path → load directly.
//   2. Relative path → resolve against the bundled templates dir, then
//      against an OS-level "user templates" dir (under data_root).

use crate::document::render::DocumentView;
use anyhow::{Context, Result};
use minijinja::Environment;
use std::path::{Path, PathBuf};

pub fn render(view: &DocumentView, output_path: &Path, template_ref: &str) -> Result<()> {
    let template_src = load_template(template_ref)
        .with_context(|| format!("looking up template '{template_ref}'"))?;
    let mut env = Environment::new();
    // Add a couple of convenience filters.
    env.add_filter("iso8601", |v: minijinja::Value| {
        v.to_string() // serde dates already render as ISO 8601
    });
    env.add_template("doc", &template_src)
        .context("compiling template")?;
    let tpl = env.get_template("doc")?;
    let ctx = minijinja::Value::from_serialize(view);
    let rendered = tpl.render(ctx).context("rendering template")?;
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(output_path, rendered)
        .with_context(|| format!("writing {}", output_path.display()))?;
    log::info!("rendered → {}", output_path.display());
    Ok(())
}

/// Resolve a template reference to its source text.
fn load_template(template_ref: &str) -> Result<String> {
    let p = Path::new(template_ref);
    if p.is_absolute() && p.exists() {
        return Ok(std::fs::read_to_string(p)?);
    }
    // Try bundled location relative to the executable.
    for base in template_dirs() {
        let candidate = base.join(template_ref);
        if candidate.exists() {
            return Ok(std::fs::read_to_string(candidate)?);
        }
    }
    // Last resort: built-in fallback so the product works out-of-the-box
    // even before any templates are copied to disk.
    if template_ref == "default.md.j2" {
        return Ok(BUILTIN_MD.to_string());
    }
    if template_ref == "default.html.j2" {
        return Ok(BUILTIN_HTML.to_string());
    }
    anyhow::bail!("template '{template_ref}' not found")
}

fn template_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    // Relative to the binary (dev: src-tauri/templates)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            out.push(parent.join("templates"));
            if let Some(p2) = parent.parent() {
                out.push(p2.join("templates"));
            }
        }
    }
    // Project-relative templates dir
    out.push(PathBuf::from("src-tauri/templates"));
    out.push(PathBuf::from("templates"));
    // User data dir
    if let Some(d) = dirs::document_dir() {
        out.push(d.join("Traceflow").join("templates"));
    }
    out
}

const BUILTIN_MD: &str = r#"# {{ title }}

{% if author %}*by {{ author }}*  {% endif %}
*Generated {{ generated_at }}*

---

{% for step in steps %}
## Step {{ step.index + 1 }}

{% if step.window_title %}**Window:** {{ step.window_title }}  {% endif %}
{% if step.app_name %}**Application:** {{ step.app_name }}  {% endif %}

![Step {{ step.index + 1 }}]({{ step.image_path }})

{{ step.description }}

---
{% endfor %}

<sub>{{ branding.footer }} · session {{ session_id[:8] }} · {{ event_count }} events</sub>
"#;

const BUILTIN_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{{ title }}</title>
<style>
  body { font-family: Georgia, serif; max-width: 820px; margin: 40px auto; padding: 0 20px; color: #181614; }
  h1 { font-size: 36px; letter-spacing: -0.02em; border-bottom: 2px solid {{ branding.accent_color }}; padding-bottom: 8px; }
  h2 { font-size: 22px; color: {{ branding.accent_color }}; margin-top: 36px; }
  img { max-width: 100%; border: 1px solid #d8cfbe; border-radius: 3px; }
  .meta { color: #8a8278; font-size: 12px; }
  .step { margin-bottom: 32px; }
  footer { margin-top: 60px; padding-top: 20px; border-top: 1px solid #d8cfbe; color: #8a8278; font-size: 11px; }
</style>
</head>
<body>
<h1>{{ title }}</h1>
<p class="meta">
  {% if author %}by {{ author }} · {% endif %}generated {{ generated_at }}
</p>

{% for step in steps %}
<section class="step">
  <h2>Step {{ step.index + 1 }}</h2>
  {% if step.window_title %}<p class="meta">Window: {{ step.window_title }}</p>{% endif %}
  <img src="{{ step.image_path }}" alt="Step {{ step.index + 1 }}">
  <p>{{ step.description }}</p>
</section>
{% endfor %}

<footer>
  {{ branding.footer }} · session {{ session_id[:8] }} · {{ event_count }} events
</footer>
</body>
</html>
"#;
