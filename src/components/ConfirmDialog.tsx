import { useEffect, useRef } from "react";

interface ConfirmDialogProps {
  title: string;
  message: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/** A native modal dialog, so focus is trapped and Escape cancels. */
export function ConfirmDialog({
  title,
  message,
  confirmLabel,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => dialog?.close();
  }, []);

  return (
    <dialog
      ref={ref}
      onCancel={(event) => {
        event.preventDefault();
        onCancel();
      }}
      className="w-[26rem] rounded-ns-lg border border-ns-border bg-ns-elevated p-5 text-ns-ink backdrop:bg-black/40"
    >
      <h2 className="text-base font-semibold">{title}</h2>
      <p className="mt-2 text-ns-ink-secondary">{message}</p>
      <div className="mt-5 flex justify-end gap-2">
        <button type="button" className="btn" onClick={onCancel} autoFocus>
          Cancel
        </button>
        <button type="button" className="btn btn-danger" onClick={onConfirm}>
          {confirmLabel}
        </button>
      </div>
    </dialog>
  );
}
