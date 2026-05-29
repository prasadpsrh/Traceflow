// Pluggable rules engine.
//
// Rules are JSON-defined; ship a base set, let customers author their own.
// Each rule is a (pattern, action) pair that can be applied to text (from OCR
// or input hooks). The engine is intentionally dumb — it doesn't know
// what "PII" means, only "match this pattern and do this action."
//
// Industries customize by writing their own rule pack:
//   - rule_packs/healthcare_phi.json
//   - rule_packs/financial.json
//   - rule_packs/secrets.json
// Customers stack any combination via traceflow.config.toml.

pub mod engine;
