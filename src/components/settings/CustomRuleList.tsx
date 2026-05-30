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