import { Editor } from "@tiptap/core";
import { describe, expect, it } from "vitest";
import allNodes from "../../../fixtures/chapters/all-nodes.html?raw";
import { chapterExtensions } from "./index";

const roundTrip = (html: string) => {
  const editor = new Editor({
    element: null,
    extensions: chapterExtensions(),
    content: html,
  });
  const out = editor.getHTML();
  editor.destroy();
  return out;
};

describe("chapter schema", () => {
  // The fixture holds every node and mark. If loading and saving it changes a
  // byte, the app would rewrite chapters it merely opened.
  it("reads and writes the full fixture unchanged", () => {
    expect(roundTrip(allNodes)).toBe(allNodes);
  });

  it("drops markup outside the schema instead of storing it", () => {
    const out = roundTrip(
      '<p onclick="x()">Hi<script>alert(1)</script></p><iframe src="https://example.com"></iframe>',
    );
    expect(out).toBe("<p>Hi</p>");
  });
});
