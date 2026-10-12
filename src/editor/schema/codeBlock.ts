import type { Editor } from "@tiptap/core";
import { CodeBlockLowlight } from "@tiptap/extension-code-block-lowlight";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import {
  codeGrammarsLoaded,
  containsNodeType,
  loadCodeGrammars,
  lowlight,
} from "./codeHighlighting";

/**
 * The lowlight plugin only recomputes decorations on doc-changing
 * transactions. Re-registering it re-runs its `init` instead, which avoids a
 * transaction that would mark the chapter dirty, add an undo step and bump the
 * assistant bridge's document version.
 */
function refreshHighlighting(editor: Editor) {
  if (editor.isDestroyed) return;
  const plugins = editor.state.plugins;
  const index = plugins.findIndex((plugin) =>
    (plugin as unknown as { key: string }).key.startsWith("lowlight$"),
  );
  if (index === -1) return;
  const plugin = plugins[index];
  editor.unregisterPlugin("lowlight");
  editor.registerPlugin(plugin, (restored, current) => {
    current.splice(index, 0, restored);
    return current;
  });
}

export const CodeBlockExtension = CodeBlockLowlight.extend({
  addProseMirrorPlugins() {
    const editor = this.editor;
    const name = this.name;
    const grammarLoader = new Plugin({
      key: new PluginKey("codeGrammarLoader"),
      view: (view) => {
        const check = () => {
          if (codeGrammarsLoaded()) return;
          if (!containsNodeType(view.state.doc, name)) return;
          loadCodeGrammars()
            .then(() => refreshHighlighting(editor))
            .catch((error) =>
              console.error("Failed to load code highlighting:", error),
            );
        };
        check();
        return {
          update: (_view, previous) => {
            if (view.state.doc !== previous.doc) check();
          },
        };
      },
    });
    return [...(this.parent?.() ?? []), grammarLoader];
  },
}).configure({
  lowlight,
  defaultLanguage: null,
  HTMLAttributes: { class: "ns-code-block" },
});
