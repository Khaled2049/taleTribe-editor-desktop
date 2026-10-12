import { BookOpen, Download, Plus, Settings, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorBanner } from "@/components/ErrorBanner";
import { api, errorMessage } from "@/data/api";
import type { Story } from "@/data/types";
import { AccountButton } from "@/features/account/AccountButton";
import { SettingsDialog } from "@/features/ai/SettingsDialog";
import { ImportDialog } from "@/features/import/ImportDialog";
import { formatDateTime, formatWords } from "@/lib/format";
import { TITLE_LIMIT } from "@/lib/limits";

export function Library({ onOpen }: { onOpen: (storyId: string) => void }) {
  const [stories, setStories] = useState<Story[] | null>(null);
  const [title, setTitle] = useState("");
  const [toDelete, setToDelete] = useState<Story | null>(null);
  const [error, setError] = useState("");
  const [showSettings, setShowSettings] = useState(false);
  const [importing, setImporting] = useState(false);
  // Bumped to make the account button re-read who is signed in.
  const [accountSeen, setAccountSeen] = useState(0);

  const load = useCallback(() => {
    api
      .listStories()
      .then(setStories)
      .catch((e) => setError(errorMessage(e)));
  }, []);
  useEffect(load, [load]);

  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    try {
      const story = await api.createStory(title);
      onOpen(story.id);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const confirmDelete = async () => {
    if (!toDelete) return;
    try {
      await api.deleteStory(toDelete.id);
      setToDelete(null);
      load();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="flex h-full flex-col">
      <ErrorBanner message={error} onDismiss={() => setError("")} />
      <main className="mx-auto w-full max-w-3xl flex-1 overflow-y-auto px-6 py-10">
        <div className="flex items-center justify-between">
          <h1 className="font-body text-3xl">Your stories</h1>
          <div className="flex gap-2">
            <button
              type="button"
              className="btn"
              onClick={() => setShowSettings(true)}
            >
              <Settings size={16} /> AI settings
            </button>
            <AccountButton key={accountSeen} />
          </div>
        </div>
        <p className="mt-1 text-ns-ink-secondary">
          Stored on this computer. Nothing here needs a connection.
        </p>

        <form onSubmit={create} className="mt-6 flex gap-2">
          <input
            className="field"
            placeholder="Title of a new story"
            aria-label="Title of a new story"
            value={title}
            maxLength={TITLE_LIMIT}
            onChange={(event) => setTitle(event.target.value)}
          />
          <button type="submit" className="btn btn-primary shrink-0">
            <Plus size={16} /> New story
          </button>
          <button
            type="button"
            className="btn shrink-0"
            onClick={() => setImporting(true)}
          >
            <Download size={16} /> Import from website
          </button>
        </form>

        <ul className="mt-8 space-y-3">
          {stories?.map((story) => (
            <li
              key={story.id}
              className="flex items-center gap-2 rounded-ns-lg border border-ns-border bg-ns-elevated pr-3"
            >
              <button
                type="button"
                onClick={() => onOpen(story.id)}
                className="min-w-0 flex-1 rounded-ns-lg px-4 py-3 text-left hover:bg-ns-surface-hover"
              >
                <span className="block truncate font-body text-xl">
                  {story.title}
                </span>
                <span className="mt-0.5 block text-xs text-ns-ink-muted">
                  {story.chapterCount}{" "}
                  {story.chapterCount === 1 ? "chapter" : "chapters"} ·{" "}
                  {formatWords(story.wordCount)} · edited{" "}
                  {formatDateTime(story.updatedAt)}
                </span>
              </button>
              <button
                type="button"
                className="icon-btn"
                aria-label={`Delete ${story.title}`}
                onClick={() => setToDelete(story)}
              >
                <Trash2 size={16} />
              </button>
            </li>
          ))}
        </ul>

        {stories?.length === 0 && (
          <div className="mt-16 flex flex-col items-center text-ns-ink-muted">
            <BookOpen size={32} />
            <p className="mt-3">No stories yet. Name one above to begin.</p>
          </div>
        )}
      </main>

      {importing && (
        <ImportDialog
          onOpen={onOpen}
          onClose={() => {
            setImporting(false);
            // The writer may have signed in, and the list may have a new story.
            setAccountSeen((seen) => seen + 1);
            load();
          }}
        />
      )}

      {showSettings && (
        <SettingsDialog onClose={() => setShowSettings(false)} />
      )}

      {toDelete && (
        <ConfirmDialog
          title={`Delete "${toDelete.title}"?`}
          message="The story, its chapters, characters, places and saved versions are removed from this computer. This cannot be undone."
          confirmLabel="Delete story"
          onConfirm={confirmDelete}
          onCancel={() => setToDelete(null)}
        />
      )}
    </div>
  );
}
