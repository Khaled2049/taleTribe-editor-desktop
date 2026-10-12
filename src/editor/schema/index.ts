import type { AnyExtension, Extensions } from "@tiptap/core";
import Blockquote from "@tiptap/extension-blockquote";
import Bold from "@tiptap/extension-bold";
import BulletList from "@tiptap/extension-bullet-list";
import Code from "@tiptap/extension-code";
import Color from "@tiptap/extension-color";
import Document from "@tiptap/extension-document";
import Heading from "@tiptap/extension-heading";
import HorizontalRule from "@tiptap/extension-horizontal-rule";
import Italic from "@tiptap/extension-italic";
import Link from "@tiptap/extension-link";
import ListItem from "@tiptap/extension-list-item";
import OrderedList from "@tiptap/extension-ordered-list";
import Paragraph from "@tiptap/extension-paragraph";
import Strike from "@tiptap/extension-strike";
import Text from "@tiptap/extension-text";
import { TextStyle } from "@tiptap/extension-text-style";
import Typography from "@tiptap/extension-typography";
import Underline from "@tiptap/extension-underline";
import {
  Dropcursor,
  Gapcursor,
  TrailingNode,
  UndoRedo,
} from "@tiptap/extensions";
import { CodeBlockExtension } from "./codeBlock";
import { ImageSchema } from "./image";
import { MarkdownHeadingInputRule } from "./markdownHeadingInputRule";
import { TaskItemExtension, TaskListExtension } from "./taskList";
import {
  FontFamilyExtension,
  FontSizeExtension,
  HighlightColorExtension,
  ParagraphStyleExtension,
  TextAlignExtension,
} from "./typography";

// Curly quotes, em dashes and ellipses only, as on the web.
const SmartTypography = Typography.configure({
  leftArrow: false,
  rightArrow: false,
  copyright: false,
  trademark: false,
  servicemark: false,
  registeredTrademark: false,
  oneHalf: false,
  oneQuarter: false,
  threeQuarters: false,
  plusMinus: false,
  notEqual: false,
  laquo: false,
  raquo: false,
  multiplication: false,
  superscriptTwo: false,
  superscriptThree: false,
});

const HeadingWithoutInputRules = Heading.extend({
  addInputRules() {
    return [];
  },
});

/**
 * The chapter format: every node and mark the web editor's TipTap setup can
 * produce, in the same order and with the same options. This list decides what
 * HTML the app reads and writes, so it must change together with the web's
 * `TipTapEditor.tsx`. `image` lets the editor swap in the node with a view.
 */
export function chapterExtensions(image: AnyExtension = ImageSchema): Extensions {
  return [
    Document,
    UndoRedo,
    Gapcursor,
    Dropcursor,
    TrailingNode,
    Paragraph,
    Text,
    Bold,
    Underline,
    Italic,
    Strike,
    image,
    TextStyle,
    Color,
    FontFamilyExtension,
    FontSizeExtension,
    HighlightColorExtension,
    TextAlignExtension,
    ParagraphStyleExtension,
    // The web passes per-level classes here in a shape TipTap turns into
    // attributes named "1", "2", "3", which a strict DOM rejects. Plain headings
    // parse the same on both sides, so the classes are left out.
    HeadingWithoutInputRules.configure({ levels: [1, 2, 3] }),
    MarkdownHeadingInputRule,
    SmartTypography,
    BulletList.configure({ HTMLAttributes: { class: "list-disc" } }),
    OrderedList.configure({ HTMLAttributes: { class: "list-decimal" } }),
    Blockquote,
    HorizontalRule,
    Link.configure({ openOnClick: false }),
    ListItem,
    Code,
    CodeBlockExtension,
    TaskListExtension,
    TaskItemExtension,
  ];
}
