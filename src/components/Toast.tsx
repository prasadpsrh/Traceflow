import { useEffect } from "react";

export interface ToastMessage {
  id: number;
  text: string;
  kind: "error" | "info" | "success";
}

interface Props {
  toasts: ToastMessage[];
  onDismiss: (id: number) => void;
}

export default function ToastStack({ toasts, onDismiss }: Props) {
  return (
    <div
      style={{
        position: "fixed",
        bottom: 48,
        right: 16,
        display: "flex",
        flexDirection: "column",
        gap: 8,
        zIndex: 9999,
        maxWidth: 360,
      }}
    >
      {toasts.map((t) => (
        <ToastItem key={t.id} toast={t} onDismiss={onDismiss} />
      ))}
    </div>
  );
}

function ToastItem({
  toast,
  onDismiss,
}: {
  toast: ToastMessage;
  onDismiss: (id: number) => void;
}) {
  useEffect(() => {
    const timer = setTimeout(() => onDismiss(toast.id), toast.kind === "error" ? 8000 : 4000);
    return () => clearTimeout(timer);
  }, [toast.id, toast.kind, onDismiss]);

  const bg =
    toast.kind === "error"
      ? "var(--accent)"
      : toast.kind === "success"
      ? "var(--good, #2d7a2d)"
      : "var(--fg)";

  return (
    <div
      style={{
        background: bg,
        color: "#fff",
        padding: "10px 14px",
        borderRadius: 5,
        fontSize: 13,
        lineHeight: 1.45,
        display: "flex",
        justifyContent: "space-between",
        alignItems: "flex-start",
        gap: 10,
        boxShadow: "0 2px 8px rgba(0,0,0,0.25)",
        cursor: "pointer",
      }}
      onClick={() => onDismiss(toast.id)}
    >
      <span>{toast.text}</span>
      <span style={{ opacity: 0.7, flexShrink: 0 }}>✕</span>
    </div>
  );
}
