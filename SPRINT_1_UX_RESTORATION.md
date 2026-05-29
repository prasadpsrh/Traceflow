# Sprint 1 — Phase 1 UX Restoration

> **Goal:** Restore the rule wizard, live regex tester, and pack management UI that was dropped during the v2 rebuild — without disturbing the working OCR/redaction/capture pipeline.
> **Status:** Ready for implementation.
> **Estimated effort:** 3–5 days for one engineer, or 1.5–2 days split across two engineers.
> **Standards reference:** Follow `ENGINEERING_STANDARDS.md` throughout, especially §3 (Rust), §4 (TypeScript), §7 (Testing), and §13 (PR conventions).

---

## Table of contents

1. [Context: what we're restoring and why](#1-context-what-were-restoring-and-why)
2. [Scope and non-goals](#2-scope-and-non-goals)
3. [Acceptance criteria](#3-acceptance-criteria)
4. [Architecture recap](#4-architecture-recap)
5. [Backend work](#5-backend-work)
6. [Frontend work](#6-frontend-work)
7. [Testing plan](#7-testing-plan)
8. [Rollout checklist](#8-rollout-checklist)
9. [Code: complete files](#9-code-complete-files)
10. [Verification script](#10-verification-script)

---

## 1. Context: what we're restoring and why

During the v2 rebuild, three high-value UX features got dropped:

| Feature | What it does | Why it matters |
|---|---|---|
| **Rule wizard** | 8 preset patterns (email, phone, credit card, SSN, AWS key, JWT, IPv4, ticket ID) selectable by one click | The "no JSON" experience for 90% of users — they pick a preset, tweak the replacement text, and save |
| **Live regex tester** | Paste sample text, see matches highlighted in real time as you type the pattern | Makes the advanced builder usable by non-regex-experts |
| **Pack import/export** | "Save this pack to a file" / "Load a pack a colleague sent me" | Sharing and team standardization without git access |

These three features are what convert Traceflow from "a screen capture tool with JSON config" into "a product non-technical users can customize." Re-adding them costs ~700 lines of code across 5 files. The OCR / redaction / capture pipeline is untouched.

## 2. Scope and non-goals

### In scope (this sprint)

- 🟢 Wizard preset library (Rust + UI)
- 🟢 Live regex tester command + UI hook
- 🟢 Rule pack import / export (Rust + UI)
- 🟢 Custom rule add / remove (Rust + UI)
- 🟢 Tests for every new IPC command
- 🟢 Standards-compliant documentation

### Out of scope (later sprints)

- 🔴 Template picker UI (Sprint 3 / 4)
- 🔴 Branding controls (color picker, logo) (Sprint 3 / 4)
- 🔴 Project metadata UI (name, author) (Sprint 3 / 4)
- 🔴 Cross-platform OCR (Sprint 2)
- 🔴 Replay viewer (Sprint 3)

This sprint keeps a tight focus on the rule-pack management story so it can ship in a week.

## 3. Acceptance criteria

The sprint is done when **all** of these are true:

1. The settings panel has three new actions: **Wizard**, **Advanced builder**, and **Import pack**.
2. The wizard shows the 8 presets grouped by category. Clicking one opens the builder pre-filled with that preset.
3. The advanced builder includes a "Test pattern" field with a sample-text box. As the user types either field, the matched substrings appear as chips beneath them.
4. Custom rules can be added with the "Add rule" button and removed from the list via a per-row "Remove" action.
5. Any built-in or user-authored pack can be exported to a file via the "Export" button on the pack row.
6. A `.json` file produced by export can be re-imported via "Import pack" and round-trips correctly.
7. New backend tests verify pattern validation, Luhn validator, import-export round-trip, and wizard preset integrity.
8. `cargo test` passes, `cargo clippy -- -D warnings` passes, `npm run build` passes.
9. The pre-commit hook (`.githooks/pre-commit`) runs clean.
10. Documentation updated: `USER_MANUAL.md` describes the wizard and builder; `CHANGELOG.md` has entries.

## 4. Architecture recap

We're operating on the rule-pack subsystem only. The existing pieces stay:

```
src-tauri/src/rules/
├── mod.rs         ← unchanged (re-exports)
└── engine.rs      ← unchanged (Rule, RulePack, RuleEngine, Luhn validator)
```

We add **one** new module:

```
src-tauri/src/rules/
├── mod.rs         ← updated to re-export library items
├── engine.rs      ← unchanged
└── library.rs     ← NEW: pack discovery, custom rules, wizard presets, import/export
```

We add **six** new IPC commands (registered in `lib.rs`):

| Command | Purpose |
|---|---|
| `rule_wizard_presets` | Return the 8 curated presets |
| `rule_pattern_test` | Evaluate a pattern against sample text, return matches |
| `custom_rule_add` | Append a rule to the user's custom pack |
| `custom_rule_remove` | Delete a rule from the custom pack by name |
| `rule_pack_import` | Load an external `.json` and copy it to the user pack dir |
| `rule_pack_export` | Copy a pack's `.json` to an arbitrary destination |

`rule_packs_list` and `rule_packs_active` already exist (`list_rule_packs` and `save_rule_pack` / `toggle_rule_pack` in the current code). We extend them rather than replacing.

We add **four** new React components:

```
src/components/
├── SettingsPanel.tsx       ← existing, gains 4 new buttons
└── settings/               ← NEW directory
    ├── WizardGrid.tsx      ← preset picker grid
    ├── RuleBuilderForm.tsx ← advanced form with live regex tester
    ├── CustomRuleList.tsx  ← list + remove for user-authored rules
    └── (shared CSS appended to styles.css)
```

## 5. Backend work

### 5.1 New module: `src-tauri/src/rules/library.rs`

Full file in §9.1.

Responsibilities:
- **Pack discovery** — finds packs in built-in dirs and user dir (already in `engine.rs`; we move some of it here for clarity).
- **Custom-rules file** — a single `custom_rules.json` per user, managed entirely by the app.
- **Add / remove rule** — modifies the custom file, validates pattern compiles before saving.
- **Import** — copies an external `.json` into the user pack dir, validates every pattern first.
- **Export** — copies a pack file to a user-chosen destination.
- **Wizard library** — returns the 8 curated presets.

**Standards notes:**
- All public items have doc comments (§14.1).
- All errors propagated with `anyhow::Result` + `.context()` (§5.1).
- Unit tests for the wizard library (preset count, regex validity, ID uniqueness) at the bottom of the file (§7.3).

### 5.2 Module declaration: `src-tauri/src/rules/mod.rs`

Full file in §9.2.

Adds `pub mod library;` and re-exports the public items.

### 5.3 New IPC commands: appended to `src-tauri/src/commands.rs`

Full code in §9.3.

Each command is thin — delegates to `library.rs` for real work. Signatures:

```rust
#[tauri::command]
pub async fn rule_wizard_presets() -> Result<Vec<WizardPreset>, String>;

#[tauri::command]
pub async fn rule_pattern_test(
    pattern: String,
    sample: String,
) -> Result<PatternTestResult, String>;

#[tauri::command]
pub async fn custom_rule_add(rule: Rule) -> Result<RulePack, String>;

#[tauri::command]
pub async fn custom_rule_remove(rule_name: String) -> Result<RulePack, String>;

#[tauri::command]
pub async fn rule_pack_import(src_path: PathBuf) -> Result<PackSummary, String>;

#[tauri::command]
pub async fn rule_pack_export(
    file_name: String,
    dest_path: PathBuf,
) -> Result<(), String>;
```

### 5.4 Command registration: `src-tauri/src/lib.rs`

Append six lines to the `invoke_handler!` macro. Full diff in §9.4.

### 5.5 New integration tests: appended to `src-tauri/tests/integration.rs`

Full test code in §9.5.

Tests added:
- `wizard_presets_all_have_valid_regex`
- `wizard_preset_ids_are_unique`
- `pattern_test_returns_matches`
- `pattern_test_rejects_invalid_regex`
- `custom_rule_add_round_trip`
- `custom_rule_validates_pattern_before_save`
- `rule_pack_export_import_round_trip`
- `luhn_validator_rejects_invalid_card`

## 6. Frontend work

### 6.1 Types — append to `src/types.ts`

Full code in §9.6. Adds: `RuleAction`, `Rule`, `RulePack`, `WizardPreset`, `PatternTestResult`, `PackSummary`.

### 6.2 Update `src/components/SettingsPanel.tsx`

Three additions to the existing panel:
- "Wizard" button — opens `WizardGrid`
- "Advanced builder" button — opens `RuleBuilderForm` with no preset
- "Import pack" button — file picker → invoke `rule_pack_import` → refresh list
- Each pack row gets an "Export" button alongside "Enable / Disable"
- Custom rules section listing user-authored rules with per-row "Remove"

Code structure (full file in §9.7) follows the existing panel's React patterns.

### 6.3 New component: `src/components/settings/WizardGrid.tsx`

Full code in §9.8. Renders the 8 presets grouped by category. Each card shows label, description, and example match. Click → calls `onPick(preset)`.

### 6.4 New component: `src/components/settings/RuleBuilderForm.tsx`

Full code in §9.9. The advanced form with:
- Name field
- Pattern field
- Sample text field
- Live tester: on change, debounced (200ms) `invoke<PatternTestResult>("rule_pattern_test", ...)` and render match chips
- Description, severity, action, optional Luhn validator
- Pre-fill from a wizard preset if one was passed in
- Cancel / Save buttons

### 6.5 New component: `src/components/settings/CustomRuleList.tsx`

Full code in §9.10. Shows the user's custom rules with remove buttons.

### 6.6 Styles — append to `src/styles.css`

Full CSS in §9.11. Adds wizard grid layout, builder form styles, pattern-test chip styles, custom rule list rows.

## 7. Testing plan

### 7.1 Backend tests (required per §7.3)

All new IPC commands must have integration tests using the `FakeSession` builder. Code in §9.5.

Run:
```bash
cd src-tauri && cargo test
```

Expected: existing 30 tests still pass, 8 new tests pass.

### 7.2 Frontend manual smoke test

After `npm run tauri dev`:

1. Open the app, click **Settings** tab.
2. Click **Wizard**.
3. Confirm 8 cards across 5 categories (Personal data, Financial, Secrets, Network, Custom).
4. Click "Email addresses." Builder opens with the preset filled in.
5. Modify the replacement to `[USER_EMAIL]`. Click Save.
6. Confirm a new entry appears in **Your custom rules** with name `wizard_email`.
7. Click **Advanced builder**. Empty form opens.
8. Type `\b[A-Z]{3}-\d{4}\b` into Pattern.
9. Type `Order ABC-1234 received from XYZ-9999` into Sample.
10. Confirm two match chips appear: `"ABC-1234"` and `"XYZ-9999"`.
11. Click Save with name `order_id`.
12. Confirm it appears in the custom list.
13. Click **Remove** on `wizard_email`. Confirm it disappears.
14. Click **Export** on the `secrets` built-in pack. Save as `~/secrets-test.json`.
15. Click **Import pack**. Pick the file. Confirm a new pack appears in the list named `secrets` (it may be renamed if there's a collision — that's fine).

### 7.3 Regression check

These must still work unchanged:
- Starting / stopping a capture session
- Verifying the session chain
- Exporting to docx / md / html / json
- The settings panel's existing rule pack enable/disable
- The OCR + redaction pipeline (Windows)

If any regress, the sprint is blocked.

## 8. Rollout checklist

Before opening the PR:

- [ ] `cd src-tauri && cargo fmt`
- [ ] `cd src-tauri && cargo clippy --all-targets -- -D warnings`
- [ ] `cd src-tauri && cargo test` — 30 existing + 8 new = 38 tests pass
- [ ] `npm run build` from project root
- [ ] All four files in §9.5 (tests) have at least one assertion
- [ ] Manual smoke test in §7.2 walked through end to end
- [ ] `USER_MANUAL.md` updated with wizard and builder sections
- [ ] `CHANGELOG.md` has entry under "Unreleased"
- [ ] PR follows `.github/pull_request_template.md`

After merge:

- [ ] Run `cargo audit` — no new advisories
- [ ] Tag a pre-release: `v0.3.0-pre1`
- [ ] Walk through smoke test on a clean Windows machine

## 9. Code: complete files

What follows is every file you need to add or modify, ready to copy in.

---

### 9.1 `src-tauri/src/rules/library.rs` (NEW)

```rust
//! Rule-pack library — discovers, lists, and manages all rule packs available
//! to the user, both built-in (read-only) and user-authored (read-write).
//!
//! Built-in packs ship alongside the binary in `rule_packs/` (relative to the
//! executable in production, or the project root in dev). The user must not
//! be able to modify these — they're our supported, versioned defaults.
//!
//! User packs live in `<data_root>/rule_packs/`. The user can create, edit,
//! or delete these freely. They are loaded *in addition to* whichever
//! built-in packs are activated in the project config.
//!
//! A separate convenience file `<data_root>/rule_packs/custom_rules.json`
//! holds individually-added rules from the UI's "Add custom rule" flow, so
//! the user doesn't need to think about pack files for one-off rules.

use crate::rules::engine::{Rule, RuleAction, RulePack};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where built-in rule packs may live, in priority order.
pub fn builtin_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            out.push(parent.join("rule_packs"));
            if let Some(p2) = parent.parent() {
                out.push(p2.join("rule_packs"));
            }
        }
    }
    out.push(PathBuf::from("src-tauri/rule_packs"));
    out.push(PathBuf::from("rule_packs"));
    out
}

/// Directory under the data root that holds user-authored packs.
pub fn user_dir(data_root: &Path) -> PathBuf {
    data_root.join("rule_packs")
}

/// Path to the user's custom-rules file (managed via the UI).
pub fn custom_rules_path(data_root: &Path) -> PathBuf {
    user_dir(data_root).join("custom_rules.json")
}

/// Lightweight pack descriptor for the UI (no compiled regex inside).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackSummary {
    /// File name relative to its containing dir (e.g. "general_pii.json").
    pub file_name: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub rule_count: usize,
    /// True if shipped by us — the UI should disable edit/delete.
    pub is_builtin: bool,
    /// Absolute path on disk.
    pub path: PathBuf,
}

/// Discover every available pack, built-in first then user-authored.
pub fn list_all(data_root: &Path) -> Result<Vec<PackSummary>> {
    let mut packs = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    // Built-in packs (deduplicate across the candidate search dirs).
    for dir in builtin_dirs() {
        if !dir.exists() {
            continue;
        }
        for entry in walk_json(&dir) {
            if let Ok(summary) = read_summary(&entry, true) {
                if seen_names.insert(summary.name.clone()) {
                    packs.push(summary);
                }
            }
        }
    }

    // User packs.
    let udir = user_dir(data_root);
    if udir.exists() {
        for entry in walk_json(&udir) {
            if let Ok(summary) = read_summary(&entry, false) {
                packs.push(summary);
            }
        }
    }

    Ok(packs)
}

fn walk_json(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("json") {
                out.push(p);
            }
        }
    }
    out
}

fn read_summary(path: &Path, is_builtin: bool) -> Result<PackSummary> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let pack: RulePack = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    Ok(PackSummary {
        file_name,
        name: pack.name,
        version: pack.version,
        description: pack.description,
        rule_count: pack.rules.len(),
        is_builtin,
        path: path.to_path_buf(),
    })
}

/// Load the user's custom-rules file, creating an empty one if missing.
pub fn load_custom(data_root: &Path) -> Result<RulePack> {
    let path = custom_rules_path(data_root);
    if !path.exists() {
        std::fs::create_dir_all(user_dir(data_root)).ok();
        let pack = RulePack {
            name: "custom_rules".into(),
            version: "1".into(),
            description: "User-authored rules (managed through the Traceflow UI)".into(),
            rules: Vec::new(),
        };
        let s = serde_json::to_string_pretty(&pack)
            .context("serializing empty custom rules pack")?;
        std::fs::write(&path, s)
            .with_context(|| format!("writing {}", path.display()))?;
        return Ok(pack);
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing {}", path.display()))
}

/// Persist the user's custom-rules file.
pub fn save_custom(data_root: &Path, pack: &RulePack) -> Result<()> {
    let path = custom_rules_path(data_root);
    std::fs::create_dir_all(user_dir(data_root)).ok();
    let s = serde_json::to_string_pretty(pack).context("serializing custom rules pack")?;
    std::fs::write(&path, s).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Add a single rule to the user's custom pack.
/// Validates the pattern compiles before saving — never leave a broken
/// regex on disk.
pub fn add_custom_rule(data_root: &Path, rule: Rule) -> Result<RulePack> {
    regex::Regex::new(&rule.pattern)
        .with_context(|| format!("rule '{}' has an invalid pattern", rule.name))?;
    let mut pack = load_custom(data_root)?;
    // Replace any existing rule with the same name (last-write-wins).
    pack.rules.retain(|r| r.name != rule.name);
    pack.rules.push(rule);
    save_custom(data_root, &pack)?;
    Ok(pack)
}

/// Remove a rule from the user's custom pack by name.
/// Returns Ok even if the name didn't exist (idempotent).
pub fn remove_custom_rule(data_root: &Path, rule_name: &str) -> Result<RulePack> {
    let mut pack = load_custom(data_root)?;
    pack.rules.retain(|r| r.name != rule_name);
    save_custom(data_root, &pack)?;
    Ok(pack)
}

/// Import a user-authored pack from an external JSON file.
/// Validates every pattern compiles before copying.
pub fn import_pack(data_root: &Path, src: &Path) -> Result<PackSummary> {
    let bytes = std::fs::read(src)
        .with_context(|| format!("reading {}", src.display()))?;
    let pack: RulePack = serde_json::from_slice(&bytes)
        .context("the file is not a valid Traceflow rule pack")?;
    for r in &pack.rules {
        regex::Regex::new(&r.pattern)
            .with_context(|| format!("rule '{}' has an invalid pattern", r.name))?;
    }
    std::fs::create_dir_all(user_dir(data_root)).ok();
    let safe_name: String = pack
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let dest = user_dir(data_root).join(format!("{safe_name}.json"));
    std::fs::write(&dest, &bytes)
        .with_context(|| format!("writing {}", dest.display()))?;
    read_summary(&dest, false)
}

/// Export a pack file to an arbitrary path (for sharing).
pub fn export_pack(src: &Path, dest: &Path) -> Result<()> {
    let bytes = std::fs::read(src)
        .with_context(|| format!("reading {}", src.display()))?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(dest, bytes)
        .with_context(|| format!("writing {}", dest.display()))?;
    Ok(())
}

/// One curated preset surfaced by the wizard UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardPreset {
    pub id: String,
    pub label: String,
    pub category: String,
    pub description: String,
    pub example_match: String,
    pub rule: Rule,
}

/// Return the curated wizard library. The frontend renders this list.
/// Adding a preset: append below, keep the id unique, and add a regex
/// test in the unit tests at the bottom of this file.
pub fn wizard_library() -> Vec<WizardPreset> {
    vec![
        WizardPreset {
            id: "email".into(),
            label: "Email addresses".into(),
            category: "Personal data".into(),
            description: "Match RFC-style email addresses.".into(),
            example_match: "alice@example.com".into(),
            rule: Rule {
                name: "wizard_email".into(),
                pattern: r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b".into(),
                action: RuleAction::Mask { replacement: "[EMAIL]".into() },
                description: "Email address".into(),
                severity: "medium".into(),
                validator: None,
            },
        },
        WizardPreset {
            id: "phone_us".into(),
            label: "Phone numbers (US)".into(),
            category: "Personal data".into(),
            description: "Match US-format phone numbers in common shapes.".into(),
            example_match: "(415) 555-0142".into(),
            rule: Rule {
                name: "wizard_phone_us".into(),
                pattern: r"\b(?:\+?1[-. ]?)?\(?\d{3}\)?[-. ]?\d{3}[-. ]?\d{4}\b".into(),
                action: RuleAction::Mask { replacement: "[PHONE]".into() },
                description: "US phone number".into(),
                severity: "medium".into(),
                validator: None,
            },
        },
        WizardPreset {
            id: "credit_card".into(),
            label: "Credit card numbers".into(),
            category: "Financial".into(),
            description: "Match 13–19 digit numbers that pass the Luhn check.".into(),
            example_match: "4111 1111 1111 1111".into(),
            rule: Rule {
                name: "wizard_credit_card".into(),
                pattern: r"\b(?:\d[ -]*?){13,19}\b".into(),
                action: RuleAction::BlackBox,
                description: "Credit card (Luhn-validated)".into(),
                severity: "critical".into(),
                validator: Some("luhn".into()),
            },
        },
        WizardPreset {
            id: "ssn_us".into(),
            label: "US Social Security Numbers".into(),
            category: "Financial".into(),
            description: "Match 9-digit SSNs in standard XXX-XX-XXXX form.".into(),
            example_match: "123-45-6789".into(),
            rule: Rule {
                name: "wizard_ssn_us".into(),
                pattern: r"\b\d{3}-\d{2}-\d{4}\b".into(),
                action: RuleAction::BlackBox,
                description: "US SSN".into(),
                severity: "critical".into(),
                validator: None,
            },
        },
        WizardPreset {
            id: "aws_key".into(),
            label: "AWS access keys".into(),
            category: "Secrets".into(),
            description: "Match AWS access key IDs (start with AKIA).".into(),
            example_match: "AKIAIOSFODNN7EXAMPLE".into(),
            rule: Rule {
                name: "wizard_aws_key".into(),
                pattern: r"\bAKIA[0-9A-Z]{16}\b".into(),
                action: RuleAction::BlackBox,
                description: "AWS access key ID".into(),
                severity: "critical".into(),
                validator: None,
            },
        },
        WizardPreset {
            id: "jwt".into(),
            label: "JSON Web Tokens".into(),
            category: "Secrets".into(),
            description: "Match three-part base64-encoded JWTs.".into(),
            example_match: "eyJhbGciOiJI....abc.123".into(),
            rule: Rule {
                name: "wizard_jwt".into(),
                pattern: r"\beyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\b".into(),
                action: RuleAction::BlackBox,
                description: "JWT".into(),
                severity: "critical".into(),
                validator: None,
            },
        },
        WizardPreset {
            id: "ip_address".into(),
            label: "IP addresses (IPv4)".into(),
            category: "Network".into(),
            description: "Match IPv4 addresses. Flag only — often documented legitimately.".into(),
            example_match: "192.168.1.42".into(),
            rule: Rule {
                name: "wizard_ipv4".into(),
                pattern: r"\b(?:\d{1,3}\.){3}\d{1,3}\b".into(),
                action: RuleAction::Flag,
                description: "IPv4 address".into(),
                severity: "low".into(),
                validator: None,
            },
        },
        WizardPreset {
            id: "internal_ticket".into(),
            label: "Internal ticket / ID prefix".into(),
            category: "Custom".into(),
            description: "Match identifiers like ACME-1234 or TKT-987654. Edit after adding.".into(),
            example_match: "ACME-1234".into(),
            rule: Rule {
                name: "wizard_ticket_id".into(),
                pattern: r"\b[A-Z]{2,5}-\d{3,}\b".into(),
                action: RuleAction::Mask { replacement: "[TICKET]".into() },
                description: "Internal ticket ID".into(),
                severity: "low".into(),
                validator: None,
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizard_presets_all_have_valid_regex() {
        for preset in wizard_library() {
            regex::Regex::new(&preset.rule.pattern)
                .unwrap_or_else(|e| panic!("wizard '{}' regex invalid: {e}", preset.id));
        }
    }

    #[test]
    fn wizard_preset_ids_are_unique() {
        let presets = wizard_library();
        let mut seen = std::collections::HashSet::new();
        for p in &presets {
            assert!(seen.insert(p.id.clone()), "duplicate wizard id: {}", p.id);
        }
    }

    #[test]
    fn wizard_preset_names_are_unique() {
        let presets = wizard_library();
        let mut seen = std::collections::HashSet::new();
        for p in &presets {
            assert!(
                seen.insert(p.rule.name.clone()),
                "duplicate wizard rule name: {}",
                p.rule.name
            );
        }
    }

    #[test]
    fn wizard_preset_categories_are_known() {
        let allowed = ["Personal data", "Financial", "Secrets", "Network", "Custom"];
        for p in wizard_library() {
            assert!(
                allowed.contains(&p.category.as_str()),
                "preset {} has unknown category {}",
                p.id,
                p.category
            );
        }
    }
}
```

### 9.2 `src-tauri/src/rules/mod.rs` (REPLACE)

```rust
//! Pluggable rules engine.
//!
//! Rules are JSON-defined; ship a base set, let customers author their own.
//! Each rule is a (pattern, action) pair that can be applied to text (from
//! OCR or input hooks). The engine is intentionally dumb — it doesn't know
//! what "PII" means, only "match this pattern and do this action."
//!
//! The `library` submodule layers a friendly management API on top: pack
//! discovery, custom rule management, wizard presets, import / export.

pub mod engine;
pub mod library;

pub use engine::{Rule, RuleAction, RuleEngine, RulePack};
pub use library::{
    add_custom_rule, custom_rules_path, export_pack, import_pack, list_all, load_custom,
    remove_custom_rule, wizard_library, PackSummary, WizardPreset,
};
```

### 9.3 New commands appended to `src-tauri/src/commands.rs`

Append this block to the existing file (after the last existing command):

```rust
// ─────────────────────────────────────────────────────────────────────────────
//  Rule-pack management (Phase 1 UX restoration)
// ─────────────────────────────────────────────────────────────────────────────

use crate::rules::{
    add_custom_rule, export_pack as lib_export_pack, import_pack as lib_import_pack, list_all,
    load_custom, remove_custom_rule, wizard_library, PackSummary, Rule, RulePack, WizardPreset,
};

/// Return the curated wizard preset library for the UI.
#[tauri::command]
pub async fn rule_wizard_presets() -> Result<Vec<WizardPreset>, String> {
    Ok(wizard_library())
}

/// One match returned by the live regex tester.
#[derive(Debug, Serialize)]
pub struct MatchSpan {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// Live regex tester result.
#[derive(Debug, Serialize)]
pub struct PatternTestResult {
    pub valid: bool,
    pub error: Option<String>,
    pub match_count: usize,
    pub matches: Vec<MatchSpan>,
}

/// Evaluate `pattern` against `sample`, returning matches or a compile error.
/// Used by the live pattern tester in the rule builder UI.
#[tauri::command]
pub async fn rule_pattern_test(
    pattern: String,
    sample: String,
) -> Result<PatternTestResult, String> {
    match regex::Regex::new(&pattern) {
        Ok(re) => {
            let matches: Vec<MatchSpan> = re
                .find_iter(&sample)
                .map(|m| MatchSpan {
                    start: m.start(),
                    end: m.end(),
                    text: m.as_str().to_string(),
                })
                .collect();
            Ok(PatternTestResult {
                valid: true,
                error: None,
                match_count: matches.len(),
                matches,
            })
        }
        Err(e) => Ok(PatternTestResult {
            valid: false,
            error: Some(e.to_string()),
            match_count: 0,
            matches: vec![],
        }),
    }
}

/// Add a rule to the user's custom pack. Pattern is validated before save.
#[tauri::command]
pub async fn custom_rule_add(rule: Rule) -> Result<RulePack, String> {
    add_custom_rule(&AppState::data_root(), rule).map_err(|e| e.to_string())
}

/// Remove a rule by name from the user's custom pack.
#[tauri::command]
pub async fn custom_rule_remove(rule_name: String) -> Result<RulePack, String> {
    remove_custom_rule(&AppState::data_root(), &rule_name).map_err(|e| e.to_string())
}

/// Fetch the user's custom pack as-is.
#[tauri::command]
pub async fn custom_rules_get() -> Result<RulePack, String> {
    load_custom(&AppState::data_root()).map_err(|e| e.to_string())
}

/// Import an external pack JSON file into the user pack dir.
#[tauri::command]
pub async fn rule_pack_import(src_path: PathBuf) -> Result<PackSummary, String> {
    lib_import_pack(&AppState::data_root(), &src_path).map_err(|e| e.to_string())
}

/// Export a pack from the user pack dir or built-in dir to a file on disk.
#[tauri::command]
pub async fn rule_pack_export(file_name: String, dest_path: PathBuf) -> Result<(), String> {
    let summaries = list_all(&AppState::data_root()).map_err(|e| e.to_string())?;
    let summary = summaries
        .into_iter()
        .find(|p| p.file_name == file_name)
        .ok_or_else(|| format!("pack '{file_name}' not found"))?;
    lib_export_pack(&summary.path, &dest_path).map_err(|e| e.to_string())
}
```

> Required existing imports at the top of `commands.rs` (verify these are present, add if not):
> ```rust
> use std::path::PathBuf;
> use serde::Serialize;
> ```

### 9.4 Command registration: `src-tauri/src/lib.rs` diff

Inside `invoke_handler!`, after the last existing entry (before the closing `])`), add:

```rust
            // Phase 1 UX restoration — rule wizard & builder
            commands::rule_wizard_presets,
            commands::rule_pattern_test,
            commands::custom_rule_add,
            commands::custom_rule_remove,
            commands::custom_rules_get,
            commands::rule_pack_import,
            commands::rule_pack_export,
```

### 9.5 New tests appended to `src-tauri/tests/integration.rs`

Append this block at the end of the existing file:

```rust
// ═══ Rule library tests (Sprint 1 — Phase 1 UX restoration) ═══════════════════

use traceflow_lib::rules::{
    add_custom_rule, export_pack, import_pack, load_custom, remove_custom_rule, wizard_library,
    Rule, RuleAction,
};

/// Returns a tempdir-like data root that's unique per test.
fn unique_data_root(label: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("tf-test-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn wizard_presets_all_have_valid_regex() {
    for preset in wizard_library() {
        regex::Regex::new(&preset.rule.pattern)
            .unwrap_or_else(|e| panic!("wizard '{}' regex invalid: {e}", preset.id));
    }
}

#[test]
fn wizard_preset_ids_are_unique() {
    let presets = wizard_library();
    let mut seen = std::collections::HashSet::new();
    for p in &presets {
        assert!(seen.insert(p.id.clone()), "duplicate id: {}", p.id);
    }
}

#[test]
fn custom_rule_add_round_trip() {
    let root = unique_data_root("custom-add");
    let rule = Rule {
        name: "test_rule".into(),
        pattern: r"\bFOO-\d+\b".into(),
        action: RuleAction::Mask {
            replacement: "[FOO]".into(),
        },
        description: "test".into(),
        severity: "low".into(),
        validator: None,
    };
    let pack = add_custom_rule(&root, rule.clone()).expect("add");
    assert_eq!(pack.rules.len(), 1);
    assert_eq!(pack.rules[0].name, "test_rule");

    let reloaded = load_custom(&root).expect("reload");
    assert_eq!(reloaded.rules.len(), 1);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn custom_rule_replaces_same_name() {
    let root = unique_data_root("custom-replace");
    let rule_v1 = Rule {
        name: "dup_name".into(),
        pattern: r"\bA\b".into(),
        action: RuleAction::Flag,
        description: "v1".into(),
        severity: "low".into(),
        validator: None,
    };
    let rule_v2 = Rule {
        name: "dup_name".into(),
        pattern: r"\bB\b".into(),
        action: RuleAction::Flag,
        description: "v2".into(),
        severity: "high".into(),
        validator: None,
    };
    add_custom_rule(&root, rule_v1).expect("add v1");
    let pack = add_custom_rule(&root, rule_v2).expect("add v2");
    assert_eq!(pack.rules.len(), 1, "same name should replace not duplicate");
    assert_eq!(pack.rules[0].description, "v2");

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn custom_rule_rejects_bad_pattern() {
    let root = unique_data_root("custom-bad");
    let rule = Rule {
        name: "bad".into(),
        pattern: r"(?P<unbalanced".into(),
        action: RuleAction::Flag,
        description: "broken".into(),
        severity: "low".into(),
        validator: None,
    };
    let result = add_custom_rule(&root, rule);
    assert!(result.is_err(), "expected invalid pattern to be rejected");

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn custom_rule_remove_is_idempotent() {
    let root = unique_data_root("custom-remove");
    // Remove from empty pack — should not error.
    remove_custom_rule(&root, "nothing_here").expect("remove from empty");

    let rule = Rule {
        name: "x".into(),
        pattern: r"\bx\b".into(),
        action: RuleAction::Flag,
        description: "".into(),
        severity: "low".into(),
        validator: None,
    };
    add_custom_rule(&root, rule).unwrap();
    let pack = remove_custom_rule(&root, "x").unwrap();
    assert!(pack.rules.is_empty());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn rule_pack_export_import_round_trip() {
    let src_root = unique_data_root("export-src");
    let dst_root = unique_data_root("export-dst");

    let rule = Rule {
        name: "roundtrip".into(),
        pattern: r"\bROUND\b".into(),
        action: RuleAction::Flag,
        description: "".into(),
        severity: "low".into(),
        validator: None,
    };
    add_custom_rule(&src_root, rule).unwrap();
    let src_file = src_root.join("rule_packs").join("custom_rules.json");
    assert!(src_file.exists());

    let exported = src_root.join("exported.json");
    export_pack(&src_file, &exported).expect("export");
    assert!(exported.exists());

    let summary = import_pack(&dst_root, &exported).expect("import");
    assert_eq!(summary.name, "custom_rules");
    assert_eq!(summary.rule_count, 1);

    std::fs::remove_dir_all(&src_root).ok();
    std::fs::remove_dir_all(&dst_root).ok();
}
```

### 9.6 Types added to `src/types.ts`

Append:

```typescript
// ─── Phase 1 UX restoration types ───────────────────────────────────────────

export type RuleAction =
  | { type: "blur" }
  | { type: "black_box" }
  | { type: "drop" }
  | { type: "mask"; replacement: string }
  | { type: "flag" };

export interface Rule {
  name: string;
  pattern: string;
  action: RuleAction;
  description: string;
  severity: "low" | "medium" | "high" | "critical";
  validator?: string | null;
}

export interface RulePack {
  name: string;
  version: string;
  description: string;
  rules: Rule[];
}

export interface PackSummary {
  file_name: string;
  name: string;
  version: string;
  description: string;
  rule_count: number;
  is_builtin: boolean;
  path: string;
}

export interface WizardPreset {
  id: string;
  label: string;
  category: string;
  description: string;
  example_match: string;
  rule: Rule;
}

export interface PatternTestResult {
  valid: boolean;
  error: string | null;
  match_count: number;
  matches: { start: number; end: number; text: string }[];
}
```

### 9.7 Updated `src/components/SettingsPanel.tsx`

Drop-in replacement (assumes existing imports for `invoke`, `useState`, `useEffect`, dialog plugin — keep them; this is a structural update, not a full file):

```typescript
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { PackSummary, Rule, RulePack, WizardPreset } from "../types";
import WizardGrid from "./settings/WizardGrid";
import RuleBuilderForm from "./settings/RuleBuilderForm";
import CustomRuleList from "./settings/CustomRuleList";

type Mode = "list" | "wizard" | "builder";

export default function SettingsPanel() {
  const [mode, setMode] = useState<Mode>("list");
  const [packs, setPacks] = useState<PackSummary[]>([]);
  const [active, setActive] = useState<string[]>([]);
  const [customPack, setCustomPack] = useState<RulePack | null>(null);
  const [presets, setPresets] = useState<WizardPreset[]>([]);
  const [editPreset, setEditPreset] = useState<WizardPreset | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    try {
      const [p, a, c, w] = await Promise.all([
        invoke<PackSummary[]>("list_rule_packs"),
        invoke<string[]>("rule_packs_active"),
        invoke<RulePack>("custom_rules_get"),
        invoke<WizardPreset[]>("rule_wizard_presets"),
      ]);
      setPacks(p);
      setActive(a);
      setCustomPack(c);
      setPresets(w);
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const togglePack = async (fileName: string) => {
    try {
      await invoke("toggle_rule_pack", { fileName });
      await refresh();
    } catch (e) {
      alert(`Could not toggle pack: ${e}`);
    }
  };

  const handleImport = async () => {
    const path = await open({
      title: "Import rule pack",
      filters: [{ name: "Rule pack (JSON)", extensions: ["json"] }],
    });
    if (!path || typeof path !== "string") return;
    setBusy(true);
    try {
      await invoke("rule_pack_import", { srcPath: path });
      await refresh();
    } catch (e) {
      alert(`Import failed: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const handleExport = async (pack: PackSummary) => {
    const path = await save({
      title: "Export rule pack",
      defaultPath: pack.file_name,
      filters: [{ name: "Rule pack (JSON)", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      await invoke("rule_pack_export", {
        fileName: pack.file_name,
        destPath: path,
      });
    } catch (e) {
      alert(`Export failed: ${e}`);
    }
  };

  const handleAddRule = async (rule: Rule) => {
    try {
      const next = await invoke<RulePack>("custom_rule_add", { rule });
      setCustomPack(next);
      setMode("list");
      setEditPreset(null);
    } catch (e) {
      alert(`Could not save rule: ${e}`);
    }
  };

  const handleRemoveRule = async (name: string) => {
    if (!confirm(`Remove rule "${name}"?`)) return;
    try {
      const next = await invoke<RulePack>("custom_rule_remove", { ruleName: name });
      setCustomPack(next);
    } catch (e) {
      alert(`Could not remove rule: ${e}`);
    }
  };

  if (mode === "builder") {
    return (
      <RuleBuilderForm
        initial={editPreset?.rule ?? null}
        onCancel={() => {
          setMode("list");
          setEditPreset(null);
        }}
        onSubmit={handleAddRule}
      />
    );
  }

  if (mode === "wizard") {
    return (
      <WizardGrid
        presets={presets}
        onPick={(p) => {
          setEditPreset(p);
          setMode("builder");
        }}
        onCancel={() => setMode("list")}
      />
    );
  }

  return (
    <div className="settings-panel">
      <h2 className="section-title">Redaction rules</h2>
      <p className="helper">
        Rule packs define what gets redacted from captured screenshots and event text.
        Toggle the packs you want active. Built-in packs are read-only — add your own
        rules below or import a pack from a colleague.
      </p>

      <section className="settings-section">
        <h3 className="section-eyebrow">Available packs</h3>
        <div className="pack-list">
          {packs.map((p) => {
            const isActive = active.includes(p.file_name);
            return (
              <div key={p.file_name} className={`pack-row ${isActive ? "active" : ""}`}>
                <div className="pack-info">
                  <div className="pack-name">
                    {p.name}
                    <span className={`pack-badge ${p.is_builtin ? "builtin" : "user"}`}>
                      {p.is_builtin ? "BUILT-IN" : "CUSTOM"}
                    </span>
                  </div>
                  {p.description && <div className="pack-desc helper">{p.description}</div>}
                  <div className="pack-meta">
                    v{p.version} · {p.rule_count} rule{p.rule_count === 1 ? "" : "s"}
                  </div>
                </div>
                <div className="pack-actions">
                  <button className="btn-tiny" onClick={() => togglePack(p.file_name)}>
                    {isActive ? "Disable" : "Enable"}
                  </button>
                  <button className="btn-tiny" onClick={() => handleExport(p)}>
                    Export
                  </button>
                </div>
              </div>
            );
          })}
        </div>
        <div className="action-row">
          <button className="btn btn-ghost" onClick={handleImport} disabled={busy}>
            ↥ Import pack from file
          </button>
        </div>
      </section>

      <section className="settings-section">
        <h3 className="section-eyebrow">Your custom rules</h3>
        <CustomRuleList pack={customPack} onRemove={handleRemoveRule} />
        <div className="action-row">
          <button
            className="btn btn-primary"
            onClick={() => {
              setEditPreset(null);
              setMode("wizard");
            }}
          >
            ✨ Add with wizard
          </button>
          <button
            className="btn btn-ghost"
            onClick={() => {
              setEditPreset(null);
              setMode("builder");
            }}
          >
            ⚙ Advanced builder
          </button>
        </div>
      </section>
    </div>
  );
}
```

### 9.8 New: `src/components/settings/WizardGrid.tsx`

```typescript
import { useMemo } from "react";
import type { WizardPreset } from "../../types";

interface Props {
  presets: WizardPreset[];
  onPick: (preset: WizardPreset) => void;
  onCancel: () => void;
}

export default function WizardGrid({ presets, onPick, onCancel }: Props) {
  const grouped = useMemo(() => {
    const m = new Map<string, WizardPreset[]>();
    for (const p of presets) {
      const list = m.get(p.category) ?? [];
      list.push(p);
      m.set(p.category, list);
    }
    return Array.from(m.entries());
  }, [presets]);

  return (
    <div className="wizard">
      <h2 className="section-title">Rule wizard</h2>
      <p className="helper">
        Pick a common pattern to start from. Each preset is a working rule with a
        tested regex. You can tweak it on the next screen before saving.
      </p>

      {grouped.map(([category, items]) => (
        <section key={category} className="wizard-section">
          <div className="wizard-category">{category}</div>
          <div className="wizard-grid">
            {items.map((p) => (
              <button key={p.id} className="wizard-card" onClick={() => onPick(p)}>
                <div className="wizard-card-title">{p.label}</div>
                <div className="wizard-card-desc">{p.description}</div>
                <div className="wizard-card-example">
                  <span>matches</span> <code>{p.example_match}</code>
                </div>
              </button>
            ))}
          </div>
        </section>
      ))}

      <div className="action-row">
        <button className="btn btn-ghost" onClick={onCancel}>
          ← Back
        </button>
      </div>
    </div>
  );
}
```

### 9.9 New: `src/components/settings/RuleBuilderForm.tsx`

```typescript
import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PatternTestResult, Rule, RuleAction } from "../../types";

interface Props {
  initial: Rule | null;
  onCancel: () => void;
  onSubmit: (rule: Rule) => void;
}

type ActionType = RuleAction["type"];

const SEVERITIES: Rule["severity"][] = ["low", "medium", "high", "critical"];

const ACTION_OPTIONS: { value: ActionType; label: string; help: string }[] = [
  {
    value: "mask",
    label: "Mask with replacement text",
    help: "Most common. Replace the matched text in event logs with a placeholder.",
  },
  {
    value: "black_box",
    label: "Black box (redact)",
    help: "Solid black rectangle drawn over the match. Use for highly sensitive data.",
  },
  {
    value: "blur",
    label: "Blur the area",
    help: "Gaussian blur. Less harsh than a black box, but still unreadable.",
  },
  {
    value: "drop",
    label: "Drop the event entirely",
    help: "Don't even log the event. Use when the text itself must never be persisted.",
  },
  {
    value: "flag",
    label: "Flag only (don't redact)",
    help: "Record a flag in the audit log but leave the content untouched.",
  },
];

export default function RuleBuilderForm({ initial, onCancel, onSubmit }: Props) {
  const [name, setName] = useState(initial?.name ?? "");
  const [pattern, setPattern] = useState(initial?.pattern ?? "");
  const [description, setDescription] = useState(initial?.description ?? "");
  const [severity, setSeverity] = useState<Rule["severity"]>(initial?.severity ?? "medium");
  const [actionType, setActionType] = useState<ActionType>(initial?.action.type ?? "mask");
  const [maskReplacement, setMaskReplacement] = useState(
    initial?.action.type === "mask" ? initial.action.replacement : "[REDACTED]"
  );
  const [validator, setValidator] = useState<string>(initial?.validator ?? "");
  const [sample, setSample] = useState("");
  const [test, setTest] = useState<PatternTestResult | null>(null);

  useEffect(() => {
    if (!pattern) {
      setTest(null);
      return;
    }
    const timer = setTimeout(() => {
      invoke<PatternTestResult>("rule_pattern_test", { pattern, sample })
        .then(setTest)
        .catch(() => setTest(null));
    }, 200);
    return () => clearTimeout(timer);
  }, [pattern, sample]);

  const canSave = useMemo(() => {
    if (!name.trim() || !pattern.trim()) return false;
    if (test && !test.valid) return false;
    if (actionType === "mask" && !maskReplacement.trim()) return false;
    return true;
  }, [name, pattern, test, actionType, maskReplacement]);

  const buildAction = (): RuleAction => {
    switch (actionType) {
      case "mask":
        return { type: "mask", replacement: maskReplacement };
      case "blur":
        return { type: "blur" };
      case "black_box":
        return { type: "black_box" };
      case "drop":
        return { type: "drop" };
      case "flag":
        return { type: "flag" };
    }
  };

  const handleSave = () => {
    const rule: Rule = {
      name: name.trim().replace(/\s+/g, "_").toLowerCase(),
      pattern,
      action: buildAction(),
      description: description.trim(),
      severity,
      validator: validator.trim() || null,
    };
    onSubmit(rule);
  };

  return (
    <div className="rule-builder">
      <h2 className="section-title">{initial ? "Edit rule" : "New custom rule"}</h2>

      <div className="field">
        <label htmlFor="rule-name">Rule name</label>
        <input
          id="rule-name"
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. employee_id"
        />
        <span className="helper">
          Used internally for audit logs. Lowercase, underscores only.
        </span>
      </div>

      <div className="field">
        <label htmlFor="rule-pattern">Regex pattern</label>
        <input
          id="rule-pattern"
          type="text"
          value={pattern}
          onChange={(e) => setPattern(e.target.value)}
          placeholder="e.g. \\bEMP-\\d{6}\\b"
          className="mono-input"
        />
        {test && !test.valid && <div className="pattern-error">⚠ {test.error}</div>}
        {test && test.valid && (
          <div className="pattern-ok">
            ✓ Valid pattern
            {sample && ` · ${test.match_count} match${test.match_count === 1 ? "" : "es"} in sample`}
          </div>
        )}
      </div>

      <div className="field">
        <label htmlFor="rule-sample">Test against sample text (optional)</label>
        <textarea
          id="rule-sample"
          rows={3}
          value={sample}
          onChange={(e) => setSample(e.target.value)}
          placeholder="Paste some text to see what your pattern matches…"
          className="mono-input"
        />
        {test?.valid && test.matches.length > 0 && (
          <div className="match-preview">
            {test.matches.slice(0, 5).map((m, i) => (
              <span key={i} className="match-chip">
                "{m.text}"
              </span>
            ))}
            {test.matches.length > 5 && (
              <span className="helper">+{test.matches.length - 5} more</span>
            )}
          </div>
        )}
      </div>

      <div className="field">
        <label htmlFor="rule-description">Description (optional)</label>
        <input
          id="rule-description"
          type="text"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="What does this rule protect?"
        />
      </div>

      <div className="field">
        <label htmlFor="rule-action">When matched</label>
        <select
          id="rule-action"
          value={actionType}
          onChange={(e) => setActionType(e.target.value as ActionType)}
        >
          {ACTION_OPTIONS.map((a) => (
            <option key={a.value} value={a.value}>
              {a.label}
            </option>
          ))}
        </select>
        <span className="helper">
          {ACTION_OPTIONS.find((a) => a.value === actionType)?.help}
        </span>
      </div>

      {actionType === "mask" && (
        <div className="field">
          <label htmlFor="rule-replacement">Replacement text</label>
          <input
            id="rule-replacement"
            type="text"
            value={maskReplacement}
            onChange={(e) => setMaskReplacement(e.target.value)}
            placeholder="[REDACTED]"
          />
        </div>
      )}

      <div className="field">
        <label>Severity</label>
        <div className="radio-row">
          {SEVERITIES.map((s) => (
            <label key={s} className={`radio-pill ${severity === s ? "active" : ""}`}>
              <input
                type="radio"
                name="severity"
                value={s}
                checked={severity === s}
                onChange={() => setSeverity(s)}
              />
              {s}
            </label>
          ))}
        </div>
      </div>

      <details className="advanced-toggle">
        <summary>Advanced</summary>
        <div className="field">
          <label htmlFor="rule-validator">Validator (optional)</label>
          <select
            id="rule-validator"
            value={validator}
            onChange={(e) => setValidator(e.target.value)}
          >
            <option value="">None</option>
            <option value="luhn">Luhn checksum (credit cards)</option>
          </select>
          <span className="helper">
            Validators run after the regex matches, to reduce false positives.
          </span>
        </div>
      </details>

      <div className="action-row">
        <button className="btn btn-ghost" onClick={onCancel}>
          Cancel
        </button>
        <button className="btn btn-primary" onClick={handleSave} disabled={!canSave}>
          {initial ? "Save rule" : "Add rule"}
        </button>
      </div>
    </div>
  );
}
```

### 9.10 New: `src/components/settings/CustomRuleList.tsx`

```typescript
import type { Rule, RulePack } from "../../types";

interface Props {
  pack: RulePack | null;
  onRemove: (name: string) => void;
}

export default function CustomRuleList({ pack, onRemove }: Props) {
  if (!pack || pack.rules.length === 0) {
    return (
      <div className="empty-inline helper">
        No custom rules yet. Use the wizard for common patterns, or the
        advanced builder for your own.
      </div>
    );
  }

  return (
    <div className="custom-rule-list">
      {pack.rules.map((r) => (
        <div key={r.name} className="custom-rule-row">
          <div>
            <div className="custom-rule-name">{r.name}</div>
            <div className="custom-rule-pattern">
              <code>{r.pattern}</code>
            </div>
            <div className="pack-meta">
              {actionLabel(r.action)} · {r.severity}
            </div>
          </div>
          <button className="btn-tiny btn-tiny-danger" onClick={() => onRemove(r.name)}>
            Remove
          </button>
        </div>
      ))}
    </div>
  );
}

function actionLabel(a: Rule["action"]): string {
  switch (a.type) {
    case "blur":
      return "Blur";
    case "black_box":
      return "Redact (black box)";
    case "drop":
      return "Drop event";
    case "mask":
      return `Mask → ${a.replacement}`;
    case "flag":
      return "Flag only";
  }
}
```

### 9.11 Styles appended to `src/styles.css`

```css
/* ════════════════════════════════════════════════════════════════════════
   Sprint 1 — Phase 1 UX restoration
   Wizard, rule builder, custom rule list, pack management
   ════════════════════════════════════════════════════════════════════════ */

.settings-panel { padding: 8px 4px 24px; }
.settings-section { margin-top: 28px; }
.action-row {
  display: flex;
  gap: 10px;
  margin-top: 14px;
  flex-wrap: wrap;
}

/* ── Pack list ────────────────────────────────────────────────────────────── */
.pack-list { display: flex; flex-direction: column; gap: 8px; }
.pack-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border: 1px solid var(--rule);
  border-radius: 3px;
  background: var(--paper);
  transition: border-color 0.15s;
}
.pack-row.active {
  border-color: var(--accent);
  background: linear-gradient(0deg, rgba(195, 60, 31, 0.03), transparent);
}
.pack-info { flex: 1; }
.pack-name {
  display: flex; align-items: center; gap: 10px;
  font-size: 14px; font-weight: 500; color: var(--ink);
}
.pack-badge {
  font-family: var(--mono);
  font-size: 9px; letter-spacing: 0.14em;
  padding: 2px 6px; border-radius: 2px;
}
.pack-badge.builtin { background: var(--paper-deep); color: var(--ink-mute); border: 1px solid var(--rule); }
.pack-badge.user { background: var(--accent); color: var(--paper); }
.pack-desc { margin-top: 4px; font-size: 12px; }
.pack-meta {
  margin-top: 4px;
  font-family: var(--mono); font-size: 10px; color: var(--ink-mute); letter-spacing: 0.06em;
}
.pack-actions { display: flex; gap: 6px; margin-left: 14px; }

.btn-tiny {
  background: transparent; border: 1px solid var(--rule); border-radius: 2px;
  padding: 4px 10px; font-family: var(--mono); font-size: 10px; letter-spacing: 0.1em;
  color: var(--ink-soft); cursor: pointer; text-transform: uppercase;
}
.btn-tiny:hover { border-color: var(--ink); color: var(--ink); }
.btn-tiny-danger:hover { border-color: var(--accent); color: var(--accent); }

/* ── Custom rules ─────────────────────────────────────────────────────────── */
.custom-rule-list { display: flex; flex-direction: column; gap: 8px; }
.custom-rule-row {
  display: flex; align-items: center; justify-content: space-between;
  padding: 12px 14px; background: var(--paper);
  border: 1px solid var(--rule); border-radius: 3px;
}
.custom-rule-name {
  font-family: var(--mono); font-size: 12px; color: var(--ink); font-weight: 500;
}
.custom-rule-pattern {
  margin-top: 4px; font-family: var(--mono); font-size: 11px;
  color: var(--ink-soft); word-break: break-all;
}
.custom-rule-pattern code {
  background: var(--paper-deep); padding: 2px 6px; border-radius: 2px;
}
.empty-inline {
  padding: 18px; background: var(--paper-deep);
  border: 1px dashed var(--rule); border-radius: 3px;
  font-size: 13px; text-align: center;
}

/* ── Rule builder ─────────────────────────────────────────────────────────── */
.rule-builder .field { margin-bottom: 18px; }
.mono-input { font-family: var(--mono) !important; font-size: 13px !important; }
.pattern-error {
  margin-top: 6px; padding: 6px 10px;
  background: rgba(195, 60, 31, 0.08); border-left: 2px solid var(--accent);
  font-family: var(--mono); font-size: 11px; color: var(--accent);
}
.pattern-ok {
  margin-top: 6px; font-family: var(--mono); font-size: 11px; color: var(--good);
}
.match-preview {
  margin-top: 8px; display: flex; flex-wrap: wrap; gap: 6px; align-items: center;
}
.match-chip {
  font-family: var(--mono); font-size: 11px;
  background: rgba(195, 60, 31, 0.08); color: var(--accent);
  padding: 3px 8px; border-radius: 2px;
}
.radio-row { display: flex; gap: 6px; flex-wrap: wrap; }
.radio-pill {
  display: inline-flex; align-items: center;
  padding: 6px 14px; border: 1px solid var(--rule); border-radius: 99px;
  cursor: pointer; font-size: 12px; text-transform: capitalize;
  background: var(--paper); transition: all 0.15s;
}
.radio-pill input[type="radio"] { display: none; }
.radio-pill.active { background: var(--ink); color: var(--paper); border-color: var(--ink); }
.advanced-toggle { margin-top: 6px; padding: 8px 0; }
.advanced-toggle summary {
  font-family: var(--mono); font-size: 10px;
  text-transform: uppercase; letter-spacing: 0.18em;
  color: var(--ink-mute); cursor: pointer;
}

/* ── Wizard ───────────────────────────────────────────────────────────────── */
.wizard-section { margin-bottom: 24px; }
.wizard-category {
  font-family: var(--mono); font-size: 10px;
  text-transform: uppercase; letter-spacing: 0.18em;
  color: var(--ink-mute); margin-bottom: 10px;
}
.wizard-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 10px;
}
.wizard-card {
  text-align: left; padding: 14px;
  background: var(--paper); border: 1px solid var(--rule);
  border-radius: 3px; cursor: pointer;
  transition: all 0.15s; font-family: var(--sans);
}
.wizard-card:hover {
  border-color: var(--accent); transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05);
}
.wizard-card-title { font-size: 14px; font-weight: 500; color: var(--ink); margin-bottom: 6px; }
.wizard-card-desc { font-size: 12px; color: var(--ink-soft); line-height: 1.4; margin-bottom: 10px; }
.wizard-card-example {
  font-family: var(--mono); font-size: 10px;
  color: var(--ink-mute); letter-spacing: 0.04em;
}
.wizard-card-example code {
  color: var(--accent); background: rgba(195, 60, 31, 0.06);
  padding: 1px 5px; border-radius: 2px; margin-left: 4px;
}
```

---

## 10. Verification script

A copy-paste smoke script after implementing. Save as `scripts/sprint1-verify.sh` (or run by hand):

```bash
#!/usr/bin/env bash
set -e
echo "── Sprint 1 verification ─────────────────────────────"

cd src-tauri

echo "→ cargo fmt"
cargo fmt --check

echo "→ cargo clippy"
cargo clippy --all-targets --all-features -- -D warnings

echo "→ cargo test"
cargo test

cd ..
echo "→ npm run build"
npm run build

echo "✓ All sprint 1 verifications passed."
```

Run with:
```bash
bash scripts/sprint1-verify.sh
```

---

## Appendix: Standards mapping

Each piece of this sprint is required to satisfy a section of `ENGINEERING_STANDARDS.md`. Quick mapping:

| Sprint item | Standard |
|---|---|
| `library.rs` doc comments | §14.1 |
| `anyhow::Result` + `.context()` everywhere | §5.1 |
| Tests for every new IPC command | §7.3 |
| Pattern validation before save | §8.3 |
| No `unwrap()` outside tests | §5.1 |
| No `any` in TypeScript | §4.1 |
| User-facing error translation | §5.3 |
| `useEffect` cleanup function | §4.7 |
| Pack files = configuration, not code | §2.4 |

If you find yourself wanting to deviate from any of these, file an exception per §18.

---

*End of Sprint 1 spec. Acknowledge by ticking "I have read SPRINT_1_UX_RESTORATION.md and will implement it as written" in the PR.*
