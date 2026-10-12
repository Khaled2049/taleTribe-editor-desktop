import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAutosave } from "./useAutosave";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("useAutosave", () => {
  it("saves only the latest value once typing pauses", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useAutosave<string>(save, 800));

    act(() => result.current.schedule("a"));
    act(() => result.current.schedule("ab"));
    expect(result.current.state).toBe("pending");
    await act(() => vi.advanceTimersByTimeAsync(800));

    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith("ab");
    expect(result.current.state).toBe("saved");
  });

  it("flush saves immediately without waiting for the timer", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useAutosave<string>(save, 800));

    act(() => result.current.schedule("text"));
    let ok = false;
    await act(async () => {
      ok = await result.current.flush();
    });

    expect(ok).toBe(true);
    expect(save).toHaveBeenCalledWith("text");
  });

  it("keeps a value whose save failed and retries it", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const save = vi
      .fn()
      .mockRejectedValueOnce(new Error("disk full"))
      .mockResolvedValue(undefined);
    const { result } = renderHook(() => useAutosave<string>(save, 800));

    act(() => result.current.schedule("precious"));
    let ok = true;
    await act(async () => {
      ok = await result.current.flush();
    });
    expect(ok).toBe(false);
    expect(result.current.state).toBe("error");

    await act(async () => {
      ok = await result.current.flush();
    });
    expect(ok).toBe(true);
    expect(save).toHaveBeenLastCalledWith("precious");
    expect(result.current.state).toBe("saved");
  });

  it("never runs two saves at once, and saves what arrived meanwhile", async () => {
    let release: () => void = () => {};
    const save = vi
      .fn()
      .mockImplementationOnce(
        () => new Promise<void>((resolve) => (release = resolve)),
      )
      .mockResolvedValue(undefined);
    const { result } = renderHook(() => useAutosave<string>(save, 800));

    act(() => result.current.schedule("first"));
    await act(() => vi.advanceTimersByTimeAsync(800));
    act(() => result.current.schedule("second"));
    await act(() => vi.advanceTimersByTimeAsync(800));
    expect(save).toHaveBeenCalledTimes(1);

    await act(async () => {
      release();
      await result.current.flush();
    });
    expect(save).toHaveBeenCalledTimes(2);
    expect(save).toHaveBeenLastCalledWith("second");
  });
});
