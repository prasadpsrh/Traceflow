// Capture-to-export integration tests.
//
// Every test in this file goes through the real production code paths.
// No mocking of core logic — only the frame *source* is synthetic (solid-colour
// or patterned RGBA images built with the `image` crate) so tests run offline
// without a display server.
//
// Structure:
//   helpers        — FakeSession builder, synthetic image factories
//   chain_*        — event-log hash-chain correctness
//   projection_*   — StepView projection from events (deletion, editing)
//   capture_*      — diff algorithm, stability machine, promote-on-replace
//   export_*       — all four output formats
//   describer_*    — smart window-title parsing

use image::{ImageBuffer, Rgba, RgbaImage};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use traceflow_lib::{
    ai::describer,
    capture::{diff, engine::simulate_capture_decisions},
    config::ProjectConfig,
    document::render::{project_steps_for_ui, render_to_file, RenderRequest},
    events::{
        chain::verify_chain,
        event::{EventKind, SessionPaths},
        log::{read_all, EventLog},
    },
};
use uuid::Uuid;

// ─── Synthetic image helpers ──────────────────────────────────────────────────

fn solid_rgba(r: u8, g: u8, b: u8) -> RgbaImage {
    ImageBuffer::from_pixel(640, 360, Rgba([r, g, b, 255]))
}

/// Dark background with `fraction` of pixels lit (simulates text on dark theme).
fn dark_with_patch(fraction: f32) -> RgbaImage {
    let mut img = solid_rgba(20, 20, 20);
    let (w, h) = img.dimensions();
    let to_change = ((w * h) as f32 * fraction) as u32;
    for i in 0..to_change {
        img.put_pixel(i % w, i / w, Rgba([200, 200, 200, 255]));
    }
    img
}

fn png_bytes(img: &RgbaImage) -> Vec<u8> {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

fn frame_hash(img: &RgbaImage) -> String {
    hex::encode(Sha256::digest(png_bytes(img)))
}

// ─── FakeSession ──────────────────────────────────────────────────────────────

/// Builds a complete on-disk session in a temp directory using only the
/// production EventLog / render code — no Tauri runtime required.
struct FakeSession {
    dir: tempfile::TempDir,
    log: EventLog,
    session_id: Uuid,
}

impl FakeSession {
    fn new(title: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let paths = SessionPaths::under(dir.path().to_path_buf());
        std::fs::create_dir_all(&paths.frames_dir).unwrap();
        let session_id = Uuid::new_v4();
        let log = EventLog::create(&paths.events_log, session_id).unwrap();
        log.append(EventKind::SessionStart {
            title: title.to_string(),
            capture_settings: serde_json::json!({}),
            app_version: "0.2.0-test".into(),
            host_os: "test".into(),
        })
        .unwrap();
        FakeSession {
            dir,
            log,
            session_id,
        }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn events_log(&self) -> PathBuf {
        self.root().join("events.ndjson")
    }

    fn frames_dir(&self) -> PathBuf {
        self.root().join("frames")
    }

    /// Write a frame PNG to `frames/` and append a StepPromoted event.
    /// Returns the content-addressed hash.
    fn add_step(
        &self,
        index: usize,
        img: &RgbaImage,
        window_title: Option<&str>,
        app_name: Option<&str>,
    ) -> String {
        let bytes = png_bytes(img);
        let hash = hex::encode(Sha256::digest(&bytes));
        let frame_path = self.frames_dir().join(format!("{hash}.png"));
        if !frame_path.exists() {
            std::fs::write(&frame_path, &bytes).unwrap();
        }
        self.log
            .append(EventKind::StepPromoted {
                step_index: index,
                frame_hash: hash.clone(),
                width: img.width(),
                height: img.height(),
                window_title: window_title.map(str::to_string),
                window_class: None,
                app_name: app_name.map(str::to_string),
            })
            .unwrap();
        hash
    }

    fn add_ai_desc(&self, step_index: usize, text: &str) {
        self.log
            .append(EventKind::AiDescription {
                step_index,
                text: text.to_string(),
                model: "test".into(),
                language: "en".into(),
            })
            .unwrap();
    }

    fn edit_desc(&self, step_index: usize, text: &str) {
        self.log
            .append(EventKind::DescriptionEdited {
                step_index,
                text: text.to_string(),
            })
            .unwrap();
    }

    fn delete_step(&self, step_index: usize) {
        self.log
            .append(EventKind::StepDeleted { step_index })
            .unwrap();
    }

    fn end(&self) {
        self.log
            .append(EventKind::SessionEnd {
                reason: "test".into(),
            })
            .unwrap();
    }

    fn render_request(&self, ext: &str) -> RenderRequest {
        RenderRequest {
            events_log: self.events_log(),
            frames_dir: self.frames_dir(),
            output_path: self.root().join(format!("out.{ext}")),
            title: "Integration Test".to_string(),
            author: Some("test".to_string()),
        }
    }
}

// ═══ Chain tests ══════════════════════════════════════════════════════════════

/// Every event written through EventLog::append must survive read_all → verify_chain.
#[test]
fn chain_intact_after_full_session() {
    let sess = FakeSession::new("chain test");
    for i in 0..5 {
        sess.add_step(i, &solid_rgba(100 + i as u8 * 20, 100, 100), None, None);
    }
    sess.end();

    let records = read_all(&sess.events_log()).unwrap();
    // SessionStart + 5 StepPromoted + SessionEnd = 7
    assert_eq!(records.len(), 7);
    let n = verify_chain(records.iter()).expect("chain should be intact");
    assert_eq!(n, 7);
}

/// Mutating a field without recomputing the hash must break the chain.
#[test]
fn tampered_event_breaks_chain() {
    let sess = FakeSession::new("tamper test");
    sess.add_step(0, &solid_rgba(200, 200, 200), None, None);
    sess.end();

    let mut records = read_all(&sess.events_log()).unwrap();
    // Silently mutate the step description field in the StepPromoted body.
    if let EventKind::StepPromoted {
        ref mut frame_hash, ..
    } = records[1].body
    {
        frame_hash.push_str("__tampered");
    }
    let result = verify_chain(records.iter());
    assert!(result.is_err(), "tampered chain must fail verification");
}

/// An empty session (no steps) still has a valid two-event chain.
#[test]
fn empty_session_chain_valid() {
    let sess = FakeSession::new("empty");
    sess.end();

    let records = read_all(&sess.events_log()).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(verify_chain(records.iter()).unwrap(), 2);
}

/// Sessions with all currently-defined EventKind variants all hash cleanly.
#[test]
fn all_event_kinds_survive_chain() {
    let sess = FakeSession::new("all variants");
    let img = solid_rgba(80, 80, 80);
    sess.add_step(0, &img, Some("Test Window"), Some("test.exe"));
    sess.add_ai_desc(0, "AI description here");
    sess.edit_desc(0, "User-edited description");
    sess.add_step(1, &solid_rgba(160, 160, 160), None, None);
    sess.delete_step(1);
    sess.log
        .append(EventKind::WindowFocusChanged {
            window_title: Some("Explorer".into()),
            window_class: None,
            app_name: Some("explorer.exe".into()),
        })
        .unwrap();
    sess.log
        .append(EventKind::RedactionApplied {
            frame_hash: Some(frame_hash(&img)),
            rule_name: "email".into(),
            match_count: 2,
            action: "mask".into(),
        })
        .unwrap();
    sess.end();

    let records = read_all(&sess.events_log()).unwrap();
    let n = verify_chain(records.iter()).expect("all variants must chain");
    assert_eq!(n as usize, records.len());
}

// ═══ Projection tests ═══════════════════════════════════════════════════════════════

/// Deleted steps must not appear in the UI projection.
#[test]
fn deleted_step_absent_from_projection() {
    let sess = FakeSession::new("deletion");
    sess.add_step(0, &solid_rgba(100, 100, 100), None, None);
    sess.add_step(1, &solid_rgba(150, 150, 150), None, None);
    sess.add_step(2, &solid_rgba(200, 200, 200), None, None);
    sess.delete_step(1);
    sess.end();

    let records = read_all(&sess.events_log()).unwrap();
    let steps = project_steps_for_ui(&records, &sess.frames_dir());

    assert_eq!(steps.len(), 2, "step 1 must be absent");
    // Steps are re-indexed contiguously after deletion.
    assert!(steps.iter().all(|s| s.index < 2));
    let hashes: Vec<_> = steps
        .iter()
        .map(|s| s.image_path.file_name().unwrap().to_str().unwrap())
        .collect();
    assert!(
        !hashes
            .iter()
            .any(|h| h.starts_with(&frame_hash(&solid_rgba(150, 150, 150)))),
        "deleted frame must not appear"
    );
}

/// A user-edited description must win over the earlier AI description.
#[test]
fn user_edit_wins_over_ai_description() {
    let sess = FakeSession::new("description priority");
    sess.add_step(0, &solid_rgba(128, 128, 128), None, None);
    sess.add_ai_desc(0, "AI generated text");
    sess.edit_desc(0, "User corrected text");
    sess.end();

    let records = read_all(&sess.events_log()).unwrap();
    let steps = project_steps_for_ui(&records, &sess.frames_dir());

    assert_eq!(steps[0].description, "User corrected text");
}

/// If only an AI description exists (no user edit), the AI text is used.
#[test]
fn ai_description_used_when_no_edit() {
    let sess = FakeSession::new("ai only");
    sess.add_step(0, &solid_rgba(80, 80, 80), None, None);
    sess.add_ai_desc(0, "AI description only");
    sess.end();

    let records = read_all(&sess.events_log()).unwrap();
    let steps = project_steps_for_ui(&records, &sess.frames_dir());
    assert_eq!(steps[0].description, "AI description only");
}

// ═══ Frame deduplication tests ══════════════════════════════════════════════════

/// Identical frames must reuse a single PNG file (content-addressed).
#[test]
fn duplicate_frames_produce_single_file() {
    let sess = FakeSession::new("dedup");
    let img = solid_rgba(123, 200, 45);
    let hash0 = sess.add_step(0, &img, None, None);
    let hash1 = sess.add_step(1, &img, None, None); // same image
    sess.end();

    assert_eq!(hash0, hash1, "same content must hash identically");
    let files: Vec<_> = std::fs::read_dir(sess.frames_dir())
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(files.len(), 1, "only one PNG should exist in frames/");
}

// ═══ Capture state-machine tests ══════════════════════════════════════════════

fn fp(img: &RgbaImage) -> image::GrayImage {
    diff::fingerprint(img)
}

/// Stable repetition of the same frame after a change must promote exactly once.
#[test]
fn stable_frame_promotes_once() {
    let a = solid_rgba(20, 20, 20);
    let b = solid_rgba(220, 220, 220);
    // sequence: A A A B B B  — one change, one promotion
    let fps = [fp(&a), fp(&a), fp(&a), fp(&b), fp(&b), fp(&b)];
    let promoted = simulate_capture_decisions(&fps, 0.04, 1);
    assert_eq!(promoted.len(), 1, "exactly one step promoted");
    assert_eq!(promoted[0], 3, "the B frame (index 3) is promoted");
}

/// Two distinct screens in sequence each get promoted.
#[test]
fn two_screen_changes_both_promoted() {
    let a = solid_rgba(20, 20, 20);
    let b = solid_rgba(120, 120, 120);
    let c = solid_rgba(220, 220, 220);
    // A(ref) A B B C C
    let fps = [fp(&a), fp(&a), fp(&b), fp(&b), fp(&c), fp(&c)];
    let promoted = simulate_capture_decisions(&fps, 0.04, 1);
    assert_eq!(promoted.len(), 2, "both B and C must be promoted");
}

/// Fast navigation: B is seen only once before C arrives.
/// Promote-on-replace must save B rather than discard it.
#[test]
fn fast_navigation_no_screen_lost() {
    let a = solid_rgba(20, 20, 20);
    let b = solid_rgba(100, 100, 100);
    let c = solid_rgba(180, 180, 180);
    let d = solid_rgba(240, 240, 240);
    // A(ref) B(candidate, never confirmed) C(triggers promote-B) D D
    let fps = [fp(&a), fp(&b), fp(&c), fp(&d), fp(&d)];
    let promoted = simulate_capture_decisions(&fps, 0.04, 1);
    assert!(
        promoted.len() >= 3,
        "B, C, D must all be captured; got {:?}",
        promoted
    );
}

/// stability_frames = 2 means a screen appearing for only 1 frame is
/// captured by promote-on-replace when the next change arrives.
#[test]
fn stability_two_requires_one_extra_confirmation() {
    let a = solid_rgba(20, 20, 20);
    let b = solid_rgba(200, 200, 200);
    // A(ref) B(candidate streak=0) B(streak=1, still <2) B(streak=2 → promote)
    let fps = [fp(&a), fp(&b), fp(&b), fp(&b)];
    let promoted = simulate_capture_decisions(&fps, 0.04, 2);
    assert_eq!(promoted.len(), 1);
    assert_eq!(promoted[0], 1);
}

/// Candidate flushed at end of sequence (simulates Stop button pressed while
/// user is still on the last screen).
#[test]
fn unconfirmed_candidate_flushed_at_end() {
    let a = solid_rgba(20, 20, 20);
    let b = solid_rgba(200, 200, 200);
    // A(ref) B(candidate, no further frames) — B must be flushed
    let fps = [fp(&a), fp(&b)];
    let promoted = simulate_capture_decisions(&fps, 0.04, 2);
    assert_eq!(
        promoted,
        vec![1],
        "unconfirmed candidate must be flushed at end"
    );
}

/// The hybrid MAD+CPC diff must detect sparse dark-theme content changes.
#[test]
fn dark_theme_sparse_change_above_threshold() {
    let base = solid_rgba(20, 20, 20);
    let changed = dark_with_patch(0.08); // 8% of pixels lit
    let score = diff::diff_score(&fp(&base), &fp(&changed));
    assert!(
        score >= 0.04,
        "dark-theme 8% pixel change must exceed threshold 0.04, got {score:.4}"
    );
}

/// Identical frames score exactly zero.
#[test]
fn identical_frames_score_zero() {
    let img = solid_rgba(50, 100, 150);
    let score = diff::diff_score(&fp(&img), &fp(&img));
    assert!(score < 0.001, "identical frames must score ≈0, got {score}");
}

/// Sub-threshold noise (< 1% tiny-delta pixels) does not trigger capture.
#[test]
fn cursor_jitter_below_threshold() {
    let base = solid_rgba(200, 200, 200);
    // Change a single 1x1 pixel — simulates cursor blink / clock update.
    let mut noisy = base.clone();
    noisy.put_pixel(0, 0, Rgba([210, 200, 200, 255]));
    let score = diff::diff_score(&fp(&base), &fp(&noisy));
    assert!(
        score < 0.04,
        "single-pixel jitter must not exceed threshold, got {score}"
    );
}

// ═══ Export tests ════════════════════════════════════════════════════════════

fn build_export_session() -> FakeSession {
    let sess = FakeSession::new("export test");
    let screens = [
        (
            solid_rgba(30, 30, 30),
            Some("GitHub - Microsoft Edge"),
            Some("msedge.exe"),
        ),
        (
            solid_rgba(230, 230, 230),
            Some("Settings - Windows Settings"),
            Some("SystemSettings.exe"),
        ),
        (
            solid_rgba(80, 80, 80),
            Some("main.rs - my-project - Code"),
            Some("code.exe"),
        ),
    ];
    for (i, (img, title, app)) in screens.iter().enumerate() {
        sess.add_step(i, img, *title, *app);
        sess.add_ai_desc(i, &format!("Step {} description", i + 1));
    }
    sess.end();
    sess
}

#[test]
fn export_docx_non_empty() {
    let sess = build_export_session();
    let cfg = ProjectConfig::default();
    render_to_file(&sess.render_request("docx"), &cfg).expect("docx export must succeed");
    let size = std::fs::metadata(sess.root().join("out.docx"))
        .unwrap()
        .len();
    assert!(
        size > 1000,
        ".docx must be non-trivially large, got {size} bytes"
    );
}

#[test]
fn export_markdown_contains_step_headings() {
    let sess = build_export_session();
    let cfg = ProjectConfig::default();
    render_to_file(&sess.render_request("md"), &cfg).expect("md export must succeed");
    let content = std::fs::read_to_string(sess.root().join("out.md")).unwrap();
    assert!(
        content.contains("## Step 1"),
        "markdown must contain step heading"
    );
    assert!(content.contains("## Step 2"), "markdown must have step 2");
    assert!(content.contains("## Step 3"), "markdown must have step 3");
    // RenderRequest::title overrides SessionStart title at export time.
    assert!(
        content.contains("# Integration Test"),
        "render title must appear as H1, got:\n{content}"
    );
}

#[test]
fn export_html_has_valid_structure() {
    let sess = build_export_session();
    let cfg = ProjectConfig::default();
    render_to_file(&sess.render_request("html"), &cfg).expect("html export must succeed");
    let content = std::fs::read_to_string(sess.root().join("out.html")).unwrap();
    assert!(content.contains("<!doctype html>"), "must be valid HTML");
    assert!(
        content.contains("<title>Integration Test</title>"),
        "render title must be in <head>, got:\n{content}"
    );
    assert!(content.contains("class=\"step\""), "step divs must exist");
    assert!(content.contains("Step 1"), "step 1 must appear");
}

#[test]
fn export_json_has_expected_fields() {
    let sess = build_export_session();
    let cfg = ProjectConfig::default();
    render_to_file(&sess.render_request("json"), &cfg).expect("json export must succeed");
    let raw = std::fs::read_to_string(sess.root().join("out.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("must be valid JSON");

    // RenderRequest::title is the export-time title, independent of SessionStart title.
    assert_eq!(doc["title"], "Integration Test");
    assert_eq!(doc["author"], "test");
    let steps = doc["steps"].as_array().expect("steps must be an array");
    assert_eq!(steps.len(), 3, "3 steps promoted");
    // Each step must carry index, description, image_path.
    for (i, step) in steps.iter().enumerate() {
        assert_eq!(step["index"], i, "step index must be sequential");
        assert!(
            step["description"].is_string(),
            "description must be string"
        );
        assert!(step["image_path"].is_string(), "image_path must be string");
    }
    assert!(
        doc["event_count"].as_u64().unwrap() > 0,
        "event_count must be positive"
    );
}

#[test]
fn export_json_deleted_step_absent() {
    let sess = FakeSession::new("deletion in json");
    sess.add_step(0, &solid_rgba(100, 100, 100), None, None);
    sess.add_step(1, &solid_rgba(150, 150, 150), None, None);
    sess.add_step(2, &solid_rgba(200, 200, 200), None, None);
    sess.delete_step(1);
    sess.end();

    let cfg = ProjectConfig::default();
    render_to_file(&sess.render_request("json"), &cfg).unwrap();
    let raw = std::fs::read_to_string(sess.root().join("out.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let steps = doc["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2, "deleted step must not appear in JSON");
}

// ═══ Describer tests ══════════════════════════════════════════════════════════

fn desc(title: &str, app: &str) -> String {
    let img = solid_rgba(200, 200, 200);
    describer::describe(&img, Some(title), Some(app)).unwrap()
}

#[test]
fn browser_edge_extracts_page_title() {
    let d = desc("GitHub - Microsoft Edge", "msedge.exe");
    assert!(d.contains("GitHub"), "must extract page title, got: {d}");
    assert!(
        !d.to_lowercase().contains("edge"),
        "must not leak browser name"
    );
}

#[test]
fn browser_chrome_strips_site_suffix() {
    let d = desc(
        "Stack Overflow - Stack Exchange - Google Chrome",
        "chrome.exe",
    );
    assert!(
        d.contains("Stack Overflow"),
        "must extract page name, got: {d}"
    );
}

#[test]
fn browser_new_tab_gives_friendly_name() {
    let d = desc("New Tab - Google Chrome", "chrome.exe");
    assert!(
        d.to_lowercase().contains("new") || d.to_lowercase().contains("tab"),
        "new-tab must produce a friendly label, got: {d}"
    );
}

#[test]
fn browser_login_page_flagged() {
    let d = desc("Sign in - Google Accounts - Mozilla Firefox", "firefox.exe");
    assert!(
        d.to_lowercase().contains("sign") || d.to_lowercase().contains("login"),
        "login page must be flagged, got: {d}"
    );
}

#[test]
fn vscode_shows_filename_and_app() {
    let d = desc("main.rs - my-project - Visual Studio Code", "code.exe");
    assert!(
        d.contains("VS Code") || d.contains("Code"),
        "app must be named, got: {d}"
    );
    assert!(d.contains("main.rs"), "filename must appear, got: {d}");
}

#[test]
fn file_explorer_shows_folder() {
    let d = desc("Downloads - File Explorer", "explorer.exe");
    assert!(
        d.to_lowercase().contains("file explorer") || d.to_lowercase().contains("explorer"),
        "must identify File Explorer, got: {d}"
    );
    assert!(d.contains("Downloads"), "folder name must appear, got: {d}");
}

#[test]
fn terminal_shows_context() {
    let d = desc("Administrator: Windows PowerShell", "powershell.exe");
    assert!(
        d.to_lowercase().contains("terminal") || d.to_lowercase().contains("powershell"),
        "must identify terminal, got: {d}"
    );
}

#[test]
fn installer_detected_from_title() {
    let d = desc("Acme Pro Setup - InstallShield Wizard", "msiexec.exe");
    assert!(
        d.to_lowercase().contains("install") || d.to_lowercase().contains("setup"),
        "must detect installer, got: {d}"
    );
}

#[test]
fn excel_shows_document_name() {
    let d = desc("budget_2026.xlsx - Excel", "excel.exe");
    assert!(d.contains("Excel"), "app name must appear, got: {d}");
    assert!(
        d.contains("budget_2026.xlsx"),
        "document name must appear, got: {d}"
    );
}

#[test]
fn dark_image_no_title_gives_fallback() {
    let img = solid_rgba(20, 20, 20);
    let d = describer::describe(&img, None, None).unwrap();
    assert!(
        d.to_lowercase().contains("dark"),
        "dark screen fallback expected, got: {d}"
    );
}

#[test]
fn light_image_no_title_gives_fallback() {
    let img = solid_rgba(240, 240, 240);
    let d = describer::describe(&img, None, None).unwrap();
    assert!(
        d.to_lowercase().contains("light") || d.to_lowercase().contains("application"),
        "light screen fallback expected, got: {d}"
    );
}
