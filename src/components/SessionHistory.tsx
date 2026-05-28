import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SessionSummary {
  id: string;
  title: string;
  started_at: string;
  ended_at: string | null;
  step_count: number;
  root: string;
}

interface Props {
  onLoad: (summary: SessionSummary) => void;
}

export default function SessionHistory({ onLoad }: Props) {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [open, setOpen] = useState(false);

  const refresh = async () => {
    setLoading(true);
    try {
      const list = await invoke<SessionSummary[]>("list_sessions");
      setSessions(list);
    } catch (e) {
      console.error("list_sessions failed", e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open) refresh();
  }, [open]);

  const handleLoad = async (s: SessionSummary) => {
    try {
      const summary = await invoke<SessionSummary>("load_session", { root: s.root });
      onLoad(summary);
      setOpen(false);
    } catch (e) {
      alert(`Could not load session: ${e}`);
    }
  };

  const fmt = (iso: string) =>
    new Date(iso).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });

  return (
    <section>
      <div className="section-eyebrow">History</div>
      <button
        className="btn btn-ghost btn-block"
        style={{ marginTop: 6 }}
        onClick={() => setOpen((v) => !v)}
      >
        {open ? "▲ Hide sessions" : "▼ Past sessions"}
      </button>

      {open && (
        <div style={{ marginTop: 10 }}>
          {loading && <p className="helper">Loading…</p>}
          {!loading && sessions.length === 0 && (
            <p className="helper">No past sessions found.</p>
          )}
          {sessions.map((s) => (
            <div
              key={s.id}
              style={{
                padding: "8px 10px",
                marginBottom: 6,
                border: "1px solid var(--border)",
                borderRadius: 4,
                cursor: "pointer",
                background: "var(--surface)",
              }}
              onClick={() => handleLoad(s)}
            >
              <div style={{ fontWeight: 600, fontSize: 13 }}>{s.title}</div>
              <div className="helper" style={{ fontSize: 11, marginTop: 2 }}>
                {fmt(s.started_at)} · {s.step_count} step
                {s.step_count === 1 ? "" : "s"}
                {s.ended_at ? "" : " · still recording"}
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
