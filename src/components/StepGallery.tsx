import { convertFileSrc } from "@tauri-apps/api/core";
import { StepView } from "../App";

interface Props {
  steps: StepView[];
  onDelete: (index: number) => void;
  onUpdateDescription: (index: number, desc: string) => void;
}

export default function StepGallery({ steps, onDelete, onUpdateDescription }: Props) {
  return (
    <div className="gallery">
      {steps.map((s) => (
        <div className="step-card" key={s.index}>
          <div className="step-number">№ {s.index + 1}</div>
          <img
            className="step-thumb"
            src={convertFileSrc(s.image_path)}
            alt={`Step ${s.index + 1}`}
            loading="lazy"
          />
          <div className="step-body">
            <textarea
              className="step-desc"
              rows={2}
              value={s.description}
              onChange={(e) => onUpdateDescription(s.index, e.target.value)}
              placeholder="Add a description…"
            />
          </div>
          <div className="step-meta">
            <span>
              {s.window_title
                ? truncate(s.window_title, 28)
                : new Date(s.captured_at).toLocaleTimeString()}
            </span>
            <button
              className="step-delete"
              onClick={() => onDelete(s.index)}
              title="Delete this step"
            >
              ✕ DELETE
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}

function truncate(s: string, n: number) {
  return s.length > n ? s.slice(0, n - 1) + "…" : s;
}
