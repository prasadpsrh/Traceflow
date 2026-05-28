// Rule engine implementation.
//
// A `Rule` has a name, a regex pattern, an action, and optional metadata
// (severity, description, examples). A `RulePack` is a versioned collection
// of rules. A `RuleEngine` is a compiled, ready-to-evaluate stack of packs.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    /// Blur the pixel region (image-level).
    Blur,
    /// Solid black box (image-level).
    BlackBox,
    /// Drop the event entirely (text-level — never log it).
    Drop,
    /// Mask matched text with the replacement (text-level).
    Mask { replacement: String },
    /// Just flag — record an event but don't redact.
    Flag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub pattern: String,
    pub action: RuleAction,
    #[serde(default)]
    pub description: String,
    /// "low" | "medium" | "high" | "critical"
    #[serde(default = "default_severity")]
    pub severity: String,
    /// Validator hook: "luhn" requires a digit run to pass Luhn check.
    #[serde(default)]
    pub validator: Option<String>,
}

fn default_severity() -> String {
    "medium".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulePack {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub rules: Vec<Rule>,
}

/// One match.
#[derive(Debug, Clone)]
pub struct Hit {
    pub rule_name: String,
    pub action: RuleAction,
    pub start: usize,
    pub end: usize,
    pub severity: String,
}

#[derive(Debug, Clone)]
struct CompiledRule {
    name: String,
    re: Regex,
    action: RuleAction,
    severity: String,
    validator: Option<String>,
}

/// Compiled rule engine — combines one or more rule packs into a single
/// evaluator. Construction is fallible; evaluation is infallible.
pub struct RuleEngine {
    rules: Vec<CompiledRule>,
    pack_names: Vec<String>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            pack_names: Vec::new(),
        }
    }

    /// Load a pack from a JSON file on disk.
    pub fn load_pack_file(&mut self, path: &Path) -> Result<()> {
        let bytes =
            std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let pack: RulePack = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing {}", path.display()))?;
        self.load_pack(pack)
    }

    /// Load a pack already in memory.
    pub fn load_pack(&mut self, pack: RulePack) -> Result<()> {
        for r in &pack.rules {
            let re = Regex::new(&r.pattern)
                .with_context(|| format!("compiling rule {}.{}", pack.name, r.name))?;
            self.rules.push(CompiledRule {
                name: format!("{}/{}", pack.name, r.name),
                re,
                action: r.action.clone(),
                severity: r.severity.clone(),
                validator: r.validator.clone(),
            });
        }
        self.pack_names.push(format!("{}@{}", pack.name, pack.version));
        Ok(())
    }

    /// Evaluate `text` and return every hit (in order of rules loaded).
    /// Validators run after the regex match. Currently supported:
    ///   - "luhn"  → matched digits must pass Luhn check
    pub fn evaluate(&self, text: &str) -> Vec<Hit> {
        let mut hits = Vec::new();
        for r in &self.rules {
            for m in r.re.find_iter(text) {
                if let Some(v) = &r.validator {
                    if v == "luhn" {
                        let digits: String =
                            m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
                        if !luhn_valid(&digits) {
                            continue;
                        }
                    }
                }
                hits.push(Hit {
                    rule_name: r.name.clone(),
                    action: r.action.clone(),
                    start: m.start(),
                    end: m.end(),
                    severity: r.severity.clone(),
                });
            }
        }
        hits
    }

    pub fn pack_names(&self) -> &[String] {
        &self.pack_names
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn luhn_valid(digits: &str) -> bool {
    let n = digits.len();
    if !(13..=19).contains(&n) {
        return false;
    }
    let mut sum = 0u32;
    for (i, c) in digits.chars().rev().enumerate() {
        let mut d = c.to_digit(10).unwrap_or(0);
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(rules: Vec<Rule>) -> RulePack {
        RulePack {
            name: "test".into(),
            version: "1".into(),
            description: String::new(),
            rules,
        }
    }

    #[test]
    fn email_rule_matches() {
        let mut e = RuleEngine::new();
        e.load_pack(pack(vec![Rule {
            name: "email".into(),
            pattern: r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b".into(),
            action: RuleAction::Mask {
                replacement: "[REDACTED]".into(),
            },
            description: String::new(),
            severity: "medium".into(),
            validator: None,
        }]))
        .unwrap();
        let hits = e.evaluate("contact alice@example.com today");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rule_name, "test/email");
    }

    #[test]
    fn luhn_validator_works() {
        let mut e = RuleEngine::new();
        e.load_pack(pack(vec![Rule {
            name: "card".into(),
            pattern: r"\b(?:\d[ -]*?){13,19}\b".into(),
            action: RuleAction::BlackBox,
            description: String::new(),
            severity: "high".into(),
            validator: Some("luhn".into()),
        }]))
        .unwrap();
        // Valid Luhn (Visa test card)
        assert_eq!(e.evaluate("card 4111 1111 1111 1111").len(), 1);
        // Random 16 digits fail Luhn
        assert_eq!(e.evaluate("card 1234 5678 9012 3456").len(), 0);
    }
}
