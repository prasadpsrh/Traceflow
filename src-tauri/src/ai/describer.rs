// Context-aware step description generator.
//
// Priority order:
//   1. Browser detected  → extract page title from window title
//   2. Known native app  → map exe/display name to a readable label
//   3. Generic title     → parse "Document — App" patterns
//   4. Image analysis    → brightness + shape heuristics as last resort
//
// Phase 2: replace with a bundled vision-language model (BLIP-2 / SmolVLM via
// `candle` or `ort`). The function signature is already stable for that drop-in.

use anyhow::Result;
use image::RgbaImage;

/// Generate a one-line heading for a captured step.
pub fn describe(
    frame: &RgbaImage,
    window_title: Option<&str>,
    app_name: Option<&str>,
) -> Result<String> {
    if let Some(desc) = describe_from_context(window_title, app_name, frame) {
        return Ok(desc);
    }
    Ok(describe_from_image(frame))
}

// ── Context-aware path ────────────────────────────────────────────────────────

fn describe_from_context(
    title: Option<&str>,
    app: Option<&str>,
    frame: &RgbaImage,
) -> Option<String> {
    let title = title.unwrap_or("").trim();
    let app_lc = app.unwrap_or("").trim().to_lowercase();

    // 1. Browser: identified by exe/display name or title suffix
    if is_browser_app(&app_lc) || title_ends_with_browser(title) {
        return Some(describe_browser_step(title, frame));
    }

    // 2. Known native apps
    if let Some(desc) = describe_known_app(&app_lc, title) {
        return Some(desc);
    }

    // 3. Generic "Document — App" parsing
    if !title.is_empty() {
        return Some(parse_generic_title(title));
    }

    None
}

// ── Browser detection & page-title extraction ─────────────────────────────────

const BROWSER_EXE: &[&str] = &[
    "chrome", "msedge", "firefox", "opera", "brave", "vivaldi", "iexplore", "safari", "arc",
    "thorium", "chromium",
];

const BROWSER_DISPLAY: &[&str] = &[
    "google chrome",
    "microsoft edge",
    "mozilla firefox",
    "opera",
    "brave browser",
    "vivaldi",
    "internet explorer",
    "safari",
    "arc",
];

fn is_browser_app(app_lc: &str) -> bool {
    BROWSER_EXE.iter().any(|b| app_lc.contains(b))
        || BROWSER_DISPLAY.iter().any(|b| app_lc.contains(b))
}

/// Browsers append " - Browser Name" to the window title.
fn title_ends_with_browser(title: &str) -> bool {
    let lc = title.to_lowercase();
    BROWSER_EXE.iter().any(|b| lc.contains(&format!(" - {b}")))
        || BROWSER_DISPLAY
            .iter()
            .any(|b| lc.ends_with(b) || lc.contains(&format!(" - {b}")))
}

fn describe_browser_step(title: &str, frame: &RgbaImage) -> String {
    let page = extract_browser_page_title(title);

    // Classify the kind of browser content from the page title
    if page.is_empty() || page.to_lowercase().contains("new tab") {
        return "New browser tab opened".to_string();
    }
    if page.to_lowercase() == "downloads" {
        return "Browser Downloads page".to_string();
    }
    if page.to_lowercase().starts_with("settings") {
        return format!("Browser settings — {page}");
    }

    // Detect if it looks like a login/auth page
    let pl = page.to_lowercase();
    if pl.contains("sign in")
        || pl.contains("log in")
        || pl.contains("login")
        || pl.contains("sign up")
    {
        return format!("Sign-in page — {page}");
    }

    // Detect checkout / payment
    if pl.contains("checkout") || pl.contains("payment") || pl.contains("cart") {
        return format!("Checkout — {page}");
    }

    // If we can detect a dialog / modal on a web page
    if detect_modal(frame) {
        return format!("Modal dialog on — {page}");
    }

    page.to_string()
}

/// Extract the page title from a browser window title.
///
/// Browser window titles follow one of these patterns:
///   "Page Title - Browser Name"
///   "Page Title | Site Name - Browser Name"
///   "Page Title — Site - Browser Name"
fn extract_browser_page_title(title: &str) -> String {
    // Strip known browser suffixes from the right
    let mut s = title.trim().to_string();
    for browser in BROWSER_DISPLAY.iter().chain(BROWSER_EXE.iter()) {
        let suffix_dash = format!(" - {}", capitalise(browser));
        let suffix_dash2 = format!(" - {browser}");
        for suffix in [&suffix_dash, &suffix_dash2] {
            if let Some(stripped) = s
                .to_lowercase()
                .rfind(&suffix.to_lowercase())
                .map(|pos| s[..pos].trim().to_string())
            {
                s = stripped;
                break;
            }
        }
    }

    // Strip trailing site name after "|" or " - "
    if let Some(pos) = s.rfind(" | ") {
        s = s[..pos].trim().to_string();
    } else if let Some(pos) = s.rfind(" - ") {
        // Only strip if what follows looks like a site name (no spaces in domain)
        let suffix = s[pos + 3..].trim();
        if !suffix.contains(' ') || suffix.split_whitespace().count() <= 3 {
            s = s[..pos].trim().to_string();
        }
    }

    s
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// ── Known native-app mapping ──────────────────────────────────────────────────

fn describe_known_app(app_lc: &str, title: &str) -> Option<String> {
    // File managers
    if app_lc.contains("explorer") && !app_lc.contains("internet") {
        let folder = extract_before_last_dash(title).unwrap_or_else(|| title.to_string());
        return Some(
            if folder.is_empty() || folder.to_lowercase() == "file explorer" {
                "File Explorer".to_string()
            } else {
                format!("File Explorer — {folder}")
            },
        );
    }

    // Terminals
    if app_lc.contains("windowsterminal")
        || app_lc.contains("terminal")
        || app_lc.contains("cmd")
        || app_lc.contains("powershell")
        || app_lc.contains("conhost")
        || app_lc.contains("bash")
        || app_lc.contains("wsl")
    {
        let context = title.trim();
        return Some(if context.is_empty() {
            "Terminal".to_string()
        } else {
            format!("Terminal — {context}")
        });
    }

    // Code editors / IDEs
    if app_lc.contains("code") && (app_lc.contains("visual") || app_lc == "code") {
        return Some(parse_editor_title(title, "VS Code"));
    }
    if app_lc.contains("devenv") || (app_lc.contains("visual") && app_lc.contains("studio")) {
        return Some(parse_editor_title(title, "Visual Studio"));
    }
    if app_lc.contains("idea") || app_lc.contains("intellij") {
        return Some(parse_editor_title(title, "IntelliJ IDEA"));
    }
    if app_lc.contains("pycharm") {
        return Some(parse_editor_title(title, "PyCharm"));
    }
    if app_lc.contains("notepad++") {
        return Some(parse_editor_title(title, "Notepad++"));
    }
    if app_lc.contains("notepad") {
        return Some(parse_editor_title(title, "Notepad"));
    }
    if app_lc.contains("sublime") {
        return Some(parse_editor_title(title, "Sublime Text"));
    }

    // Office suite
    if app_lc.contains("winword") || (app_lc.contains("word") && app_lc.contains("microsoft")) {
        return Some(parse_office_title(title, "Word"));
    }
    if app_lc.contains("excel") {
        return Some(parse_office_title(title, "Excel"));
    }
    if app_lc.contains("powerpnt") || app_lc.contains("powerpoint") {
        return Some(parse_office_title(title, "PowerPoint"));
    }
    if app_lc.contains("outlook") {
        return Some(parse_office_title(title, "Outlook"));
    }

    // System utilities
    if app_lc.contains("taskmgr") || title.to_lowercase().contains("task manager") {
        return Some("Task Manager".to_string());
    }
    if app_lc.contains("control") || title.to_lowercase().contains("control panel") {
        return Some(format!("Control Panel — {}", title.trim()));
    }
    if app_lc.contains("mmc") {
        return Some(format!("Management Console — {}", title.trim()));
    }
    if app_lc.contains("regedit") {
        return Some("Registry Editor".to_string());
    }
    if app_lc.contains("msiexec")
        || title.to_lowercase().contains("setup")
        || title.to_lowercase().contains("install")
    {
        return Some(format!("Installer — {}", title.trim()));
    }

    // Settings (Windows 10/11)
    if app_lc.contains("systemsettings") || app_lc.contains("winstore") {
        return Some(format!("Settings — {}", title.trim()));
    }

    // Slack / Teams / Zoom / Discord
    if app_lc.contains("slack") {
        return Some(format!(
            "Slack — {}",
            extract_before_last_dash(title).unwrap_or_default()
        ));
    }
    if app_lc.contains("teams") {
        return Some(format!("Microsoft Teams — {}", title.trim()));
    }
    if app_lc.contains("zoom") {
        return Some(format!("Zoom — {}", title.trim()));
    }
    if app_lc.contains("discord") {
        return Some(format!("Discord — {}", title.trim()));
    }

    None
}

fn parse_editor_title(title: &str, app: &str) -> String {
    // "filename.rs — project — VS Code" → "filename.rs in project"
    // "Welcome - VS Code" → "VS Code — Welcome"
    if let Some(file) = extract_before_last_dash(title) {
        if !file.is_empty() && !file.to_lowercase().contains(app.to_lowercase().as_str()) {
            return format!("{app} — {file}");
        }
    }
    format!("{app} — {}", title.trim())
}

fn parse_office_title(title: &str, app: &str) -> String {
    // "Document1 - Word" or "Budget.xlsx - Excel"
    if let Some(doc) = extract_before_last_dash(title) {
        if !doc.is_empty() {
            return format!("{app} — {doc}");
        }
    }
    app.to_string()
}

// ── Generic title parsing ─────────────────────────────────────────────────────

fn parse_generic_title(title: &str) -> String {
    // "Document — App Name" → "App Name — Document"  (swap for readability)
    if let Some(before) = extract_before_last_dash(title) {
        let app_part = title[title.rfind(" - ").map(|i| i + 3).unwrap_or(title.len())..].trim();
        if !before.is_empty() && !app_part.is_empty() && before != app_part {
            return format!("{app_part} — {before}");
        }
    }
    title.trim().to_string()
}

fn extract_before_last_dash(title: &str) -> Option<String> {
    title
        .rfind(" - ")
        .map(|pos| title[..pos].trim().to_string())
}

// ── Image analysis fallback ───────────────────────────────────────────────────

fn describe_from_image(frame: &RgbaImage) -> String {
    if detect_modal(frame) {
        return "Dialog or modal shown".to_string();
    }
    if detect_context_menu(frame) {
        return "Context menu or dropdown opened".to_string();
    }

    let brightness = mean_brightness(frame);
    if brightness > 210 {
        "Light-themed application screen".to_string()
    } else if brightness < 55 {
        "Dark-themed application screen".to_string()
    } else {
        "Application screen changed".to_string()
    }
}

fn mean_brightness(frame: &RgbaImage) -> u32 {
    let (w, h) = frame.dimensions();
    let n = (w * h) as u64;
    let mut sum: u64 = 0;
    for p in frame.pixels() {
        sum += (p[0] as u32 + p[1] as u32 + p[2] as u32) as u64 / 3;
    }
    (sum / n) as u32
}

/// Heuristic: sample a central band and compare it to the screen edges.
/// A modal/dialog creates a lighter/darker rectangle on a dimmed background.
fn detect_modal(frame: &RgbaImage) -> bool {
    let (w, h) = frame.dimensions();
    if w < 200 || h < 200 {
        return false;
    }

    // Sample a 10-pixel strip around the edges (background region)
    let mut edge_sum: u64 = 0;
    let mut edge_n: u64 = 0;
    for x in (0..w).step_by(4) {
        for &y in &[5u32, h - 6] {
            let p = frame.get_pixel(x, y);
            edge_sum += (p[0] as u64 + p[1] as u64 + p[2] as u64) / 3;
            edge_n += 1;
        }
    }

    // Sample the central 40 % of the screen
    let cx0 = w * 3 / 10;
    let cx1 = w * 7 / 10;
    let cy0 = h * 3 / 10;
    let cy1 = h * 7 / 10;
    let mut center_sum: u64 = 0;
    let mut center_n: u64 = 0;
    for x in (cx0..cx1).step_by(4) {
        for y in (cy0..cy1).step_by(4) {
            let p = frame.get_pixel(x, y);
            center_sum += (p[0] as u64 + p[1] as u64 + p[2] as u64) / 3;
            center_n += 1;
        }
    }

    if edge_n == 0 || center_n == 0 {
        return false;
    }

    let edge_avg = (edge_sum / edge_n) as i64;
    let center_avg = (center_sum / center_n) as i64;
    // Modal: center is noticeably brighter than dimmed edges (or vice-versa)
    (center_avg - edge_avg).unsigned_abs() > 30
}

/// Heuristic: a context menu is a narrow tall region that's brighter/darker
/// than its surroundings, occupying < 30 % of screen width.
fn detect_context_menu(frame: &RgbaImage) -> bool {
    let (w, h) = frame.dimensions();
    if w < 100 || h < 100 {
        return false;
    }
    // Sample three vertical strips (left-quarter, centre, right-quarter)
    // and check if one strip has clearly different brightness from the others.
    let cols = [w / 8, w / 2, w * 7 / 8];
    let mut avgs = [0u64; 3];
    for (i, &cx) in cols.iter().enumerate() {
        let x = cx.min(w - 1);
        let mut s: u64 = 0;
        let mut n: u64 = 0;
        for y in (0..h).step_by(4) {
            let p = frame.get_pixel(x, y);
            s += (p[0] as u64 + p[1] as u64 + p[2] as u64) / 3;
            n += 1;
        }
        avgs[i] = if n > 0 { s / n } else { 0 };
    }
    let max = avgs.iter().copied().max().unwrap_or(0) as i64;
    let min = avgs.iter().copied().min().unwrap_or(0) as i64;
    max - min > 40
}

// ── Stable public interface for future model drop-in ─────────────────────────

#[allow(dead_code)]
pub fn describe_with_model(_frame: &RgbaImage, _lang: &str) -> Result<String> {
    unimplemented!("model-backed describer not yet wired")
}
