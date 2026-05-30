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