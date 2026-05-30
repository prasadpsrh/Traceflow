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