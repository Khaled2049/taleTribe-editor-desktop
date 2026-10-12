import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, expect, it, vi } from "vitest";
import { ImportDialog } from "./ImportDialog";

// Stands in for the Rust side, which has its own tests.
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args: unknown) => invoke(command, args),
  Channel: class {
    onmessage = () => {};
  },
}));

beforeAll(() => {
  // jsdom does not lay dialogs out; opening one is all the component needs.
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
});

it("asks before touching a story that is already here, then opens the result", async () => {
  const imports: unknown[] = [];
  invoke.mockImplementation(async (command: string, args: unknown) => {
    if (command === "get_account") return { uid: "u1", email: "a@b.c" };
    if (command === "list_website_stories") {
      return [
        {
          id: "r1",
          title: "The Long Crossing",
          description: "",
          published: true,
          chapterCount: 6,
          wordCount: 16883,
          updatedAt: "",
          localId: "s1",
        },
      ];
    }
    if (command === "import_story") {
      imports.push(args);
      return imports.length === 1
        ? { status: "alreadyHere", storyId: "s1", title: "The Long Crossing" }
        : {
            status: "done",
            storyId: "s1",
            chapters: 6,
            characters: 0,
            places: 0,
            images: 0,
            imagesMissing: 1,
            keptLocal: 2,
            linked: true,
          };
    }
    throw new Error(`unexpected command: ${command}`);
  });
  const opened: string[] = [];

  render(<ImportDialog onClose={() => {}} onOpen={(id) => opened.push(id)} />);
  fireEvent.click(await screen.findByText("The Long Crossing"));
  expect(imports[0]).toMatchObject({ remoteId: "r1", existing: null });

  fireEvent.click(await screen.findByText("Replace it with the website's version"));
  expect(await screen.findByText(/2 chapters that exist only on this computer were kept/)).toBeTruthy();
  expect(screen.getByText(/1 image couldn't be downloaded/)).toBeTruthy();
  expect(imports[1]).toMatchObject({ remoteId: "r1", existing: "replace" });

  fireEvent.click(screen.getByRole("button", { name: "Open story" }));
  expect(opened).toEqual(["s1"]);
});
