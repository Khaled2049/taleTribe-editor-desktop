import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, expect, it, vi } from "vitest";
import { App } from "./App";

const chapter = {
  id: "c1",
  storyId: "s1",
  title: "Chapter 1",
  wordCount: 2,
  position: 0,
  updatedAt: 0,
  content: "<p>Once upon</p>",
};
const story = {
  id: "s1",
  title: "The Lighthouse",
  description: "",
  chapterCount: 1,
  wordCount: 2,
  createdAt: 0,
  updatedAt: 0,
};

// Stands in for the Rust side, which has its own tests.
const invoke = vi.fn(async (command: string) => {
  switch (command) {
    case "list_stories":
      return [story];
    case "list_chapters":
      return [chapter];
    case "get_chapter":
      return chapter;
    case "get_account":
      return null;
    default:
      throw new Error(`unexpected command: ${command}`);
  }
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string) => invoke(command),
  convertFileSrc: (path: string) => path,
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ onCloseRequested: async () => () => {} }),
}));

beforeAll(() => {
  // jsdom has no layout; ProseMirror asks for these when it places the caret.
  Range.prototype.getClientRects = () => [] as unknown as DOMRectList;
  Range.prototype.getBoundingClientRect = () => new DOMRect();
  Element.prototype.scrollIntoView = () => {};
  document.elementFromPoint = () => null;
});

it("opens a story from the library into an editor holding its first chapter", async () => {
  render(<App />);

  // Signed out is the normal state: the home screen offers sign-in, never demands it.
  expect(await screen.findByRole("button", { name: "Sign in" })).toBeTruthy();
  fireEvent.click(await screen.findByText("The Lighthouse"));

  expect(await screen.findByText("Once upon")).toBeTruthy();
  expect(screen.getByRole("toolbar", { name: "Formatting" })).toBeTruthy();
  // Once in the chapter list, once in the status bar from the live editor.
  await waitFor(() => expect(screen.getAllByText("2 words")).toHaveLength(2));
  expect(screen.getByRole("status").textContent).toBe("Saved");
});
