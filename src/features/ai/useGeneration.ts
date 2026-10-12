import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorCode, errorMessage } from "@/data/api";
import type { AiTask, Generated } from "@/data/types";

export interface GenerationState {
  status: "idle" | "running" | "done" | "error";
  /** The answer so far while running, and in full once done. */
  text: string;
  error: { message: string; code: string | null } | null;
}

const IDLE: GenerationState = { status: "idle", text: "", error: null };

/** Runs one AI request at a time; starting another cancels the one in flight. */
export function useGeneration() {
  const [state, setState] = useState<GenerationState>(IDLE);
  const current = useRef<string | null>(null);

  const cancel = useCallback(() => {
    const id = current.current;
    current.current = null;
    if (id) void api.cancelGeneration(id).catch(() => {});
  }, []);

  const run = useCallback(
    async (task: AiTask): Promise<Generated | null> => {
      cancel();
      const id = crypto.randomUUID();
      current.current = id;
      // Late pieces from a cancelled request must not land in the new answer.
      const live = () => current.current === id;
      setState({ status: "running", text: "", error: null });
      try {
        const result = await api.generate(id, task, (piece) => {
          if (live()) setState((s) => ({ ...s, text: s.text + piece }));
        });
        if (!live()) return null;
        current.current = null;
        setState({ status: "done", text: result.text, error: null });
        return result;
      } catch (error) {
        if (!live()) return null;
        current.current = null;
        setState({
          status: "error",
          text: "",
          error: { message: errorMessage(error), code: errorCode(error) },
        });
        return null;
      }
    },
    [cancel],
  );

  const reset = useCallback(() => {
    cancel();
    setState(IDLE);
  }, [cancel]);

  useEffect(() => cancel, [cancel]);

  return { state, run, reset };
}
