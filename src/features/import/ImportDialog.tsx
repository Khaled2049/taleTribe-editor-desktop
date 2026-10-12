import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorCode, errorMessage } from "@/data/api";
import type {
  Account,
  ImportExisting,
  ImportOutcome,
  WebsiteStory,
} from "@/data/types";
import { SignInForm } from "@/features/account/SignInForm";
import { formatWords } from "@/lib/format";

interface ImportDialogProps {
  onClose: () => void;
  /** Opens a story once it is on this computer. */
  onOpen: (storyId: string) => void;
}

export function ImportDialog({ onClose, onOpen }: ImportDialogProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  // Undefined while loading, null when signed out.
  const [account, setAccount] = useState<Account | null | undefined>(undefined);
  const [stories, setStories] = useState<WebsiteStory[] | null>(null);
  const [picked, setPicked] = useState<WebsiteStory | null>(null);
  const [step, setStep] = useState("");
  const [running, setRunning] = useState(false);
  const [outcome, setOutcome] = useState<ImportOutcome | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  const fail = useCallback((e: unknown) => {
    // An expired or revoked sign-in sends the writer back to the form.
    if (errorCode(e) === "signInNeeded") setAccount(null);
    setError(errorMessage(e));
  }, []);

  useEffect(() => {
    api.getAccount().then(setAccount).catch(fail);
  }, [fail]);

  useEffect(() => {
    if (!account) return;
    setError("");
    api.listWebsiteStories().then(setStories).catch(fail);
  }, [account, fail]);

  const run = async (story: WebsiteStory, existing: ImportExisting | null) => {
    setPicked(story);
    setRunning(true);
    setError("");
    setStep("Reading the story");
    try {
      setOutcome(await api.importStory(story.id, existing, setStep));
    } catch (e) {
      setPicked(null);
      fail(e);
    } finally {
      setRunning(false);
    }
  };

  const back = () => {
    setPicked(null);
    setOutcome(null);
  };

  let body: React.ReactNode;
  let actions: React.ReactNode = null;

  if (account === undefined) {
    body = <p className="text-ns-ink-secondary">{error ? null : "Loading…"}</p>;
  } else if (account === null) {
    body = (
      <SignInForm
        prompt="Sign in to see your stories on TheTaleTribe."
        onSignedIn={setAccount}
      />
    );
  } else if (running) {
    body = (
      <p role="status" aria-live="polite">
        {step}…
      </p>
    );
  } else if (picked && outcome?.status === "alreadyHere") {
    body = (
      <div className="space-y-3">
        <p>
          <span className="font-medium">{outcome.title}</span> is already on
          this computer. What should happen to the copy here?
        </p>
        <div className="space-y-2">
          <button
            type="button"
            className="w-full rounded-ns border border-ns-border p-3 text-left hover:border-ns-accent"
            onClick={() => void run(picked, "replace")}
          >
            <span className="block font-medium">
              Replace it with the website's version
            </span>
            <span className="block text-xs text-ns-ink-secondary">
              Chapters here are updated to match the website. The text they had
              is kept under Versions, and chapters that exist only here stay.
            </span>
          </button>
          <button
            type="button"
            className="w-full rounded-ns border border-ns-border p-3 text-left hover:border-ns-accent"
            onClick={() => void run(picked, "copy")}
          >
            <span className="block font-medium">Keep both</span>
            <span className="block text-xs text-ns-ink-secondary">
              Adds the website's version as a separate story. That copy is not
              connected to the website; publishing it would create a new story.
            </span>
          </button>
        </div>
      </div>
    );
    actions = (
      <button type="button" className="btn" onClick={back}>
        Back
      </button>
    );
  } else if (outcome?.status === "done") {
    const parts = [
      `${outcome.chapters} ${outcome.chapters === 1 ? "chapter" : "chapters"}`,
      outcome.characters > 0 && `${outcome.characters} characters`,
      outcome.places > 0 && `${outcome.places} places`,
      outcome.images > 0 && `${outcome.images} images`,
    ].filter(Boolean);
    body = (
      <div className="space-y-2">
        <p className="font-medium">
          {picked?.title} is on this computer and can be edited offline.
        </p>
        <p className="text-ns-ink-secondary">Brought over: {parts.join(", ")}.</p>
        {outcome.keptLocal > 0 && (
          <p className="text-ns-ink-secondary">
            {outcome.keptLocal}{" "}
            {outcome.keptLocal === 1 ? "chapter that exists" : "chapters that exist"}{" "}
            only on this computer {outcome.keptLocal === 1 ? "was" : "were"} kept
            at the end.
          </p>
        )}
        {outcome.imagesMissing > 0 && (
          <p className="text-ns-gold">
            {outcome.imagesMissing}{" "}
            {outcome.imagesMissing === 1 ? "image" : "images"} couldn't be
            downloaded and will show as missing here. Nothing changed on the
            website.
          </p>
        )}
        {!outcome.linked && (
          <p className="text-ns-ink-secondary">
            This copy is not connected to the website.
          </p>
        )}
      </div>
    );
    actions = (
      <>
        <button type="button" className="btn" onClick={back}>
          Import another
        </button>
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => onOpen(outcome.storyId)}
        >
          Open story
        </button>
      </>
    );
  } else if (!stories) {
    body = <p className="text-ns-ink-secondary">{error ? null : "Loading your stories…"}</p>;
  } else if (stories.length === 0) {
    body = (
      <p className="text-ns-ink-secondary">
        You have no stories on TheTaleTribe yet.
      </p>
    );
  } else {
    body = (
      <div className="space-y-3">
        <p className="text-ns-ink-secondary">
          Choose a story to copy onto this computer. Chapters, characters and
          places come across; comments, likes and plots stay on the website.
        </p>
        <ul className="space-y-2">
          {stories.map((story) => (
            <li key={story.id}>
              <button
                type="button"
                onClick={() => void run(story, null)}
                className="w-full rounded-ns border border-ns-border p-3 text-left hover:border-ns-accent"
              >
                <span className="block truncate font-body text-lg">
                  {story.title}
                </span>
                <span className="block text-xs text-ns-ink-muted">
                  {story.chapterCount}{" "}
                  {story.chapterCount === 1 ? "chapter" : "chapters"} ·{" "}
                  {formatWords(story.wordCount)} ·{" "}
                  {story.published ? "public" : "draft"}
                  {story.localId && " · already on this computer"}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  return (
    <dialog
      ref={dialog}
      onCancel={(event) => {
        event.preventDefault();
        if (!running) onClose();
      }}
      className="w-[34rem] rounded-ns-lg border border-ns-border bg-ns-elevated p-0 text-ns-ink backdrop:bg-black/40"
    >
      <div className="max-h-[75vh] space-y-4 overflow-y-auto p-5">
        <h2 className="text-base font-semibold">Import from TheTaleTribe</h2>
        {body}
        {error && !running && (
          <p role="alert" className="text-ns-destructive">
            {error}
          </p>
        )}
      </div>
      <div className="flex justify-end gap-2 border-t border-ns-border p-3">
        <button type="button" className="btn" disabled={running} onClick={onClose}>
          Close
        </button>
        {!running && actions}
      </div>
    </dialog>
  );
}
