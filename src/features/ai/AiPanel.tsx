import type { Editor } from "@tiptap/react";
import { useEditorState } from "@tiptap/react";
import { Settings, Square, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { api, errorMessage } from "@/data/api";
import type { AiStatus, EnhanceAction, IdeaKind } from "@/data/types";
import { SettingsDialog } from "./SettingsDialog";
import { textToContent } from "./textContent";
import { useGeneration, type GenerationState } from "./useGeneration";

const TABS = [
  { id: "next", label: "Next line" },
  { id: "improve", label: "Improve" },
  { id: "ideas", label: "Ideas" },
] as const;
type Tab = (typeof TABS)[number]["id"];

const ACTIONS: { id: EnhanceAction; label: string }[] = [
  { id: "expand", label: "Expand" },
  { id: "dialogue", label: "Better dialogue" },
  { id: "rewrite", label: "Rewrite" },
];

const KINDS: IdeaKind[] = ["characters", "plots", "places", "themes"];

/** Failures that Settings can fix. */
const SETUP_CODES = ["notConfigured", "missingKey", "badKey", "modelMissing", "ollamaUnreachable"];

interface AiPanelProps {
  storyId: string;
  chapterId: string;
  editor: Editor | null;
  onClose: () => void;
}

function Failure({
  state,
  onSettings,
}: {
  state: GenerationState;
  onSettings: () => void;
}) {
  if (!state.error) return null;
  return (
    <div role="alert" className="text-ns-destructive">
      {state.error.message}
      {SETUP_CODES.includes(state.error.code ?? "") && (
        <button type="button" className="ml-2 underline" onClick={onSettings}>
          Open Settings
        </button>
      )}
    </div>
  );
}

export function AiPanel({ storyId, chapterId, editor, onClose }: AiPanelProps) {
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [tab, setTab] = useState<Tab>("next");
  const [notice, setNotice] = useState("");
  const { state, run, reset } = useGeneration();

  const [suggestions, setSuggestions] = useState<string[]>([]);
  // The selection an "Improve" answer was written for, so it replaces that text only.
  const [target, setTarget] = useState<{ from: number; to: number; text: string } | null>(null);
  const [kind, setKind] = useState<IdeaKind>("characters");
  const [ask, setAsk] = useState("");

  const loadStatus = useCallback(() => {
    api
      .getAiSettings()
      .then(setStatus)
      .catch((e) => setNotice(errorMessage(e)));
  }, []);
  useEffect(loadStatus, [loadStatus]);

  const selected =
    useEditorState({
      editor,
      selector: ({ editor }) => {
        if (!editor) return "";
        const { from, to } = editor.state.selection;
        return editor.state.doc.textBetween(from, to, "\n\n");
      },
    }) ?? "";

  const switchTab = (next: Tab) => {
    reset();
    setNotice("");
    setSuggestions([]);
    setTarget(null);
    setTab(next);
  };

  const suggest = async () => {
    if (!editor) return;
    setSuggestions([]);
    const { doc, selection } = editor.state;
    const result = await run({
      task: "nextLines",
      storyId,
      chapterId,
      before: doc.textBetween(0, selection.from, "\n"),
      after: doc.textBetween(selection.to, doc.content.size, "\n"),
    });
    setSuggestions(result?.suggestions ?? []);
  };

  const insertLine = (line: string) => {
    if (!editor) return;
    const { from } = editor.state.selection;
    const previous = editor.state.doc.textBetween(Math.max(0, from - 1), from, "\n");
    const spaced = previous && !/\s/.test(previous) ? ` ${line}` : line;
    editor.chain().focus().insertContentAt(from, { type: "text", text: spaced }).run();
    setSuggestions([]);
    reset();
  };

  const improve = async (action: EnhanceAction) => {
    if (!editor || !selected.trim()) return;
    const { from, to } = editor.state.selection;
    setNotice("");
    setTarget({ from, to, text: selected });
    await run({ task: "enhance", storyId, action, text: selected });
  };

  const replaceSelection = () => {
    if (!editor || !target) return;
    const { from, to, text } = target;
    const unchanged =
      to <= editor.state.doc.content.size &&
      editor.state.doc.textBetween(from, to, "\n\n") === text;
    if (!unchanged) {
      setNotice("The chapter changed since this was written. Select the text and try again.");
      return;
    }
    editor.chain().focus().insertContentAt({ from, to }, textToContent(state.text)).run();
    setTarget(null);
    reset();
  };

  const running = state.status === "running";
  const provider = status?.settings.provider;

  return (
    <aside className="flex w-80 shrink-0 flex-col border-l border-ns-border bg-ns-surface">
      <div className="flex items-center justify-between border-b border-ns-border px-3 py-2">
        <h2 className="font-semibold">Writing help</h2>
        <div className="flex">
          <button
            type="button"
            className="icon-btn"
            aria-label="AI settings"
            onClick={() => setShowSettings(true)}
          >
            <Settings size={16} />
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label="Close writing help"
            onClick={onClose}
          >
            <X size={16} />
          </button>
        </div>
      </div>

      {status && !provider ? (
        <div className="space-y-3 p-3">
          <p className="text-ns-ink-secondary">
            Get next-line suggestions, rewrites and ideas from a model on this
            computer or from your own Anthropic or OpenAI key.
          </p>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => setShowSettings(true)}
          >
            Set up
          </button>
        </div>
      ) : (
        <>
          <div role="tablist" className="flex gap-1 border-b border-ns-border p-2">
            {TABS.map((item) => (
              <button
                key={item.id}
                type="button"
                role="tab"
                aria-selected={tab === item.id}
                onClick={() => switchTab(item.id)}
                className={`rounded-ns px-2.5 py-1 ${tab === item.id ? "bg-ns-elevated shadow-sm" : "text-ns-ink-secondary hover:bg-ns-surface-hover"}`}
              >
                {item.label}
              </button>
            ))}
          </div>

          <div className="flex-1 space-y-3 overflow-y-auto p-3">
            {tab === "next" && (
              <>
                <p className="text-ns-ink-secondary">
                  Three ways to continue from where your cursor is.
                </p>
                <button
                  type="button"
                  className="btn btn-primary"
                  disabled={running || !editor}
                  onClick={() => void suggest()}
                >
                  {running ? "Thinking…" : "Suggest next line"}
                </button>
                <ul className="space-y-2">
                  {suggestions.map((line) => (
                    <li key={line}>
                      <button
                        type="button"
                        title="Insert at the cursor"
                        onClick={() => insertLine(line)}
                        className="w-full rounded-ns border border-ns-border bg-ns-elevated p-2 text-left font-body text-base hover:border-ns-accent"
                      >
                        {line}
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            )}

            {tab === "improve" && (
              <>
                {selected.trim() ? (
                  <p className="line-clamp-3 border-l-2 border-ns-border-strong pl-2 font-body text-ns-ink-secondary">
                    {selected}
                  </p>
                ) : (
                  <p className="text-ns-ink-secondary">
                    Select some text in the chapter to improve it.
                  </p>
                )}
                <div className="flex flex-wrap gap-2">
                  {ACTIONS.map((action) => (
                    <button
                      key={action.id}
                      type="button"
                      className="btn"
                      disabled={running || !selected.trim()}
                      onClick={() => void improve(action.id)}
                    >
                      {action.label}
                    </button>
                  ))}
                </div>
                {state.text && (
                  <p className="whitespace-pre-wrap rounded-ns border border-ns-border bg-ns-elevated p-2 font-body text-base">
                    {state.text}
                  </p>
                )}
                {state.status === "done" && target && (
                  <div className="flex gap-2">
                    <button
                      type="button"
                      className="btn btn-primary"
                      onClick={replaceSelection}
                    >
                      Replace selection
                    </button>
                    <button
                      type="button"
                      className="btn"
                      onClick={() => {
                        setTarget(null);
                        reset();
                      }}
                    >
                      Discard
                    </button>
                  </div>
                )}
              </>
            )}

            {tab === "ideas" && (
              <>
                <div className="flex gap-2">
                  <select
                    className="field capitalize"
                    aria-label="Kind of idea"
                    value={kind}
                    onChange={(event) => setKind(event.target.value as IdeaKind)}
                  >
                    {KINDS.map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    className="btn btn-primary shrink-0"
                    disabled={running}
                    onClick={() =>
                      void run({ task: "brainstorm", storyId, kind, prompt: ask })
                    }
                  >
                    {running ? "Thinking…" : "Brainstorm"}
                  </button>
                </div>
                <input
                  className="field"
                  placeholder="Anything specific? (optional)"
                  aria-label="Anything specific?"
                  maxLength={1000}
                  value={ask}
                  onChange={(event) => setAsk(event.target.value)}
                />
                {state.text && (
                  <p className="whitespace-pre-wrap rounded-ns border border-ns-border bg-ns-elevated p-2 select-text">
                    {state.text}
                  </p>
                )}
              </>
            )}

            {running && (
              <button
                type="button"
                className="inline-flex items-center gap-1 text-xs text-ns-ink-secondary hover:text-ns-ink"
                onClick={reset}
              >
                <Square size={12} /> Stop
              </button>
            )}
            <Failure state={state} onSettings={() => setShowSettings(true)} />
            {notice && (
              <p role="alert" className="text-ns-gold">
                {notice}
              </p>
            )}
          </div>
        </>
      )}

      {showSettings && (
        <SettingsDialog
          onClose={() => {
            setShowSettings(false);
            loadStatus();
          }}
        />
      )}
    </aside>
  );
}
