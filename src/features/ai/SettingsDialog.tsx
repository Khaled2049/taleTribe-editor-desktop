import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage } from "@/data/api";
import type {
  AiSettings,
  AiStatus,
  ModelInfo,
  Provider,
} from "@/data/types";

const PROVIDERS: { id: Provider; name: string; blurb: string }[] = [
  {
    id: "ollama",
    name: "Ollama (on this computer)",
    blurb: "Runs a model locally. Nothing you write leaves this computer.",
  },
  {
    id: "anthropic",
    name: "Anthropic (Claude)",
    blurb: "Uses your own API key. The text near your cursor and your story notes are sent to Anthropic.",
  },
  {
    id: "openai",
    name: "OpenAI (ChatGPT models)",
    blurb: "Uses your own API key. The text near your cursor and your story notes are sent to OpenAI.",
  },
];

const BUDGETS: { id: AiSettings["contextBudget"]; label: string }[] = [
  { id: "small", label: "Small: best for local models with little memory" },
  { id: "medium", label: "Medium" },
  { id: "large", label: "Large: more characters and places, slower locally" },
];

/** The model to preselect when the writer has not chosen one. */
function defaultModel(provider: Provider, models: ModelInfo[]) {
  if (provider === "anthropic") {
    return models.find((m) => m.id === "claude-opus-5-5") ?? models[0];
  }
  // OpenAI's list has no obvious default, so the writer picks.
  return provider === "ollama" ? models[0] : undefined;
}

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [status, setStatus] = useState<AiStatus | null>(null);
  const [form, setForm] = useState<AiSettings | null>(null);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [keyInput, setKeyInput] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  useEffect(() => {
    api
      .getAiSettings()
      .then((loaded) => {
        setStatus(loaded);
        setForm(loaded.settings);
      })
      .catch((e) => setError(errorMessage(e)));
  }, []);

  const provider = form?.provider ?? null;
  const hasKey =
    provider === "anthropic"
      ? status?.hasAnthropicKey
      : provider === "openai"
        ? status?.hasOpenaiKey
        : true;
  // Read through a ref so typing an address does not refetch on every keystroke;
  // the refresh button applies it.
  const ollamaUrl = useRef("");
  ollamaUrl.current = form?.ollamaUrl ?? "";

  const loadModels = useCallback(async () => {
    if (!provider || !hasKey) return setModels([]);
    setLoading(true);
    setError("");
    try {
      const listed = await api.listModels(provider, ollamaUrl.current);
      setModels(listed);
      setForm((current) => {
        if (!current || current[provider].id) return current;
        const pick = defaultModel(provider, listed);
        return pick ? { ...current, [provider]: pick } : current;
      });
    } catch (e) {
      setModels([]);
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, [provider, hasKey]);
  useEffect(() => void loadModels(), [loadModels]);

  if (!form || !status) {
    return (
      <dialog ref={dialog} onCancel={onClose} className="rounded-ns-lg p-5">
        {error || "Loading…"}
      </dialog>
    );
  }

  const saveKey = async () => {
    if (!provider || provider === "ollama") return;
    try {
      await api.setApiKey(provider, keyInput);
      setKeyInput("");
      setStatus(await api.getAiSettings());
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const removeKey = async () => {
    if (!provider || provider === "ollama") return;
    try {
      await api.deleteApiKey(provider);
      setStatus(await api.getAiSettings());
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const save = async () => {
    try {
      await api.saveAiSettings(form);
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const chosen = provider ? form[provider] : null;
  // Keep a saved model selectable even if the provider no longer lists it.
  const options =
    chosen?.id && !models.some((m) => m.id === chosen.id)
      ? [{ ...chosen, label: chosen.id }, ...models]
      : models;

  return (
    <dialog
      ref={dialog}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      className="w-[34rem] rounded-ns-lg border border-ns-border bg-ns-elevated p-0 text-ns-ink backdrop:bg-black/40"
    >
      <div className="max-h-[80vh] space-y-5 overflow-y-auto p-5">
        <div>
          <h2 className="text-base font-semibold">AI writing help</h2>
          <p className="mt-1 text-ns-ink-secondary">
            Optional. Choose where suggestions come from.
          </p>
        </div>

        <fieldset className="space-y-2">
          <legend className="sr-only">Provider</legend>
          {PROVIDERS.map((option) => (
            <label
              key={option.id}
              className={`flex cursor-pointer gap-3 rounded-ns border p-3 ${provider === option.id ? "border-ns-accent bg-ns-accent-subtle" : "border-ns-border"}`}
            >
              <input
                type="radio"
                name="provider"
                className="mt-1 accent-[var(--ns-accent)]"
                checked={provider === option.id}
                onChange={() => {
                  setError("");
                  setKeyInput("");
                  setForm({ ...form, provider: option.id });
                }}
              />
              <span>
                <span className="block font-medium">{option.name}</span>
                <span className="block text-xs text-ns-ink-secondary">
                  {option.blurb}
                </span>
              </span>
            </label>
          ))}
        </fieldset>

        {provider === "ollama" && (
          <label className="block">
            <span className="mb-1 block text-xs font-medium text-ns-ink-secondary">
              Ollama address
            </span>
            <input
              className="field"
              value={form.ollamaUrl}
              onChange={(event) =>
                setForm({ ...form, ollamaUrl: event.target.value })
              }
            />
          </label>
        )}

        {provider && provider !== "ollama" && (
          <div>
            <span className="mb-1 block text-xs font-medium text-ns-ink-secondary">
              API key
            </span>
            {hasKey ? (
              <div className="flex items-center gap-3">
                <span>A key is saved in your system keychain.</span>
                <button type="button" className="btn" onClick={() => void removeKey()}>
                  Remove
                </button>
              </div>
            ) : (
              <div className="flex gap-2">
                <input
                  className="field"
                  type="password"
                  autoComplete="off"
                  aria-label="API key"
                  placeholder="Paste your key"
                  value={keyInput}
                  onChange={(event) => setKeyInput(event.target.value)}
                />
                <button
                  type="button"
                  className="btn shrink-0"
                  disabled={!keyInput.trim()}
                  onClick={() => void saveKey()}
                >
                  Save key
                </button>
              </div>
            )}
          </div>
        )}

        {provider && hasKey && (
          <div>
            <span className="mb-1 block text-xs font-medium text-ns-ink-secondary">
              Model
            </span>
            <div className="flex gap-2">
              <select
                className="field"
                aria-label="Model"
                value={chosen?.id ?? ""}
                onChange={(event) => {
                  const pick = options.find((m) => m.id === event.target.value);
                  if (pick) setForm({ ...form, [provider]: pick });
                }}
              >
                <option value="" disabled>
                  {loading ? "Loading models…" : "Choose a model"}
                </option>
                {options.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.label}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="btn shrink-0"
                aria-label="Refresh model list"
                disabled={loading}
                onClick={() => void loadModels()}
              >
                <RefreshCw size={16} />
              </button>
            </div>
            {provider === "ollama" && !loading && models.length === 0 && !error && (
              <p className="mt-1 text-xs text-ns-ink-secondary">
                Ollama has no models installed. Run{" "}
                <code>ollama pull</code> with a model name, then refresh.
              </p>
            )}
          </div>
        )}

        {provider && (
          <label className="block">
            <span className="mb-1 block text-xs font-medium text-ns-ink-secondary">
              How much of your story to include
            </span>
            <select
              className="field"
              value={form.contextBudget}
              onChange={(event) =>
                setForm({
                  ...form,
                  contextBudget: event.target.value as AiSettings["contextBudget"],
                })
              }
            >
              {BUDGETS.map((budget) => (
                <option key={budget.id} value={budget.id}>
                  {budget.label}
                </option>
              ))}
            </select>
          </label>
        )}

        {error && (
          <p role="alert" className="text-ns-destructive">
            {error}
          </p>
        )}
      </div>

      <div className="flex justify-end gap-2 border-t border-ns-border p-3">
        <button type="button" className="btn" onClick={onClose}>
          Cancel
        </button>
        <button
          type="button"
          className="btn btn-primary"
          disabled={!provider || !chosen?.id}
          onClick={() => void save()}
        >
          Save
        </button>
      </div>
    </dialog>
  );
}
