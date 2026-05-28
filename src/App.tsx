import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import CaptureControl from "./components/CaptureControl";
import StepGallery from "./components/StepGallery";
import ExportPanel from "./components/ExportPanel";

export interface StepView {
  index: number;
  captured_at: string;
  image_path: string;
  description: string;
  window_title: string | null;
  app_name: string | null;
  width: number;
  height: number;
}

export interface CaptureSettings {
  poll_fps: number;
  change_threshold: number;
  stability_frames: number;
  ai_describe: boolean;
  redact_pii: boolean;
  language: string;
  keep_all_frames: boolean;
  monitor_index: number;
}

export interface MonitorInfo {
  index: number;
  name: string;
  width: number;
  height: number;
  is_primary: boolean;
}

export type ExportFormat = "docx" | "md" | "html" | "json";

export default function App() {
  const [recording, setRecording] = useState(false);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [sessionTitle, setSessionTitle] = useState("Untitled documentation");
  const [steps, setSteps] = useState<StepView[]>([]);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [verifyMsg, setVerifyMsg] = useState<string | null>(null);
  const [settings, setSettings] = useState<CaptureSettings>({
    poll_fps: 8,
    change_threshold: 0.04,
    stability_frames: 1,
    ai_describe: true,
    redact_pii: true,
    language: "en",
    keep_all_frames: false,
    monitor_index: 0,
  });

  useEffect(() => {
    invoke<MonitorInfo[]>("list_monitors").then(setMonitors).catch(console.error);
    invoke<CaptureSettings>("get_settings").then(setSettings).catch(console.error);
  }, []);

  // When a step is captured, re-fetch the projected step list (cheap and authoritative)
  useEffect(() => {
    let un: UnlistenFn | null = null;
    listen("step-captured", async () => {
      try {
        const updated = await invoke<StepView[]>("get_session_steps");
        setSteps(updated);
      } catch (e) {
        console.error(e);
      }
    }).then((u) => (un = u));
    return () => {
      if (un) un();
    };
  }, []);

  const handleStart = async () => {
    try {
      await invoke("update_settings", { settings });
      const id = await invoke<string>("start_capture", { title: sessionTitle });
      setSessionId(id);
      setSteps([]);
      setVerifyMsg(null);
      setRecording(true);
    } catch (e) {
      alert(`Could not start capture: ${e}`);
    }
  };

  const handleStop = async () => {
    try {
      await invoke("stop_capture");
      setRecording(false);
    } catch (e) {
      alert(`Could not stop capture: ${e}`);
    }
  };

  const handleDeleteStep = async (idx: number) => {
    await invoke("delete_step", { stepIndex: idx });
    const updated = await invoke<StepView[]>("get_session_steps");
    setSteps(updated);
  };

  const handleUpdateDescription = async (idx: number, desc: string) => {
    await invoke("update_step_description", { stepIndex: idx, description: desc });
    setSteps((prev) =>
      prev.map((s) => (s.index === idx ? { ...s, description: desc } : s))
    );
  };

  const handleExport = async (fmt: ExportFormat) => {
    const ext = fmt;
    const target = await save({
      title: `Export ${fmt.toUpperCase()}`,
      defaultPath: `${sessionTitle.replace(/[^a-z0-9-_ ]/gi, "_")}.${ext}`,
      filters: [{ name: fmt.toUpperCase(), extensions: [ext] }],
    });
    if (!target) return;
    try {
      const path = await invoke<string>("export_document", {
        req: { output_path: target, title: sessionTitle, author: null },
      });
      alert(`Exported to:\n${path}`);
    } catch (e) {
      alert(`Export failed: ${e}`);
    }
  };

  const handleVerify = async () => {
    try {
      const rep = await invoke<{ ok: boolean; verified_events: number; message: string }>(
        "verify_session_chain"
      );
      setVerifyMsg(rep.message);
    } catch (e) {
      setVerifyMsg(`Verification error: ${e}`);
    }
  };

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <div className="brand-mark">
            Trace<em>flow</em>
          </div>
          <span className="brand-tag">v0.2 · event-sourced · offline</span>
        </div>
        <div className="session-meta">
          {sessionId ? `session ${sessionId.slice(0, 8)}` : "no active session"}
        </div>
      </header>

      <div className="main">
        <aside className="sidebar">
          <CaptureControl
            recording={recording}
            sessionTitle={sessionTitle}
            onTitleChange={setSessionTitle}
            settings={settings}
            onSettingsChange={setSettings}
            monitors={monitors}
            onStart={handleStart}
            onStop={handleStop}
          />
          <div style={{ height: 24 }} />
          <ExportPanel
            stepCount={steps.length}
            disabled={steps.length === 0}
            onExport={handleExport}
            onVerify={handleVerify}
            verifyMsg={verifyMsg}
          />
        </aside>

        <main className="workspace">
          <div className="section-eyebrow">Captured steps</div>
          <h2 className="section-title">{sessionTitle || <em>Untitled</em>}</h2>
          {steps.length === 0 ? (
            <div className="empty">
              <div className="empty-glyph">∅</div>
              <div className="empty-title">No steps yet</div>
              <p className="empty-sub helper">
                Start a capture session, then walk through any workflow — an installer,
                a configuration screen, a procedure. Traceflow appends each meaningful
                change to a tamper-evident event log, then renders it into the document
                format you choose.
              </p>
            </div>
          ) : (
            <StepGallery
              steps={steps}
              onDelete={handleDeleteStep}
              onUpdateDescription={handleUpdateDescription}
            />
          )}
        </main>
      </div>

      <footer className="statusbar">
        <div>
          <span className={`status-dot ${recording ? "recording" : ""}`} />
          {recording ? "Recording" : "Idle"} · {settings.poll_fps} fps · threshold{" "}
          {(settings.change_threshold * 100).toFixed(1)}%
        </div>
        <div>
          {settings.redact_pii ? "PII rules on" : "PII rules off"} ·{" "}
          {settings.ai_describe ? "AI on" : "AI off"} · {steps.length} step
          {steps.length === 1 ? "" : "s"}
        </div>
      </footer>
    </div>
  );
}
