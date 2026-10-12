import { ArrowLeft, Upload } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { ErrorBanner } from "@/components/ErrorBanner";
import { api, errorMessage } from "@/data/api";
import type { Character, Place, Story } from "@/data/types";
import { PublishDialog } from "@/features/publish/PublishDialog";
import { DESCRIPTION_LIMIT, TITLE_LIMIT } from "@/lib/limits";
import { ChaptersView } from "./ChaptersView";
import { EntityPanel } from "./EntityPanel";

const TABS = ["chapters", "characters", "places", "details"] as const;
type Tab = (typeof TABS)[number];

const CHARACTER_FIELDS = [
  { key: "soul", label: "Soul: what drives them" },
  { key: "personality", label: "Personality" },
  { key: "voice", label: "Voice: how they speak" },
  { key: "backstory", label: "Backstory" },
  { key: "affiliations", label: "Affiliations" },
  { key: "notes", label: "Notes" },
] as const;

const PLACE_FIELDS = [
  { key: "description", label: "Description" },
  { key: "atmosphere", label: "Atmosphere" },
  { key: "geography", label: "Geography" },
  { key: "history", label: "History" },
  { key: "significance", label: "Significance to the story" },
  { key: "notes", label: "Notes" },
] as const;

interface StoryWorkspaceProps {
  storyId: string;
  onBack: () => void;
}

export function StoryWorkspace({ storyId, onBack }: StoryWorkspaceProps) {
  const [story, setStory] = useState<Story | null>(null);
  const [tab, setTab] = useState<Tab>("chapters");
  const [error, setError] = useState("");
  const [publishing, setPublishing] = useState(false);
  const guard = useRef<(() => Promise<boolean>) | null>(null);

  useEffect(() => {
    api
      .listStories()
      .then((stories) => setStory(stories.find((s) => s.id === storyId) ?? null))
      .catch((e) => setError(errorMessage(e)));
  }, [storyId]);

  /** Runs `go` once the open view has saved its work. */
  const leave = useCallback(async (go: () => void) => {
    if ((await guard.current?.()) === false) return;
    go();
  }, []);

  const saveDetails = async (patch: Partial<Story>) => {
    if (!story) return;
    const next = { ...story, ...patch };
    try {
      setStory(await api.updateStory(story.id, next.title, next.description));
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-3 border-b border-ns-border px-3 py-2">
        <button
          type="button"
          className="icon-btn"
          aria-label="Back to your stories"
          onClick={() => void leave(onBack)}
        >
          <ArrowLeft size={18} />
        </button>
        <input
          key={story?.title}
          className="min-w-0 flex-1 rounded bg-transparent px-2 py-1 font-body text-xl hover:bg-ns-surface"
          aria-label="Story title"
          defaultValue={story?.title ?? ""}
          maxLength={TITLE_LIMIT}
          onBlur={(event) =>
            event.target.value !== story?.title &&
            void saveDetails({ title: event.target.value })
          }
        />
        <nav className="flex gap-1" aria-label="Story sections">
          {TABS.map((name) => (
            <button
              key={name}
              type="button"
              aria-current={tab === name ? "page" : undefined}
              onClick={() => void leave(() => setTab(name))}
              className={`rounded-ns px-3 py-1.5 capitalize ${tab === name ? "bg-ns-accent-subtle text-ns-accent" : "text-ns-ink-secondary hover:bg-ns-surface-hover"}`}
            >
              {name}
            </button>
          ))}
        </nav>
        {/* Unsaved text is saved first, so what is published is what is on screen. */}
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void leave(() => setPublishing(true))}
        >
          <Upload size={16} /> Publish
        </button>
      </header>
      <ErrorBanner message={error} onDismiss={() => setError("")} />

      {publishing && (
        <PublishDialog storyId={storyId} onClose={() => setPublishing(false)} />
      )}

      {tab === "chapters" && (
        <ChaptersView storyId={storyId} guard={guard} onError={setError} />
      )}

      {tab === "characters" && (
        <EntityPanel<Character>
          noun="character"
          fields={[...CHARACTER_FIELDS]}
          blank={() => ({
            id: "",
            storyId,
            name: "",
            age: null,
            soul: "",
            personality: "",
            voice: "",
            backstory: "",
            affiliations: "",
            notes: "",
          })}
          load={() => api.listCharacters(storyId)}
          save={api.saveCharacter}
          remove={api.deleteCharacter}
          extra={(draft, change) => (
            <label className="block w-32">
              <span className="mb-1 block text-xs font-medium text-ns-ink-secondary">
                Age
              </span>
              <input
                className="field"
                type="number"
                min={0}
                value={draft.age ?? ""}
                onChange={(event) =>
                  change({
                    age:
                      event.target.value === ""
                        ? null
                        : Math.trunc(Number(event.target.value)),
                  })
                }
              />
            </label>
          )}
          guard={guard}
          onError={setError}
        />
      )}

      {tab === "places" && (
        <EntityPanel<Place>
          noun="place"
          fields={[...PLACE_FIELDS]}
          blank={() => ({
            id: "",
            storyId,
            name: "",
            description: "",
            atmosphere: "",
            geography: "",
            history: "",
            significance: "",
            notes: "",
          })}
          load={() => api.listPlaces(storyId)}
          save={api.savePlace}
          remove={api.deletePlace}
          guard={guard}
          onError={setError}
        />
      )}

      {tab === "details" && story && (
        <div className="mx-auto w-full max-w-2xl px-8 py-8">
          <label className="block">
            <span className="mb-1 block text-xs font-medium text-ns-ink-secondary">
              Description
            </span>
            <textarea
              className="field min-h-40 resize-y"
              placeholder="What is this story about?"
              defaultValue={story.description}
              maxLength={DESCRIPTION_LIMIT}
              onBlur={(event) =>
                event.target.value !== story.description &&
                void saveDetails({ description: event.target.value })
              }
            />
          </label>
          <p className="mt-2 text-xs text-ns-ink-muted">
            Saved when you click away.
          </p>
        </div>
      )}
    </div>
  );
}
