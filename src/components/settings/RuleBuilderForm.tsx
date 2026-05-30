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