import type { JSONContent } from "@tiptap/core";

/**
 * Model output as editor content. It is inserted as text nodes, never parsed
 * as HTML, so markup in an answer shows up as the characters it is.
 */
export function textToContent(text: string): JSONContent | JSONContent[] {
  const paragraphs = text
    .split(/\n\s*\n/)
    .map((paragraph) => paragraph.replace(/\s*\n\s*/g, " ").trim())
    .filter(Boolean);
  if (paragraphs.length <= 1) {
    return { type: "text", text: paragraphs[0] ?? "" };
  }
  return paragraphs.map((paragraph) => ({
    type: "paragraph",
    content: [{ type: "text", text: paragraph }],
  }));
}
