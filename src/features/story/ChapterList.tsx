import { ArrowDown, ArrowUp, Pencil, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import type { ChapterSummary } from "@/data/types";
import { formatWords } from "@/lib/format";
import { STORY_CHAPTER_LIMIT, TITLE_LIMIT } from "@/lib/limits";

interface ChapterListProps {
  chapters: ChapterSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onAdd: () => void;
  onRename: (id: string, title: string) => void;
  onMove: (id: string, direction: -1 | 1) => void;
  onDelete: (chapter: ChapterSummary) => void;
}

export function ChapterList({
  chapters,
  selectedId,
  onSelect,
  onAdd,
  onRename,
  onMove,
  onDelete,
}: ChapterListProps) {
  const [renaming, setRenaming] = useState<{ id: string; title: string } | null>(
    null,
  );

  const commitRename = () => {
    if (renaming) onRename(renaming.id, renaming.title);
    setRenaming(null);
  };

  return (
    <aside className="flex w-72 shrink-0 flex-col border-r border-ns-border bg-ns-surface">
      <ol className="flex-1 overflow-y-auto p-2">
        {chapters.map((chapter, index) => {
          const selected = chapter.id === selectedId;
          return (
            <li
              key={chapter.id}
              className={`group rounded-ns ${selected ? "bg-ns-elevated shadow-sm" : "hover:bg-ns-surface-hover"}`}
            >
              {renaming?.id === chapter.id ? (
                <input
                  autoFocus
                  className="field"
                  aria-label="Chapter title"
                  value={renaming.title}
                  maxLength={TITLE_LIMIT}
                  onChange={(event) =>
                    setRenaming({ id: chapter.id, title: event.target.value })
                  }
                  onBlur={commitRename}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") commitRename();
                    if (event.key === "Escape") setRenaming(null);
                  }}
                />
              ) : (
                <div className="flex items-center">
                  <button
                    type="button"
                    aria-current={selected ? "true" : undefined}
                    onClick={() => onSelect(chapter.id)}
                    className="min-w-0 flex-1 rounded-ns px-3 py-2 text-left"
                  >
                    <span className="block truncate">
                      <span className="text-ns-ink-muted">{index + 1}.</span>{" "}
                      {chapter.title}
                    </span>
                    <span className="block text-xs text-ns-ink-muted">
                      {formatWords(chapter.wordCount)}
                    </span>
                  </button>
                  <div className="flex pr-1 opacity-0 focus-within:opacity-100 group-hover:opacity-100">
                    <button
                      type="button"
                      className="icon-btn"
                      aria-label={`Move ${chapter.title} up`}
                      disabled={index === 0}
                      onClick={() => onMove(chapter.id, -1)}
                    >
                      <ArrowUp size={14} />
                    </button>
                    <button
                      type="button"
                      className="icon-btn"
                      aria-label={`Move ${chapter.title} down`}
                      disabled={index === chapters.length - 1}
                      onClick={() => onMove(chapter.id, 1)}
                    >
                      <ArrowDown size={14} />
                    </button>
                    <button
                      type="button"
                      className="icon-btn"
                      aria-label={`Rename ${chapter.title}`}
                      onClick={() =>
                        setRenaming({ id: chapter.id, title: chapter.title })
                      }
                    >
                      <Pencil size={14} />
                    </button>
                    <button
                      type="button"
                      className="icon-btn"
                      aria-label={`Delete ${chapter.title}`}
                      onClick={() => onDelete(chapter)}
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                </div>
              )}
            </li>
          );
        })}
      </ol>
      {chapters.length > STORY_CHAPTER_LIMIT && (
        <p className="border-t border-ns-border px-3 py-2 text-xs text-ns-gold">
          A story on TheTaleTribe can have {STORY_CHAPTER_LIMIT} chapters. This
          one will need fewer before it can be published.
        </p>
      )}
      <div className="border-t border-ns-border p-2">
        <button type="button" className="btn w-full justify-center" onClick={onAdd}>
          <Plus size={16} /> Add chapter
        </button>
      </div>
    </aside>
  );
}
