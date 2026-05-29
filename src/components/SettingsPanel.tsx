import { FormEvent, useMemo, useState } from "react";

export interface RulePackAction {
  type: "mask" | "black_box" | "blur" | "drop" | "flag";
  replacement?: string;
}

export interface RuleDefinition {
  name: string;
  pattern: string;
  action: RulePackAction;
  description: string;
  severity: string;
  validator?: string;
}

export interface RulePackForm {
  name: string;
  version: string;
  description: string;
  rules: RuleDefinition[];
}

export interface RulePackSummary {
  name: string;
  version: string;
  description: string;
  path: string;
  enabled: boolean;
}

interface SettingsPanelProps {
  rulePacks: RulePackSummary[];
  onSaveRulePack: (pack: RulePackForm) => Promise<void>;
  onTogglePack: (path: string, enabled: boolean) => Promise<void>;
  onRefreshPacks: () => void;
}

const defaultRulePack: RulePackForm = {
  name: "custom_pii",
  version: "1.0",
  description: "Custom user-defined rule pack",
  rules: [
    {
      name: "Sensitive pattern",
      pattern: "",
      action: { type: "mask", replacement: "[REDACTED]" },
      description: "Mask matching sensitive text.",
      severity: "warning",
      validator: "",
    },
  ],
};

export default function SettingsPanel({ rulePacks, onSaveRulePack, onTogglePack, onRefreshPacks }: SettingsPanelProps) {
  const [form, setForm] = useState<RulePackForm>(defaultRulePack);
  const [saving, setSaving] = useState(false);

  const currentRule = form.rules[0];

  const canSave = useMemo(
    () =>
      form.name.trim().length > 0 &&
      form.version.trim().length > 0 &&
      currentRule.name.trim().length > 0 &&
      currentRule.pattern.trim().length > 0,
    [form, currentRule]
  );

  const handleField = (field: keyof RulePackForm, value: string) => {
    setForm((current) => ({ ...current, [field]: value }));
  };

  const handleRuleField = (field: keyof RuleDefinition, value: string) => {
    setForm((current) => ({
      ...current,
      rules: [
        {
          ...current.rules[0],
          [field]: value,
        },
      ],
    }));
  };

  const handleActionType = (value: RulePackAction["type"]) => {
    setForm((current) => ({
      ...current,
      rules: [
        {
          ...current.rules[0],
          action: { type: value, replacement: current.rules[0].action.replacement },
        },
      ],
    }));
  };

  const handleSave = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!canSave) return;

    setSaving(true);
    try {
      await onSaveRulePack(form);
      setForm(defaultRulePack);
    } finally {
      setSaving(false);
    }
  };

  return (
    <section className="settings-panel">
      <div className="section-eyebrow">Settings</div>
      <h2 className="section-title">Rule packs & builder</h2>
      <p className="section-copy">
        Manage built-in and custom rule packs, then use the quick builder to create a
        new pattern-based rule.
      </p>

      <div className="settings-grid">
        <div className="settings-card">
          <div className="card-header">
            <div>
              <h3>Installed rule packs</h3>
              <p className="helper">Each JSON pack is scanned from configured rule-pack directories.</p>
            </div>
            <button type="button" className="secondary" onClick={onRefreshPacks}>
              Refresh
            </button>
          </div>
          {rulePacks.length === 0 ? (
            <div className="empty-card">No rule packs found yet.</div>
          ) : (
            <div className="rule-pack-list">
              {rulePacks.map((pack) => (
                <div className="rule-pack-item" key={pack.path}>
                  <div>
                    <strong>{pack.name}</strong> <span className="muted">v{pack.version}</span>
                    <div className="rule-pack-description">{pack.description}</div>
                  </div>
                  <div className="rule-pack-actions">
                    <div className={`status-pill ${pack.enabled ? "enabled" : "disabled"}`}>
                      {pack.enabled ? "Enabled" : "Available"}
                    </div>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => onTogglePack(pack.path, !pack.enabled)}
                    >
                      {pack.enabled ? "Disable" : "Enable"}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="settings-card">
          <div className="card-header">
            <div>
              <h3>Quick rule builder</h3>
              <p className="helper">Create a new rule pack with one pattern and save it to user space.</p>
            </div>
          </div>

          <form className="rule-builder" onSubmit={handleSave}>
            <label>
              Pack name
              <input
                value={form.name}
                onChange={(event) => handleField("name", event.target.value)}
                placeholder="custom_pii"
              />
            </label>
            <label>
              Version
              <input
                value={form.version}
                onChange={(event) => handleField("version", event.target.value)}
                placeholder="1.0"
              />
            </label>
            <label>
              Description
              <input
                value={form.description}
                onChange={(event) => handleField("description", event.target.value)}
                placeholder="Mask user-defined sensitive text"
              />
            </label>

            <fieldset className="rule-section">
              <legend>Rule definition</legend>
              <label>
                Rule name
                <input
                  value={currentRule.name}
                  onChange={(event) => handleRuleField("name", event.target.value)}
                />
              </label>
              <label>
                Match pattern
                <input
                  value={currentRule.pattern}
                  onChange={(event) => handleRuleField("pattern", event.target.value)}
                  placeholder="(?i)password|secret"
                />
              </label>
              <div className="field-row">
                <label>
                  Action
                  <select
                    value={currentRule.action.type}
                    onChange={(event) => handleActionType(event.target.value as RulePackAction["type"])}
                  >
                    <option value="mask">Mask</option>
                    <option value="black_box">Black box</option>
                    <option value="blur">Blur</option>
                    <option value="drop">Drop</option>
                    <option value="flag">Flag</option>
                  </select>
                </label>
                <label>
                  Replacement text
                  <input
                    value={currentRule.action.replacement ?? ""}
                    onChange={(event) =>
                      setForm((current) => ({
                        ...current,
                        rules: [
                          {
                            ...current.rules[0],
                            action: {
                              ...current.rules[0].action,
                              replacement: event.target.value,
                            },
                          },
                        ],
                      }))
                    }
                    disabled={currentRule.action.type !== "mask"}
                    placeholder="[REDACTED]"
                  />
                </label>
              </div>
              <label>
                Severity
                <input
                  value={currentRule.severity}
                  onChange={(event) => handleRuleField("severity", event.target.value)}
                />
              </label>
              <label>
                Rule description
                <textarea
                  rows={3}
                  value={currentRule.description}
                  onChange={(event) => handleRuleField("description", event.target.value)}
                />
              </label>
            </fieldset>

            <div className="form-actions">
              <button type="submit" className="primary" disabled={!canSave || saving}>
                {saving ? "Saving…" : "Save rule pack"}
              </button>
            </div>
          </form>
        </div>
      </div>
    </section>
  );
}
