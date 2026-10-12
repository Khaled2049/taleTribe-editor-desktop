import { useCallback, useEffect, useRef, useState } from "react";

export type SaveState = "saved" | "pending" | "saving" | "error";

/**
 * Debounced, serialized saving of the latest value. A failed save keeps its
 * value and is retried by the next `schedule` or `flush`, so text is never
 * dropped because one write failed.
 */
export function useAutosave<T>(
  save: (value: T) => Promise<void>,
  delayMs = 800,
) {
  const [state, setState] = useState<SaveState>("saved");
  const saveRef = useRef(save);
  saveRef.current = save;
  const pending = useRef<{ value: T } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const inFlight = useRef<Promise<boolean> | null>(null);

  const drain = useCallback((): Promise<boolean> => {
    // A running drain picks up anything scheduled while it saves.
    if (inFlight.current) return inFlight.current;
    const run = (async () => {
      while (pending.current) {
        const next = pending.current;
        pending.current = null;
        setState("saving");
        try {
          await saveRef.current(next.value);
        } catch (error) {
          console.error("Autosave failed:", error);
          pending.current ??= next;
          setState("error");
          return false;
        }
      }
      setState("saved");
      return true;
    })().finally(() => {
      inFlight.current = null;
    });
    inFlight.current = run;
    return run;
  }, []);

  const schedule = useCallback(
    (value: T) => {
      pending.current = { value };
      setState((current) => (current === "saving" ? current : "pending"));
      clearTimeout(timer.current);
      timer.current = setTimeout(() => void drain(), delayMs);
    },
    [delayMs, drain],
  );

  /** Saves now. Resolves false if the save failed and the value is still unsaved. */
  const flush = useCallback((): Promise<boolean> => {
    clearTimeout(timer.current);
    return drain();
  }, [drain]);

  useEffect(() => () => void flush(), [flush]);

  return { state, schedule, flush };
}
