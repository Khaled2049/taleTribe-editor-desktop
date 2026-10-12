import { Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { errorMessage } from "@/data/api";
import { NAME_LIMIT, PROSE_LIMIT } from "@/lib/limits";
import type { LeaveGuard } from "./guard";

interface Entity {
  id: string;
  name: string;
}

type TextKey<T> = {
  [K in keyof T]: T[K] extends string ? K : never;
}[keyof T] &
  string;

interface EntityPanelProps<T extends Entity> {
  /** Singular, lower case: "character", "place". */
  noun: string;
  fields: { key: TextKey<T>; label: string }[];
  blank: () => T;
  load: () => Promise<T[]>;
  save: (item: T) => Promise<T>;
  remove: (id: string) => Promise<void>;
  /** Inputs that are not plain text, shown under the name. */
  extra?: (draft: T, change: (patch: Partial<T>) => void) => ReactNode;
  guard: LeaveGuard;
  onError: (message: string) => void;
}

/** A list of named records with a form beside it; used for characters and places. */
export function EntityPanel<T extends Entity>({
  noun,
  fields,
  blank,
  load,
  save,
  remove,
  extra,
  guard,
  onError,
}: EntityPanelProps<T>) {
  const [items, setItems] = useState<T[]>([]);
  const [draft, setDraft] = useState<T | null>(null);
  const [dirty, setDirty] = useState(false);
  const [confirming, setConfirming] = useState(false);
  // The loaders are new functions each render; read them through a ref.
  const actions = useRef({ load, save, remove });
  actions.current = { load, save, remove };

  const refresh = useCallback(
    () =>
      actions.current
        .load()
        .then(setItems)
        .catch((e) => onError(errorMessage(e))),
    [onError],
  );
  useEffect(() => void refresh(), [refresh]);

  /** Saves the open form if it changed. Resolves false if it could not be saved. */
  const commit = useCallback(async () => {
    if (!draft || !dirty) return true;
    try {
      const saved = await actions.current.save(draft);
      setDraft(saved);
      setDirty(false);
      await refresh();
      return true;
    } catch (e) {
      onError(errorMessage(e));
      return false;
    }
  }, [draft, dirty, refresh, onError]);

  useEffect(() => {
    guard.current = commit;
    return () => {
      guard.current = null;
    };
  }, [guard, commit]);

  const select = async (item: T) => {
    if (await commit()) {
      setDraft(item);
      setDirty(false);
    }
  };

  const change = (patch: Partial<T>) => {
    setDraft((current) => (current ? { ...current, ...patch } : current));
    setDirty(true);
  };

  const confirmDelete = async () => {
    setConfirming(false);
    if (!draft) return;
    try {
      if (draft.id) await actions.current.remove(draft.id);
      setDraft(null);
      setDirty(false);
      await refresh();
    } catch (e) {
      onError(errorMessage(e));
    }
  };

  return (
    <div className="flex min-h-0 flex-1">
      <aside className="flex w-72 shrink-0 flex-col border-r border-ns-border bg-ns-surface">
        <ul className="flex-1 overflow-y-auto p-2">
          {items.map((item) => (
            <li key={item.id}>
              <button
                type="button"
                aria-current={draft?.id === item.id ? "true" : undefined}
                onClick={() => void select(item)}
                className={`w-full truncate rounded-ns px-3 py-2 text-left hover:bg-ns-surface-hover ${draft?.id === item.id ? "bg-ns-elevated shadow-sm" : ""}`}
              >
                {item.name}
              </button>
            </li>
          ))}
          {items.length === 0 && (
            <li className="px-3 py-2 text-ns-ink-muted">No {noun}s yet.</li>
          )}
        </ul>
        <div className="border-t border-ns-border p-2">
          <button
            type="button"
            className="btn w-full justify-center"
            onClick={() => void select(blank())}
          >
            <Plus size={16} /> Add {noun}
          </button>
        </div>
      </aside>

      {draft ? (
        <form
          className="flex-1 overflow-y-auto"
          onSubmit={(event) => {
            event.preventDefault();
            void commit();
          }}
        >
          <div className="mx-auto max-w-2xl space-y-4 px-8 py-8">
            <label className="block">
              <span className="mb-1 block text-xs font-medium text-ns-ink-secondary">
                Name
              </span>
              <input
                className="field text-base"
                required
                autoFocus={!draft.id}
                maxLength={NAME_LIMIT}
                value={draft.name}
                onChange={(event) =>
                  change({ name: event.target.value } as Partial<T>)
                }
              />
            </label>
            {extra?.(draft, change)}
            {fields.map((field) => (
              <label key={field.key} className="block">
                <span className="mb-1 block text-xs font-medium text-ns-ink-secondary">
                  {field.label}
                </span>
                <textarea
                  className="field min-h-24 resize-y"
                  maxLength={PROSE_LIMIT}
                  value={draft[field.key] as string}
                  onChange={(event) =>
                    change({ [field.key]: event.target.value } as Partial<T>)
                  }
                />
              </label>
            ))}
            <div className="flex items-center gap-2 pt-2">
              <button
                type="submit"
                className="btn btn-primary"
                disabled={!dirty}
              >
                {dirty ? "Save" : "Saved"}
              </button>
              <button
                type="button"
                className="btn ml-auto"
                onClick={() => setConfirming(true)}
              >
                <Trash2 size={16} /> Delete
              </button>
            </div>
          </div>
        </form>
      ) : (
        <p className="m-auto text-ns-ink-muted">
          Select a {noun}, or add a new one.
        </p>
      )}

      {confirming && draft && (
        <ConfirmDialog
          title={`Delete ${draft.name || `this ${noun}`}?`}
          message="This cannot be undone."
          confirmLabel={`Delete ${noun}`}
          onConfirm={() => void confirmDelete()}
          onCancel={() => setConfirming(false)}
        />
      )}
    </div>
  );
}
