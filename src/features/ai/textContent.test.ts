import { Editor } from "@tiptap/core";
import { expect, it } from "vitest";
import { chapterExtensions } from "@/editor/schema";
import { textToContent } from "./textContent";

const replaceAll = (answer: string) => {
  const editor = new Editor({
    element: null,
    extensions: chapterExtensions(),
    content: "<p>old</p>",
  });
  editor.commands.insertContentAt({ from: 1, to: 4 }, textToContent(answer));
  const html = editor.getHTML();
  editor.destroy();
  return html;
};

it("inserts an answer as text, keeping its paragraphs and neutralising markup", () => {
  expect(replaceAll("One line\nstill one.")).toBe("<p>One line still one.</p>");
  expect(replaceAll("First.\n\nSecond.")).toBe("<p>First.</p><p>Second.</p>");
  expect(replaceAll('<img src=x onerror="alert(1)"> hi')).toBe(
    '<p>&lt;img src=x onerror="alert(1)"&gt; hi</p>',
  );
});
