import { X } from "lucide-react";

export function ErrorBanner({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  if (!message) return null;
  return (
    <div
      role="alert"
      className="flex items-center justify-between gap-3 border-b border-ns-destructive/30 bg-ns-destructive/10 px-4 py-2 text-ns-destructive"
    >
      <span>{message}</span>
      <button
        type="button"
        className="icon-btn"
        onClick={onDismiss}
        aria-label="Dismiss"
      >
        <X size={14} />
      </button>
    </div>
  );
}
