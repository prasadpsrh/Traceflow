import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";

interface TimelineEntry {
  timestamp_ms: number;
  timestamp: string;
  step_index: number;
  frame_path: string;
  frame_hash: string;
  window_title: string | null;
  description: string;
  events: TimelineEvent[];
}

interface TimelineEvent {
  seq: number;
  at: string;
  kind: string;
  summary: string;
}

interface TimelineView {
  session_id: string;
  started_at: string | null;
  ended_at: string | null;
  duration_ms: number;
  total_events: number;
  entries: TimelineEntry[];
}

interface Props {
  onClose: () => void;
}

export default function ReplayViewer({ onClose }: Props) {
  const [timeline, setTimeline] = useState<TimelineView | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<TimelineView>("get_session_timeline")
      .then((t) => {
        setTimeline(t);
        if (t.entries.length > 0) setSelectedIndex(0);
      })
      .catch((e) => setError(`Could not load timeline: ${e}`));
  }, []);

  if (error) {
    return (
      <div className="replay-viewer">
        <div className="replay-header">
          <h2 className="section-title">Time Machine</h2>
          <button className="btn btn-ghost" onClick={onClose}>← Back</button>
        </div>
        <div className="empty">
          <div className="empty-title">Error</div>
          <p className="helper">{error}</p>
        </div>
      </div>
    );
  }

  if (!timeline || timeline.entries.length === 0) {
    return (
      <div className="replay-viewer">
        <div className="replay-header">
          <h2 className="section-title">Time Machine</h2>
          <button className="btn btn-ghost" onClick={onClose}>← Back</button>
        </div>
        <div className="empty">
          <div className="empty-glyph">⏱</div>
          <div className="empty-title">No steps to replay</div>
          <p className="helper">
            Capture a session first, then open Time Machine to replay it
            step by step with full event context.
          </p>
        </div>
      </div>
    );
  }

  const entry = timeline.entries[selectedIndex];


  return (
    <div className="replay-viewer">
      <div className="replay-header">
        <div>
          <h2 className="section-title">Time Machine</h2>
          <div className="pack-meta">
            Session {timeline.session_id.slice(0, 8)} ·{" "}
            {timeline.total_events} events ·{" "}
            {(timeline.duration_ms / 1000).toFixed(1)}s duration
          </div>
        </div>
        <button className="btn btn-ghost" onClick={onClose}>← Back</button>
      </div>

      <div className="replay-body">
        <div className="replay-frame-panel">
          <div className="replay-step-label">
            Step {entry.step_index + 1} of {timeline.entries.length}
            {entry.window_title && (
              <span className="helper"> — {entry.window_title}</span>
            )}
          </div>
          <img
            className="replay-frame"
            src={convertFileSrc(entry.frame_path)}
            alt={`Step ${entry.step_index + 1}`}
          />
          <div className="replay-description">{entry.description}</div>

          <div className="replay-scrubber">
            <input
              type="range"
              min={0}
              max={timeline.entries.length - 1}
              value={selectedIndex}
              onChange={(e) => setSelectedIndex(parseInt(e.target.value))}
              className="replay-range"
            />
            <div className="replay-timestamps">
              <span>{new Date(timeline.entries[0].timestamp).toLocaleTimeString()}</span>
              <span className="replay-current-time">
                {new Date(entry.timestamp).toLocaleTimeString()}
              </span>
              <span>
                {new Date(
                  timeline.entries[timeline.entries.length - 1].timestamp
                ).toLocaleTimeString()}
              </span>
            </div>
          </div>

          <div className="replay-nav">
            <button
              className="btn btn-ghost"
              disabled={selectedIndex === 0}
              onClick={() => setSelectedIndex((i) => i - 1)}
            >
              ← Previous
            </button>
            <span className="pack-meta">
              {entry.frame_hash.slice(0, 12)}…
            </span>
            <button
              className="btn btn-ghost"
              disabled={selectedIndex === timeline.entries.length - 1}
              onClick={() => setSelectedIndex((i) => i + 1)}
            >
              Next →
            </button>
          </div>
        </div>

        <div className="replay-events-panel">
          <div className="section-eyebrow">Events near this step</div>
          {entry.events.length === 0 ? (
            <div className="helper">No events within ±500ms of this step.</div>
          ) : (
            <div className="replay-event-list">
              {entry.events.map((ev) => (
                <div key={ev.seq} className="replay-event-row">
                  <div className="replay-event-kind">{ev.kind}</div>
                  <div className="replay-event-summary">{ev.summary}</div>
                  <div className="replay-event-time">
                    {new Date(ev.at).toLocaleTimeString()}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}