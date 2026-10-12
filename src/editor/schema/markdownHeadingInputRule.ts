import { Extension, textblockTypeInputRule } from "@tiptap/core";

const HEADING_MARKDOWN_REGEX = /^(#{1,3})\s$/;

export const MarkdownHeadingInputRule = Extension.create({
  name: "markdownHeadingInputRule",

  addInputRules() {
    const headingNode = this.editor.schema.nodes.heading;

    if (!headingNode) {
      return [];
    }

    return [
      textblockTypeInputRule({
        find: HEADING_MARKDOWN_REGEX,
        type: headingNode,
        getAttributes: (match) => ({
          level: match[1].length,
        }),
      }),
    ];
  },
});
