import { CaptureSettings, MonitorInfo } from "../App";

interface Props {
  recording: boolean;
  sessionTitle: string;
  onTitleChange: (s: string) => void;
  settings: CaptureSettings;
  onSettingsChange: (s: CaptureSettings) => void;
  monitors: MonitorInfo[];
  onStart: () => void;
  onStop: () => void;
}

export default function CaptureControl({
  recording,
  sessionTitle,
  onTitleChange,
  settings,
  onSettingsChange,
  monitors,
  onStart,
  onStop,
}: Props) {
  const update = <K extends keyof CaptureSettings>(
    k: K,
    v: CaptureSettings[K]
  ) => onSettingsChange({ ...settings, [k]: v });

  return (
    <section>
      <div className="section-eyebrow">Capture</div>
      <h3 className="section-title" style={{ fontSize: 22, marginBottom: 18 }}>
        Session
      </h3>

      <div className="field">
        <label htmlFor="title">Title</label>
        <input
          id="title"
          type="text"
          value={sessionTitle}
          onChange={(e) => onTitleChange(e.target.value)}
          disabled={recording}
          placeholder="e.g. Installing Acme Pro 5.2"
        />
      </div>

      <div className="field">
        <label htmlFor="monitor">Monitor</label>
        <select
          id="monitor"
          value={settings.monitor_index}
          onChange={(e) => update("monitor_index", parseInt(e.target.value, 10))}
          disabled={recording}
        >
          {monitors.map((m) => (
            <option key={m.index} value={m.index}>
              #{m.index} · {m.width}×{m.height}
              {m.is_primary ? " · primary" : ""}
            </option>
          ))}
        </select>
      </div>

      <div className="field-row">
        <div className="field">
          <label htmlFor="fps">Poll fps</label>
          <select
            id="fps"
            value={settings.poll_fps}
            onChange={(e) => update("poll_fps", parseInt(e.target.value, 10))}
            disabled={recording}
          >
            <option value={2}>2</option>
            <option value={4}>4</option>
            <option value={8}>8</option>
          </select>
        </div>
        <div className="field">
          <label htmlFor="lang">Language</label>
          <select
            id="lang"
            value={settings.language}
            onChange={(e) => update("language", e.target.value)}
            disabled={recording}
          >
            <option value="en">English</option>
            <option value="es">Español</option>
            <option value="fr">Français</option>
            <option value="de">Deutsch</option>
            <option value="ja">日本語</option>
            <option value="zh">中文</option>
          </select>
        </div>
      </div>

      <div className="field">
        <label htmlFor="threshold">
          Sensitivity · {(settings.change_threshold * 100).toFixed(1)}%
        </label>
        <input
          id="threshold"
          type="range"
          min={0.01}
          max={0.2}
          step={0.005}
          value={settings.change_threshold}
          onChange={(e) =>
            update("change_threshold", parseFloat(e.target.value))
          }
          disabled={recording}
        />
        <div className="helper" style={{ fontSize: 11 }}>
          Lower = more captures · Higher = only big changes
        </div>
      </div>

      <div style={{ marginTop: 16, marginBottom: 20 }}>
        <div className="checkbox-row">
          <input
            id="ai"
            type="checkbox"
            checked={settings.ai_describe}
            onChange={(e) => update("ai_describe", e.target.checked)}
            disabled={recording}
          />
          <label htmlFor="ai">AI step descriptions (local)</label>
        </div>
        <div className="checkbox-row">
          <input
            id="pii"
            type="checkbox"
            checked={settings.redact_pii}
            onChange={(e) => update("redact_pii", e.target.checked)}
            disabled={recording}
          />
          <label htmlFor="pii">Auto-redact PII &amp; passwords</label>
        </div>
      </div>

      {recording ? (
        <button className="btn btn-danger btn-block" onClick={onStop}>
          ■ Stop recording
        </button>
      ) : (
        <button
          className="btn btn-primary btn-block"
          onClick={onStart}
          disabled={!sessionTitle.trim()}
        >
          ● Start recording
        </button>
      )}
    </section>
  );
}
