import { createLowlight } from "lowlight";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";

/**
 * Starts empty: the grammars are most of highlight.js's weight and most
 * chapters have no code, so they load when a code block first appears.
 */
export const lowlight = createLowlight();

let loading: Promise<void> | null = null;
let loaded = false;

export const codeGrammarsLoaded = () => loaded;

export function loadCodeGrammars(): Promise<void> {
  loading ??= import("./codeGrammars")
    .then(({ registerCodeGrammars }) => {
      registerCodeGrammars(lowlight);
      loaded = true;
    })
    .catch((error: unknown) => {
      loading = null;
      throw error;
    });
  return loading;
}

/** Walks blocks only; code blocks never sit inside inline content. */
export function containsNodeType(doc: ProseMirrorNode, typeName: string) {
  let found = false;
  doc.descendants((node) => {
    if (found) return false;
    if (node.type.name === typeName) {
      found = true;
      return false;
    }
    return !node.isTextblock;
  });
  return found;
}
