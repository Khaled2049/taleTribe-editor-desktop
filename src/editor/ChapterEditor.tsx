import { Extension } from "@tiptap/core";
import Placeholder from "@tiptap/extension-placeholder";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { EditorContent, useEditor, type Editor } from "@tiptap/react";
import { useEffect, useMemo, useRef } from "react";
import { api, errorMessage } from "@/data/api";
import { ALLOWED_IMAGE_TYPES, prepareImageForUpload } from "@/lib/imageUpload";
import { MAX_CHAPTER_IMAGES } from "@/lib/limits";
import { assetSrc } from "./assetUrl";
import { docWordCount } from "./docWordCount";
import { ImageNode } from "./ImageNode";
import { chapterExtensions } from "./schema";
import { Toolbar } from "./Toolbar";

interface ChapterEditorProps {
  /** Read once; mount a new editor (change `key`) to load different text. */
  initialContent: string;
  onChange: (html: string) => void;
  onBlur: () => void;
  onWordCount: (count: number) => void;
  onError: (message: string) => void;
  /** Receives the editor once it exists, and null when it goes away. */
  onEditor?: (editor: Editor | null) => void;
}

function countImages(editor: Editor): number {
  let count = 0;
  editor.state.doc.descendants((node) => {
    if (node.type.name === "image") count += 1;
  });
  return count;
}

export function ChapterEditor(props: ChapterEditorProps) {
  // Editor callbacks are bound once, so they read the latest props through a ref.
  const latest = useRef(props);
  latest.current = props;
  const editorRef = useRef<Editor | null>(null);

  const insertImage = useRef(async (file: File, pos?: number) => {
    const target = editorRef.current;
    if (!target) return;
    const { onError } = latest.current;
    if (!ALLOWED_IMAGE_TYPES.has(file.type)) {
      return onError("Unsupported format. Use JPEG, PNG, or WebP.");
    }
    if (countImages(target) >= MAX_CHAPTER_IMAGES) {
      return onError(`Maximum ${MAX_CHAPTER_IMAGES} images per chapter.`);
    }
    try {
      const name = await api.importImage(await prepareImageForUpload(file));
      // The writer may have opened another chapter while the image was stored.
      if (target.isDestroyed) return;
      const image = { type: "image", attrs: { src: assetSrc(name) } };
      const at = Math.min(
        pos ?? target.state.selection.to,
        target.state.doc.content.size,
      );
      target.chain().focus().insertContentAt(at, image).run();
    } catch (error) {
      onError(errorMessage(error));
    }
  }).current;

  const extensions = useMemo(() => {
    const imageFile = (files: FileList | undefined) =>
      Array.from(files ?? []).find((file) => file.type.startsWith("image/"));
    const ImagePaste = Extension.create({
      name: "imagePaste",
      addProseMirrorPlugins() {
        return [
          new Plugin({
            key: new PluginKey("imagePaste"),
            props: {
              handlePaste(_view, event) {
                const file = imageFile(event.clipboardData?.files);
                if (!file) return false;
                event.preventDefault();
                void insertImage(file);
                return true;
              },
              handleDrop(view, event, _slice, moved) {
                // `moved` is a drag of existing content within the page.
                if (moved) return false;
                const file = imageFile(event.dataTransfer?.files);
                if (!file) return false;
                event.preventDefault();
                const drop = view.posAtCoords({
                  left: event.clientX,
                  top: event.clientY,
                });
                void insertImage(file, drop?.pos);
                return true;
              },
            },
          }),
        ];
      },
    });
    return [
      ...chapterExtensions(ImageNode),
      ImagePaste,
      Placeholder.configure({ placeholder: "Start writing…" }),
    ];
  }, [insertImage]);

  const editor = useEditor({
    extensions,
    content: props.initialContent,
    autofocus: "end",
    onCreate: ({ editor }) =>
      latest.current.onWordCount(docWordCount(editor.state.doc)),
    onUpdate: ({ editor }) => {
      latest.current.onChange(editor.getHTML());
      latest.current.onWordCount(docWordCount(editor.state.doc));
    },
    onBlur: () => latest.current.onBlur(),
  });
  useEffect(() => {
    editorRef.current = editor;
    latest.current.onEditor?.(editor);
    return () => latest.current.onEditor?.(null);
  }, [editor]);

  if (!editor) return null;
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Toolbar editor={editor} onPickImage={(file) => void insertImage(file)} />
      <div className="flex-1 overflow-y-auto">
        <EditorContent
          editor={editor}
          className="mx-auto max-w-[42rem] px-8 py-10"
        />
      </div>
    </div>
  );
}
