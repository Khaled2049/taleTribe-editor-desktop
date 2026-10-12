import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, beforeEach, expect, it, vi } from "vitest";
import { PublishDialog } from "./PublishDialog";

const plan = {
  linked: true,
  storyChanged: false,
  chapters: { new: 0, changed: 1, deleted: 0 },
  characters: { new: 0, changed: 0, deleted: 0 },
  places: { new: 0, changed: 0, deleted: 0 },
  imagesToUpload: 0,
  problems: [],
  nothingToSend: false,
};

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

// Braces matter: a function returned from beforeEach is run as a cleanup hook.
beforeEach(() => {
  invoke.mockReset();
});

it("asks about a chapter changed on the website and publishes with the answer", async () => {
  const conflict = {
    id: "chapter:c2",
    kind: "chapter",
    label: "The Storm",
    deletedHere: false,
  };
  const publishes: unknown[] = [];
  invoke.mockImplementation(async (command: string, args: unknown) => {
    if (command === "get_account") return { uid: "u1", email: "a@b.c" };
    if (command === "publish_preview") return plan;
    if (command === "publish") {
      publishes.push(args);
      return publishes.length === 1
        ? { status: "conflicts", conflicts: [conflict] }
        : {
            status: "done",
            remoteId: "r1",
            public: true,
            summary: { created: 0, updated: 1, deleted: 0, images: 0, released: 0 },
          };
    }
    throw new Error(`unexpected command: ${command}`);
  });

  render(<PublishDialog storyId="s1" onClose={() => {}} />);
  expect(await screen.findByText("1 changed chapter")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Publish" }));

  expect(await screen.findByText(/The Storm/)).toBeTruthy();
  const proceed = screen.getByRole("button", { name: "Continue" });
  expect(proceed).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByLabelText("Keep the website's version"));
  fireEvent.click(proceed);

  expect(await screen.findByText("Your story is public on TheTaleTribe.")).toBeTruthy();
  expect(screen.getByRole("button", { name: "View on the website" })).toBeTruthy();
  expect(publishes[1]).toMatchObject({
    storyId: "s1",
    mode: "publish",
    decisions: { "chapter:c2": "skip" },
  });
});

it("shows the sign-in form when there is no account, and never the publish buttons", async () => {
  invoke.mockImplementation(async (command: string) => {
    if (command === "get_account") return null;
    if (command === "publish_preview") return plan;
    throw new Error(`unexpected command: ${command}`);
  });

  render(<PublishDialog storyId="s1" onClose={() => {}} />);
  expect(await screen.findByLabelText("Email")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Publish" })).toBeNull();
});
