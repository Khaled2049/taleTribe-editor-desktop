import Image from "@tiptap/extension-image";
import {
  NodeSelection,
  TextSelection,
  Selection,
  Plugin,
  PluginKey,
} from "@tiptap/pm/state";

/**
 * The image node without its React view: attributes, keyboard handling and
 * HTML form. Copied from the web app's ImageNode; keep the two in step.
 */
export const ImageSchema = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      displayMode: {
        default: "break",
        parseHTML: (element) => {
          const stored = element.getAttribute("data-display-mode");
          if (stored === "inline" || stored === "wrap" || stored === "break") {
            return stored;
          }
          // Legacy migration: float-aligned images become wrap mode
          const align = element.getAttribute("data-align");
          if (align === "left" || align === "right") return "wrap";
          return "break";
        },
        renderHTML: (attrs) => ({ "data-display-mode": attrs.displayMode }),
      },
      align: {
        default: "center",
        parseHTML: (element) => element.getAttribute("data-align") ?? "center",
        renderHTML: (attrs) => ({ "data-align": attrs.align }),
      },
      width: {
        default: null,
        parseHTML: (element) => {
          const w = element.getAttribute("data-width");
          return w ? parseInt(w, 10) : null;
        },
        renderHTML: (attrs) =>
          attrs.width ? { "data-width": attrs.width } : {},
      },
      height: {
        default: null,
        parseHTML: (element) => {
          const h = element.getAttribute("data-height");
          return h ? parseInt(h, 10) : null;
        },
        renderHTML: (attrs) =>
          attrs.height ? { "data-height": attrs.height } : {},
      },
    };
  },

  addKeyboardShortcuts() {
    const isImageSelected = () => {
      const { selection } = this.editor.state;
      return (
        selection instanceof NodeSelection &&
        selection.node.type.name === this.name
      );
    };

    const moveCursorAfter = (): boolean => {
      if (!isImageSelected()) return false;
      const { state, view } = this.editor;
      const pos = state.selection.to;
      const $pos = state.doc.resolve(pos);
      const found = Selection.findFrom($pos, 1, true);
      if (found) {
        view.dispatch(state.tr.setSelection(found));
        return true;
      }
      const tr = state.tr;
      tr.insert(pos, state.schema.nodes.paragraph.create());
      tr.setSelection(TextSelection.create(tr.doc, pos + 1));
      view.dispatch(tr);
      return true;
    };

    const moveCursorBefore = (): boolean => {
      if (!isImageSelected()) return false;
      const { state, view } = this.editor;
      const $pos = state.doc.resolve(state.selection.from);
      const found = Selection.findFrom($pos, -1, true);
      if (found) {
        view.dispatch(state.tr.setSelection(found));
        return true;
      }
      return false;
    };

    return {
      ArrowLeft: moveCursorBefore,
      ArrowUp: moveCursorBefore,
      ArrowRight: moveCursorAfter,
      ArrowDown: moveCursorAfter,
      Enter: () => {
        if (!isImageSelected()) return false;
        const { state, view } = this.editor;
        const pos = state.selection.to;
        const tr = state.tr;
        tr.insert(pos, state.schema.nodes.paragraph.create());
        tr.setSelection(TextSelection.create(tr.doc, pos + 1));
        view.dispatch(tr);
        return true;
      },
      Escape: moveCursorAfter,
    };
  },

  addProseMirrorPlugins() {
    const extensionName = this.name;
    return [
      new Plugin({
        key: new PluginKey("imageNodeTypeProtect"),
        props: {
          handleKeyDown(view, event) {
            const { selection } = view.state;
            if (!(selection instanceof NodeSelection)) return false;
            if (selection.node.type.name !== extensionName) return false;

            if (
              event.key.length !== 1 ||
              event.ctrlKey ||
              event.metaKey ||
              event.altKey
            ) {
              return false;
            }

            const pos = selection.to;
            const $pos = view.state.doc.resolve(pos);
            const found = Selection.findFrom($pos, 1, true);

            let tr;
            if (found) {
              tr = view.state.tr.setSelection(found).insertText(event.key);
            } else {
              const paragraph = view.state.schema.nodes.paragraph.create(
                null,
                view.state.schema.text(event.key),
              );
              tr = view.state.tr.insert(pos, paragraph);
              tr.setSelection(
                TextSelection.create(tr.doc, pos + 1 + event.key.length),
              );
            }
            view.dispatch(tr);
            return true;
          },
        },
      }),
    ];
  },
});
