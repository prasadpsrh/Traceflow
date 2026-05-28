import { ExportFormat } from "../App";

interface Props {
  stepCount: number;
  disabled: boolean;
  onExport: (fmt: ExportFormat) => void;
  onVerify: () => void;
  verifyMsg: string | null;
}

export default function ExportPanel({
  stepCount,
  disabled,
  onExport,
  onVerify,
  verifyMsg,
}: Props) {
  const fmts: { id: ExportFormat; label: string }[] = [
    { id: "docx", label: ".docx (Word)" },
    { id: "md", label: ".md (Markdown)" },
    { id: "html", label: ".html (Web)" },
    { id: "json", label: ".json (Audit)" },
  ];

  return (
    <section>
      <div className="section-eyebrow">Export</div>
      <h3 className="section-title" style={{ fontSize: 22, marginBottom: 18 }}>
        Render document
      </h3>

      <p className="helper" style={{ marginBottom: 14 }}>
        The event log is the source of truth — every export is a fresh
        projection. {stepCount} step{stepCount === 1 ? "" : "s"} captured.
      </p>

      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {fmts.map((f) => (
          <button
            key={f.id}
            className="btn btn-ghost btn-block"
            onClick={() => onExport(f.id)}
            disabled={disabled}
          >
            ↓ Export {f.label}
          </button>
        ))}
      </div>

      <div style={{ marginTop: 24 }}>
        <div className="section-eyebrow">Audit</div>
        <button
          className="btn btn-ghost btn-block"
          onClick={onVerify}
          disabled={!stepCount}
          style={{ marginTop: 8 }}
        >
          ✓ Verify chain integrity
        </button>
        {verifyMsg && (
          <p
            className="helper"
            style={{
              marginTop: 10,
              fontFamily: "var(--mono)",
              fontSize: 11,
              color: verifyMsg.startsWith("chain intact") ? "var(--good)" : "var(--accent)",
            }}
          >
            {verifyMsg}
          </p>
        )}
      </div>
    </section>
  );
}
