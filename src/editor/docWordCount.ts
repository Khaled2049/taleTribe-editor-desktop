import { getHTMLFromFragment } from "@tiptap/core";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { chapterWordCount } from "@/lib/limits";

const cache = new WeakMap<ProseMirrorNode, number>();

/** story-data's word count of the document's HTML, cached per immutable doc. */
export function docWordCount(doc: ProseMirrorNode): number {
  const cached = cache.get(doc);
  if (cached !== undefined) return cached;
  const count = chapterWordCount(
    getHTMLFromFragment(doc.content, doc.type.schema),
  );
  cache.set(doc, count);
  return count;
}
