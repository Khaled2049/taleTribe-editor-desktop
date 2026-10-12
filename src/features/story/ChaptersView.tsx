import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Editor } from "@tiptap/react";
import { History, Sparkles } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { api, errorMessage } from "@/data/api";
import type { Chapter, ChapterSummary } from "@/data/types";
import { ChapterEditor } from "@/editor/ChapterEditor";
import { useAutosave, type SaveState } from "@/hooks/useAutosave";
import { formatWords } from "@/lib/format";
import { AiPanel } from "@/features/ai/AiPanel";
import { CHAPTER_WORD_LIMIT } from "@/lib/limits";
import { ChapterList } from "./ChapterList";
import type { LeaveGuard } from "./guard";
import { HistoryPanel } from "./HistoryPanel";

const SAVE_LABEL: Record<SaveState, string> = {
  saved: "Saved",
  pending: "Unsaved changes",
  saving: "Saving…",
  error: "Couldn't save",
};
const UNSAVED = "This chapter couldn't be saved, so it is still open. Try again.";

interface ChaptersViewProps {
  storyId: string;
  guard: LeaveGuard;
  onError: (message: string) => void;
}

export function ChaptersView({ storyId, guard, onError }: ChaptersViewProps) {
  const [chapters, setChapters] = useState<ChapterSummary[]>([]);
  const [current, setCurrent] = useState<Chapter | null>(null);
  // Bumped to remount the editor when the same chapter gets new text (a restore).
  const [editorKey, setEditorKey] = useState(0);
  const [words, setWords] = useState(0);
  const [panel, setPanel] = useState<"versions" | "ai" | null>(null);
  const [editor, setEditor] = useState<Editor | null>(null);
  const [toDelete, setToDelete] = useState<ChapterSummary | null>(null);

  const fail = useCallback((e: unknown) => onError(errorMessage(e)), [onError]);

  const { state, schedule, flush } = useAutosave<{ id: string; content: string }>(
    async ({ id, content }) => {
      const saved = await api.saveChapterContent(id, content);
      setChapters((list) => list.map((c) => (c.id === id ? saved : c)));
    },
  );

  /** Saves pending text; reports and resolves false if that fails. */
  const saveFirst = useCallback(async () => {
    const ok = await flush();
    if (!ok) onError(UNSAVED);
    return ok;
  }, [flush, onError]);

  useEffect(() => {
    guard.current = saveFirst;
    return () => {
      guard.current = null;
    };
  }, [guard, saveFirst]);

  // Closing the window or switching apps must not strand the last keystrokes.
  useEffect(() => {
    const onBlur = () => void flush();
    window.addEventListener("blur", onBlur);
    const unlisten = getCurrentWindow().onCloseRequested(async (event) => {
      if (!(await flush())) {
        event.preventDefault();
        onError(UNSAVED);
      }
    });
    return () => {
      window.removeEventListener("blur", onBlur);
      void unlisten.then((stop) => stop());
    };
  }, [flush, onError]);

  const show = useCallback((chapter: Chapter | null) => {
    setCurrent(chapter);
    setEditorKey((key) => key + 1);
  }, []);

  const open = useCallback(
    async (id: string) => {
      if (!(await saveFirst())) return;
      try {
        show(await api.getChapter(id));
      } catch (e) {
        fail(e);
      }
    },
    [saveFirst, show, fail],
  );

  useEffect(() => {
    let active = true;
    (async () => {
      const list = await api.listChapters(storyId);
      if (!active) return;
      setChapters(list);
      if (list[0]) show(await api.getChapter(list[0].id));
    })().catch(fail);
    return () => {
      active = false;
    };
  }, [storyId, show, fail]);

  const add = async () => {
    if (!(await saveFirst())) return;
    try {
      const chapter = await api.createChapter(storyId);
      setChapters((list) => [...list, chapter]);
      show(chapter);
    } catch (e) {
      fail(e);
    }
  };

  const rename = async (id: string, title: string) => {
    try {
      const renamed = await api.renameChapter(id, title);
      setChapters((list) => list.map((c) => (c.id === id ? renamed : c)));
    } catch (e) {
      fail(e);
    }
  };

  const move = async (id: string, direction: -1 | 1) => {
    const ids = chapters.map((c) => c.id);
    const from = ids.indexOf(id);
    const to = from + direction;
    if (from < 0 || to < 0 || to >= ids.length) return;
    [ids[from], ids[to]] = [ids[to], ids[from]];
    try {
      setChapters(await api.reorderChapters(storyId, ids));
    } catch (e) {
      fail(e);
    }
  };

  const confirmDelete = async () => {
    const target = toDelete;
    setToDelete(null);
    if (!target) return;
    // A save still pending for this chapter would otherwise fail after it is gone.
    if (!(await saveFirst())) return;
    try {
      await api.deleteChapter(target.id);
      const rest = chapters.filter((c) => c.id !== target.id);
      setChapters(rest);
      if (current?.id === target.id) {
        show(rest[0] ? await api.getChapter(rest[0].id) : null);
      }
    } catch (e) {
      fail(e);
    }
  };

  const restore = async (snapshotId: string) => {
    if (!(await saveFirst())) return;
    try {
      const restored = await api.restoreSnapshot(snapshotId);
      setChapters((list) =>
        list.map((c) => (c.id === restored.id ? restored : c)),
      );
      show(restored);
    } catch (e) {
      fail(e);
    }
  };

  const savedAt = chapters.find((c) => c.id === current?.id)?.updatedAt ?? 0;
  const overLimit = words > CHAPTER_WORD_LIMIT;

  return (
    <div className="flex min-h-0 flex-1">
      <ChapterList
        chapters={chapters}
        selectedId={current?.id ?? null}
        onSelect={(id) => id !== current?.id && void open(id)}
        onAdd={() => void add()}
        onRename={(id, title) => void rename(id, title)}
        onMove={(id, direction) => void move(id, direction)}
        onDelete={setToDelete}
      />

      <section className="flex min-w-0 flex-1 flex-col">
        {current ? (
          <ChapterEditor
            key={`${current.id}:${editorKey}`}
            initialContent={current.content}
            onChange={(content) => schedule({ id: current.id, content })}
            onBlur={() => void flush()}
            onWordCount={setWords}
            onError={onError}
            onEditor={setEditor}
          />
        ) : (
          <p className="m-auto text-ns-ink-muted">
            This story has no chapters. Add one to start writing.
          </p>
        )}

        {current && (
          <footer className="flex items-center gap-4 border-t border-ns-border bg-ns-surface px-4 py-1.5 text-xs text-ns-ink-secondary">
            <span
              role="status"
              className={state === "error" ? "text-ns-destructive" : ""}
            >
              {SAVE_LABEL[state]}
            </span>
            {state === "error" && (
              <button
                type="button"
                className="underline"
                onClick={() => void saveFirst()}
              >
                Try again
              </button>
            )}
            <span className={overLimit ? "text-ns-gold" : ""}>
              {formatWords(words)}
              {overLimit &&
                ` · over the ${CHAPTER_WORD_LIMIT.toLocaleString()}-word limit for publishing; split this chapter before you publish`}
            </span>
            <button
              type="button"
              className="ml-auto inline-flex items-center gap-1 hover:text-ns-ink"
              aria-expanded={panel === "ai"}
              onClick={() => setPanel(panel === "ai" ? null : "ai")}
            >
              <Sparkles size={14} /> Writing help
            </button>
            <button
              type="button"
              className="inline-flex items-center gap-1 hover:text-ns-ink"
              aria-expanded={panel === "versions"}
              onClick={() => setPanel(panel === "versions" ? null : "versions")}
            >
              <History size={14} /> Versions
            </button>
          </footer>
        )}
      </section>

      {panel === "ai" && current && (
        <AiPanel
          key={current.id}
          storyId={storyId}
          chapterId={current.id}
          editor={editor}
          onClose={() => setPanel(null)}
        />
      )}

      {panel === "versions" && current && (
        <HistoryPanel
          chapterId={current.id}
          savedAt={savedAt}
          onRestore={(id) => void restore(id)}
          onClose={() => setPanel(null)}
          onError={onError}
        />
      )}

      {toDelete && (
        <ConfirmDialog
          title={`Delete "${toDelete.title}"?`}
          message="The chapter and its saved versions are removed from this computer. This cannot be undone."
          confirmLabel="Delete chapter"
          onConfirm={() => void confirmDelete()}
          onCancel={() => setToDelete(null)}
        />
      )}
    </div>
  );
}
