import { X } from "lucide-react";
import { useEffect, useState } from "react";
import { api, errorMessage } from "@/data/api";
import type { Snapshot } from "@/data/types";
import { ReadOnlyChapter } from "@/editor/ReadOnlyChapter";
import { formatDateTime, formatWords } from "@/lib/format";

interface HistoryPanelProps {
  chapterId: string;
  /** Changes whenever the chapter is saved, so the list stays current. */
  savedAt: number;
  onRestore: (snapshotId: string) => void;
  onClose: () => void;
  onError: (message: string) => void;
}

export function HistoryPanel({
  chapterId,
  savedAt,
  onRestore,
  onClose,
  onError,
}: HistoryPanelProps) {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [preview, setPreview] = useState<{ id: string; content: string } | null>(
    null,
  );

  useEffect(() => {
    let current = true;
    api
      .listSnapshots(chapterId)
      .then((list) => current && setSnapshots(list))
      .catch((e) => onError(errorMessage(e)));
    return () => {
      current = false;
    };
  }, [chapterId, savedAt, onError]);

  const show = (id: string) => {
    api
      .getSnapshotContent(id)
      .then((content) => setPreview({ id, content }))
      .catch((e) => onError(errorMessage(e)));
  };

  return (
    <aside className="flex w-80 shrink-0 flex-col border-l border-ns-border bg-ns-surface">
      <div className="flex items-center justify-between border-b border-ns-border px-3 py-2">
        <h2 className="font-semibold">Earlier versions</h2>
        <button
          type="button"
          className="icon-btn"
          aria-label="Close versions"
          onClick={onClose}
        >
          <X size={16} />
        </button>
      </div>
      {snapshots.length === 0 ? (
        <p className="p-3 text-ns-ink-secondary">
          No earlier versions yet. One is kept every few minutes while you
          write, up to the last 20.
        </p>
      ) : (
        <ul className="max-h-56 shrink-0 overflow-y-auto border-b border-ns-border p-2">
          {snapshots.map((snapshot) => (
            <li key={snapshot.id}>
              <button
                type="button"
                aria-current={preview?.id === snapshot.id ? "true" : undefined}
                onClick={() => show(snapshot.id)}
                className={`w-full rounded-ns px-3 py-1.5 text-left hover:bg-ns-surface-hover ${preview?.id === snapshot.id ? "bg-ns-elevated" : ""}`}
              >
                {formatDateTime(snapshot.createdAt)}
                <span className="block text-xs text-ns-ink-muted">
                  {formatWords(snapshot.wordCount)}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {preview && (
        <>
          <div className="flex-1 overflow-y-auto p-3 text-sm [&_.ProseMirror]:min-h-0 [&_.ProseMirror]:text-sm">
            <ReadOnlyChapter content={preview.content} />
          </div>
          <div className="border-t border-ns-border p-2">
            <button
              type="button"
              className="btn btn-primary w-full justify-center"
              onClick={() => onRestore(preview.id)}
            >
              Restore this version
            </button>
            <p className="mt-1.5 text-xs text-ns-ink-muted">
              Your current text is kept as a version first.
            </p>
          </div>
        </>
      )}
    </aside>
  );
}
