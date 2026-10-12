import type { Editor } from "@tiptap/react";
import { useEditorState } from "@tiptap/react";
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  Bold,
  Code,
  Heading1,
  Heading2,
  Heading3,
  ImagePlus,
  Italic,
  List,
  ListChecks,
  ListOrdered,
  Minus,
  Quote,
  Redo2,
  SquareCode,
  Strikethrough,
  Underline,
  Undo2,
  type LucideIcon,
} from "lucide-react";
import { useRef } from "react";

interface Action {
  label: string;
  icon: LucideIcon;
  run: (editor: Editor) => void;
  active?: (editor: Editor) => boolean;
  disabled?: (editor: Editor) => boolean;
}

const aligned = (editor: Editor, value: string) =>
  editor.isActive({ textAlign: value });

const GROUPS: Action[][] = [
  [
    {
      label: "Undo",
      icon: Undo2,
      run: (e) => e.chain().focus().undo().run(),
      disabled: (e) => !e.can().undo(),
    },
    {
      label: "Redo",
      icon: Redo2,
      run: (e) => e.chain().focus().redo().run(),
      disabled: (e) => !e.can().redo(),
    },
  ],
  ([1, 2, 3] as const).map((level) => ({
    label: `Heading ${level}`,
    icon: [Heading1, Heading2, Heading3][level - 1],
    run: (e: Editor) => e.chain().focus().toggleHeading({ level }).run(),
    active: (e: Editor) => e.isActive("heading", { level }),
  })),
  [
    {
      label: "Bold",
      icon: Bold,
      run: (e) => e.chain().focus().toggleBold().run(),
      active: (e) => e.isActive("bold"),
    },
    {
      label: "Italic",
      icon: Italic,
      run: (e) => e.chain().focus().toggleItalic().run(),
      active: (e) => e.isActive("italic"),
    },
    {
      label: "Underline",
      icon: Underline,
      run: (e) => e.chain().focus().toggleUnderline().run(),
      active: (e) => e.isActive("underline"),
    },
    {
      label: "Strikethrough",
      icon: Strikethrough,
      run: (e) => e.chain().focus().toggleStrike().run(),
      active: (e) => e.isActive("strike"),
    },
    {
      label: "Inline code",
      icon: Code,
      run: (e) => e.chain().focus().toggleCode().run(),
      active: (e) => e.isActive("code"),
    },
  ],
  [
    {
      label: "Bulleted list",
      icon: List,
      run: (e) => e.chain().focus().toggleBulletList().run(),
      active: (e) => e.isActive("bulletList"),
    },
    {
      label: "Numbered list",
      icon: ListOrdered,
      run: (e) => e.chain().focus().toggleOrderedList().run(),
      active: (e) => e.isActive("orderedList"),
    },
    {
      label: "Task list",
      icon: ListChecks,
      run: (e) => e.chain().focus().toggleTaskList().run(),
      active: (e) => e.isActive("taskList"),
    },
  ],
  [
    {
      label: "Quote",
      icon: Quote,
      run: (e) => e.chain().focus().toggleBlockquote().run(),
      active: (e) => e.isActive("blockquote"),
    },
    {
      label: "Code block",
      icon: SquareCode,
      run: (e) => e.chain().focus().toggleCodeBlock().run(),
      active: (e) => e.isActive("codeBlock"),
    },
    {
      label: "Divider",
      icon: Minus,
      run: (e) => e.chain().focus().setHorizontalRule().run(),
    },
  ],
  [
    {
      label: "Align left",
      icon: AlignLeft,
      run: (e) => e.chain().focus().unsetTextAlign().run(),
      active: (e) => aligned(e, "left"),
    },
    {
      label: "Align center",
      icon: AlignCenter,
      run: (e) => e.chain().focus().setTextAlign("center").run(),
      active: (e) => aligned(e, "center"),
    },
    {
      label: "Align right",
      icon: AlignRight,
      run: (e) => e.chain().focus().setTextAlign("right").run(),
      active: (e) => aligned(e, "right"),
    },
  ],
];

const ACTIONS = GROUPS.flat();

interface ToolbarProps {
  editor: Editor;
  onPickImage: (file: File) => void;
}

export function Toolbar({ editor, onPickImage }: ToolbarProps) {
  const fileInput = useRef<HTMLInputElement>(null);
  // Re-renders only when a button's pressed or disabled state changes.
  const flags = useEditorState({
    editor,
    selector: ({ editor }) =>
      ACTIONS.map(
        (action) =>
          (action.active?.(editor) ? "a" : "") +
          (action.disabled?.(editor) ? "d" : ""),
      ).join(","),
  }).split(",");

  let index = 0;
  return (
    <div
      role="toolbar"
      aria-label="Formatting"
      className="flex flex-wrap items-center gap-0.5 border-b border-ns-border bg-ns-surface px-3 py-1.5"
    >
      {GROUPS.map((group, groupIndex) => (
        <div
          key={groupIndex}
          className="flex items-center gap-0.5 border-r border-ns-border pr-1.5 mr-1"
        >
          {group.map((action) => {
            const flag = flags[index++] ?? "";
            const active = flag.includes("a");
            return (
              <button
                key={action.label}
                type="button"
                title={action.label}
                aria-label={action.label}
                aria-pressed={action.active ? active : undefined}
                disabled={flag.includes("d")}
                // Keeps the selection in the text while the button is pressed.
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => action.run(editor)}
                className={`icon-btn ${active ? "bg-ns-accent-subtle text-ns-accent" : ""}`}
              >
                <action.icon size={16} />
              </button>
            );
          })}
        </div>
      ))}
      <button
        type="button"
        title="Insert image"
        aria-label="Insert image"
        className="icon-btn"
        onClick={() => fileInput.current?.click()}
      >
        <ImagePlus size={16} />
      </button>
      <input
        ref={fileInput}
        type="file"
        accept="image/jpeg,image/png,image/webp"
        hidden
        onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = "";
          if (file) onPickImage(file);
        }}
      />
    </div>
  );
}
