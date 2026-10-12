import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorCode, errorMessage } from "@/data/api";
import { SignInForm } from "@/features/account/SignInForm";
import type {
  Account,
  PublishChoice,
  PublishConflict,
  PublishCounts,
  PublishMode,
  PublishOutcome,
  PublishPlan,
} from "@/data/types";

/** Answers `gone`: publish as a new story. Must match `RECREATE` in `sync/publish.rs`. */
const RECREATE = "recreate";

const describe = (counts: PublishCounts, one: string, many: string) => {
  const parts = [
    counts.new && `${counts.new} new`,
    counts.changed && `${counts.changed} changed`,
    counts.deleted && `${counts.deleted} removed`,
  ].filter(Boolean);
  const total = counts.new + counts.changed + counts.deleted;
  return parts.length ? `${parts.join(", ")} ${total === 1 ? one : many}` : null;
};

function Conflicts({
  conflicts,
  choices,
  onChoose,
}: {
  conflicts: PublishConflict[];
  choices: Record<string, PublishChoice>;
  onChoose: (id: string, choice: PublishChoice) => void;
}) {
  return (
    <div className="space-y-3">
      <p>
        These were changed on the website after this computer last published
        them. Choose what to keep for each.
      </p>
      <ul className="space-y-2">
        {conflicts.map((conflict) => (
          <li
            key={conflict.id}
            className="rounded-ns border border-ns-border p-3"
          >
            <div className="font-medium">
              <span className="capitalize">{conflict.kind}</span>:{" "}
              {conflict.label}
            </div>
            <div className="mt-2 flex flex-col gap-1">
              {(
                [
                  [
                    "overwrite",
                    conflict.deletedHere
                      ? "Delete it on the website (it was deleted here)"
                      : "Use this computer's version",
                  ],
                  ["skip", "Keep the website's version"],
                ] as const
              ).map(([choice, label]) => (
                <label key={choice} className="flex items-center gap-2">
                  <input
                    type="radio"
                    name={conflict.id}
                    className="accent-[var(--ns-accent)]"
                    checked={choices[conflict.id] === choice}
                    onChange={() => onChoose(conflict.id, choice)}
                  />
                  {label}
                </label>
              ))}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

interface PublishDialogProps {
  storyId: string;
  onClose: () => void;
}

export function PublishDialog({ storyId, onClose }: PublishDialogProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [account, setAccount] = useState<Account | null | undefined>(undefined);
  const [plan, setPlan] = useState<PublishPlan | null>(null);
  const [running, setRunning] = useState<PublishMode | null>(null);
  const [step, setStep] = useState("");
  const [outcome, setOutcome] = useState<PublishOutcome | null>(null);
  const [choices, setChoices] = useState<Record<string, PublishChoice>>({});
  const [error, setError] = useState("");
  // The mode a paused publish (agreement, conflicts) resumes with.
  const [mode, setMode] = useState<PublishMode>("publish");

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  const refresh = useCallback(() => {
    Promise.all([api.getAccount(), api.publishPreview(storyId)])
      .then(([signedIn, preview]) => {
        setAccount(signedIn);
        setPlan(preview);
      })
      .catch((e) => setError(errorMessage(e)));
  }, [storyId]);
  useEffect(refresh, [refresh]);

  const run = async (
    next: PublishMode,
    decisions: Record<string, PublishChoice>,
  ) => {
    setMode(next);
    setRunning(next);
    setError("");
    setStep("Checking the website");
    try {
      const result = await api.publish(storyId, next, decisions, setStep);
      setOutcome(result);
      if (result.status === "done") refresh();
    } catch (e) {
      // An expired or revoked sign-in sends the writer back to the form.
      if (errorCode(e) === "signInNeeded") setAccount(null);
      setError(errorMessage(e));
    } finally {
      setRunning(null);
    }
  };

  const signOut = async () => {
    await api.signOut().catch((e) => setError(errorMessage(e)));
    setOutcome(null);
    setAccount(null);
  };

  const conflicts = outcome?.status === "conflicts" ? outcome.conflicts : [];
  const answered = conflicts.every((conflict) => choices[conflict.id]);
  const problems =
    outcome?.status === "blocked" ? outcome.problems : (plan?.problems ?? []);

  let body: React.ReactNode;
  let actions: React.ReactNode = null;

  if (account === undefined || !plan) {
    body = <p className="text-ns-ink-secondary">{error ? null : "Loading…"}</p>;
  } else if (running) {
    body = (
      <p role="status" aria-live="polite">
        {step}…
      </p>
    );
  } else if (account === null) {
    body = (
      <SignInForm
        prompt="Sign in with your TheTaleTribe account to publish."
        onSignedIn={setAccount}
      />
    );
  } else if (outcome?.status === "done") {
    const { summary } = outcome;
    const sent = summary.created + summary.updated + summary.deleted;
    body = (
      <div className="space-y-2">
        <p className="font-medium">
          {outcome.public
            ? "Your story is public on TheTaleTribe."
            : "Your story is on TheTaleTribe as a draft. Only you can see it, in the website's editor, when you are signed in there."}
        </p>
        <p className="text-ns-ink-secondary">
          {sent === 0 && summary.released === 0
            ? "Everything was already up to date."
            : `${summary.created} created, ${summary.updated} updated, ${summary.deleted} removed` +
              (summary.images ? `, ${summary.images} images uploaded` : "") +
              (summary.released
                ? `, ${summary.released} chapters released to readers`
                : "") +
              "."}
        </p>
      </div>
    );
    actions = (
      <button
        type="button"
        className="btn btn-primary"
        onClick={() =>
          void api
            // A draft has no public page; the website would answer "Story not found".
            .openWebsite(outcome.public ? "story" : "editor", storyId)
            .catch((e) => setError(errorMessage(e)))
        }
      >
        {outcome.public ? "View on the website" : "Open in the website's editor"}
      </button>
    );
  } else if (outcome?.status === "needsAgreement") {
    body = (
      <p>
        Before your first story goes up, the website asks you to accept its
        writer agreement. Open the website, start a new story there to see the
        agreement, accept it, then come back and try again.
      </p>
    );
    actions = (
      <>
        <button
          type="button"
          className="btn"
          onClick={() =>
            void api
              .openWebsite("agreement")
              .catch((e) => setError(errorMessage(e)))
          }
        >
          Open the website
        </button>
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void run(mode, choices)}
        >
          Try again
        </button>
      </>
    );
  } else if (outcome?.status === "gone") {
    body = (
      <p>
        The story this was published as is no longer on the website, or
        belongs to a different account. You can publish it again as a new
        story.
      </p>
    );
    actions = (
      <button
        type="button"
        className="btn btn-primary"
        onClick={() => void run(mode, { ...choices, [RECREATE]: "overwrite" })}
      >
        Publish as a new story
      </button>
    );
  } else if (conflicts.length > 0) {
    body = (
      <Conflicts
        conflicts={conflicts}
        choices={choices}
        onChoose={(id, choice) => setChoices({ ...choices, [id]: choice })}
      />
    );
    actions = (
      <button
        type="button"
        className="btn btn-primary"
        disabled={!answered}
        onClick={() => void run(mode, choices)}
      >
        Continue
      </button>
    );
  } else if (problems.length > 0) {
    body = (
      <div className="space-y-2">
        <p>Fix these before publishing:</p>
        <ul className="list-disc space-y-1 pl-5">
          {problems.map((problem) => (
            <li key={problem}>{problem}</li>
          ))}
        </ul>
      </div>
    );
  } else {
    const lines = [
      plan.storyChanged && (plan.linked ? "Title or description" : null),
      describe(plan.chapters, "chapter", "chapters"),
      describe(plan.characters, "character", "characters"),
      describe(plan.places, "place", "places"),
      plan.imagesToUpload > 0 &&
        `${plan.imagesToUpload} ${plan.imagesToUpload === 1 ? "image" : "images"} to upload`,
    ].filter(Boolean);
    body = (
      <div className="space-y-3">
        <p>
          {plan.linked
            ? "This story is already on TheTaleTribe. Publishing updates that same story."
            : "This will create the story on TheTaleTribe."}
        </p>
        {lines.length > 0 ? (
          <ul className="list-disc space-y-1 pl-5 text-ns-ink-secondary">
            {lines.map((line) => (
              <li key={String(line)}>{line}</li>
            ))}
          </ul>
        ) : (
          <p className="text-ns-ink-secondary">
            Nothing has changed here since the last publish.
          </p>
        )}
        <p className="text-xs text-ns-ink-muted">
          Publish makes the story and every chapter with text visible to
          readers. Send as draft makes nothing new visible
          {plan.linked
            ? ", but edits to chapters readers can already see do reach them."
            : "."}
        </p>
      </div>
    );
    actions = (
      <>
        <button
          type="button"
          className="btn"
          onClick={() => void run("draft", {})}
        >
          Send as draft
        </button>
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => void run("publish", {})}
        >
          Publish
        </button>
      </>
    );
  }

  return (
    <dialog
      ref={dialog}
      onCancel={(event) => {
        event.preventDefault();
        if (!running) onClose();
      }}
      className="w-[32rem] rounded-ns-lg border border-ns-border bg-ns-elevated p-0 text-ns-ink backdrop:bg-black/40"
    >
      <div className="max-h-[75vh] space-y-4 overflow-y-auto p-5">
        <h2 className="text-base font-semibold">Publish to TheTaleTribe</h2>
        {body}
        {error && !running && (
          <p role="alert" className="text-ns-destructive">
            {error}
          </p>
        )}
      </div>
      <div className="flex items-center gap-2 border-t border-ns-border p-3">
        {account && !running && (
          <span className="mr-auto truncate text-xs text-ns-ink-muted">
            {account.email} ·{" "}
            <button type="button" className="underline" onClick={() => void signOut()}>
              Sign out
            </button>
          </span>
        )}
        <span className="ml-auto flex gap-2">
          <button
            type="button"
            className="btn"
            disabled={!!running}
            onClick={onClose}
          >
            {outcome?.status === "done" ? "Close" : "Cancel"}
          </button>
          {account && !running && actions}
        </span>
      </div>
    </dialog>
  );
}
