import { Extension } from "@tiptap/core";

type TextAlignValue = "left" | "center" | "right" | "justify";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    editorTypography: {
      setFontFamily: (fontFamily: string) => ReturnType;
      unsetFontFamily: () => ReturnType;
      setFontSize: (fontSize: string) => ReturnType;
      unsetFontSize: () => ReturnType;
      setHighlightColor: (color: string) => ReturnType;
      unsetHighlightColor: () => ReturnType;
      setTextAlign: (align: TextAlignValue) => ReturnType;
      unsetTextAlign: () => ReturnType;
      setLineHeight: (lineHeight: string) => ReturnType;
      setParagraphSpacing: (spacing: string) => ReturnType;
      increaseIndent: () => ReturnType;
      decreaseIndent: () => ReturnType;
      clearTextFormatting: () => ReturnType;
    };
  }
}

export const FontFamilyExtension = Extension.create({
  name: "fontFamily",

  addGlobalAttributes() {
    return [
      {
        types: ["textStyle"],
        attributes: {
          fontFamily: {
            default: null,
            parseHTML: (element) => element.style.fontFamily || null,
            renderHTML: (attributes) => {
              if (!attributes.fontFamily) {
                return {};
              }
              return { style: `font-family: ${attributes.fontFamily}` };
            },
          },
        },
      },
    ];
  },

  addCommands() {
    return {
      setFontFamily:
        (fontFamily) =>
        ({ chain }) =>
          chain().setMark("textStyle", { fontFamily }).run(),
      unsetFontFamily:
        () =>
        ({ chain }) =>
          chain()
            .setMark("textStyle", { fontFamily: null })
            .removeEmptyTextStyle()
            .run(),
    };
  },
});

export const FontSizeExtension = Extension.create({
  name: "fontSize",

  addGlobalAttributes() {
    return [
      {
        types: ["textStyle"],
        attributes: {
          fontSize: {
            default: null,
            parseHTML: (element) => element.style.fontSize || null,
            renderHTML: (attributes) => {
              if (!attributes.fontSize) {
                return {};
              }
              return { style: `font-size: ${attributes.fontSize}` };
            },
          },
        },
      },
    ];
  },

  addCommands() {
    return {
      setFontSize:
        (fontSize) =>
        ({ chain }) =>
          chain().setMark("textStyle", { fontSize }).run(),
      unsetFontSize:
        () =>
        ({ chain }) =>
          chain()
            .setMark("textStyle", { fontSize: null })
            .removeEmptyTextStyle()
            .run(),
    };
  },
});

export const HighlightColorExtension = Extension.create({
  name: "highlightColor",

  addGlobalAttributes() {
    return [
      {
        types: ["textStyle"],
        attributes: {
          backgroundColor: {
            default: null,
            parseHTML: (element) => element.style.backgroundColor || null,
            renderHTML: (attributes) => {
              if (!attributes.backgroundColor) {
                return {};
              }
              return {
                style: `background-color: ${attributes.backgroundColor}`,
              };
            },
          },
        },
      },
    ];
  },

  addCommands() {
    return {
      setHighlightColor:
        (color) =>
        ({ chain }) =>
          chain().setMark("textStyle", { backgroundColor: color }).run(),
      unsetHighlightColor:
        () =>
        ({ chain }) =>
          chain()
            .setMark("textStyle", { backgroundColor: null })
            .removeEmptyTextStyle()
            .run(),
    };
  },
});

export const TextAlignExtension = Extension.create({
  name: "textAlign",

  addGlobalAttributes() {
    return [
      {
        types: ["paragraph", "heading"],
        attributes: {
          textAlign: {
            default: null,
            parseHTML: (element) => element.style.textAlign || null,
            renderHTML: (attributes) => {
              if (!attributes.textAlign) {
                return {};
              }
              return { style: `text-align: ${attributes.textAlign}` };
            },
          },
        },
      },
    ];
  },

  addCommands() {
    return {
      setTextAlign:
        (align) =>
        ({ commands }) =>
          commands.updateAttributes("paragraph", { textAlign: align }) ||
          commands.updateAttributes("heading", { textAlign: align }),
      unsetTextAlign:
        () =>
        ({ commands }) =>
          commands.updateAttributes("paragraph", { textAlign: null }) ||
          commands.updateAttributes("heading", { textAlign: null }),
    };
  },
});

export const ParagraphStyleExtension = Extension.create({
  name: "paragraphStyle",

  addGlobalAttributes() {
    return [
      {
        types: ["paragraph", "heading"],
        attributes: {
          lineHeight: {
            default: null,
            parseHTML: (element) => element.style.lineHeight || null,
            renderHTML: (attributes) => {
              if (!attributes.lineHeight) {
                return {};
              }
              return { style: `line-height: ${attributes.lineHeight}` };
            },
          },
          paragraphSpacing: {
            default: null,
            parseHTML: (element) => element.style.marginBottom || null,
            renderHTML: (attributes) => {
              if (!attributes.paragraphSpacing) {
                return {};
              }
              return { style: `margin-bottom: ${attributes.paragraphSpacing}` };
            },
          },
          indent: {
            default: 0,
            parseHTML: (element) => {
              const value = element.getAttribute("data-indent");
              return value ? parseInt(value, 10) : 0;
            },
            renderHTML: (attributes) => {
              const indent = Number(attributes.indent || 0);
              if (!indent) {
                return {};
              }
              return {
                "data-indent": indent,
                style: `margin-left: ${indent * 1.5}rem`,
              };
            },
          },
        },
      },
    ];
  },

  addCommands() {
    return {
      setLineHeight:
        (lineHeight) =>
        ({ commands }) =>
          commands.updateAttributes("paragraph", { lineHeight }) ||
          commands.updateAttributes("heading", { lineHeight }),
      setParagraphSpacing:
        (spacing) =>
        ({ commands }) =>
          commands.updateAttributes("paragraph", {
            paragraphSpacing: spacing,
          }) ||
          commands.updateAttributes("heading", { paragraphSpacing: spacing }),
      increaseIndent:
        () =>
        ({ state, commands }) => {
          const parent = state.selection.$from.parent;
          if (
            parent.type.name !== "paragraph" &&
            parent.type.name !== "heading"
          ) {
            return false;
          }
          const currentIndent = Number(parent.attrs.indent || 0);
          return commands.updateAttributes(parent.type.name, {
            indent: Math.min(currentIndent + 1, 8),
          });
        },
      decreaseIndent:
        () =>
        ({ state, commands }) => {
          const parent = state.selection.$from.parent;
          if (
            parent.type.name !== "paragraph" &&
            parent.type.name !== "heading"
          ) {
            return false;
          }
          const currentIndent = Number(parent.attrs.indent || 0);
          return commands.updateAttributes(parent.type.name, {
            indent: Math.max(currentIndent - 1, 0),
          });
        },
      clearTextFormatting:
        () =>
        ({ chain }) =>
          chain()
            .unsetAllMarks()
            .clearNodes()
            .unsetTextAlign()
            .setLineHeight("1.8")
            .setParagraphSpacing("0")
            .run(),
    };
  },
});
