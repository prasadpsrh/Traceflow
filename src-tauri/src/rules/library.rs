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
