import { EditorContent, useEditor } from "@tiptap/react";
import { useMemo } from "react";
import { ImageNode } from "./ImageNode";
import { chapterExtensions } from "./schema";

/**
 * Shows chapter HTML without editing. It goes through the schema rather than
 * `innerHTML`, so only known nodes and marks are ever rendered.
 */
export function ReadOnlyChapter({ content }: { content: string }) {
  const extensions = useMemo(() => chapterExtensions(ImageNode), []);
  const editor = useEditor({ extensions, content, editable: false }, [content]);
  return <EditorContent editor={editor} />;
}
