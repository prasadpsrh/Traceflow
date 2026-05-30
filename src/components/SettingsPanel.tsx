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