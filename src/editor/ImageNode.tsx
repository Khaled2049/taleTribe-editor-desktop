import {
  NodeViewWrapper,
  ReactNodeViewRenderer,
  NodeViewProps,
} from "@tiptap/react";
import { ImageSchema } from "./schema/image";
import { displaySrc } from "./assetUrl";
import React, { useCallback, useRef, useState } from "react";
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  PanelLeft,
  PanelRight,
  SeparatorHorizontal,
  WrapText,
  X,
} from "lucide-react";

const MIN_WIDTH = 48;

type DisplayMode = "inline" | "wrap" | "break";
type AlignValue = "left" | "center" | "right";

interface HandleConfig {
  pos: "nw" | "ne" | "sw" | "se";
  xSign: 1 | -1;
  cursor: string;
  cls: string;
}

const HANDLES: HandleConfig[] = [
  {
    pos: "nw",
    xSign: -1,
    cursor: "nwse-resize",
    cls: "top-0 left-0 -translate-x-1/2 -translate-y-1/2",
  },
  {
    pos: "ne",
    xSign: 1,
    cursor: "nesw-resize",
    cls: "top-0 right-0 translate-x-1/2 -translate-y-1/2",
  },
  {
    pos: "sw",
    xSign: -1,
    cursor: "nesw-resize",
    cls: "bottom-0 left-0 -translate-x-1/2 translate-y-1/2",
  },
  {
    pos: "se",
    xSign: 1,
    cursor: "nwse-resize",
    cls: "bottom-0 right-0 translate-x-1/2 translate-y-1/2",
  },
];

function getLayoutClasses(displayMode: DisplayMode, align: AlignValue): string {
  if (displayMode === "inline") {
    return "inline-block align-middle";
  }
  if (displayMode === "wrap") {
    if (align === "right") return "float-right ml-4 mb-2 clear-right";
    // left (or center fallback)
    return "float-left mr-4 mb-2 clear-left";
  }
  // break
  if (align === "left") return "block clear-both mr-auto";
  if (align === "right") return "block clear-both ml-auto";
  return "block clear-both mx-auto";
}

const ImageNodeView: React.FC<NodeViewProps> = ({
  node,
  updateAttributes,
  deleteNode,
  selected,
}) => {
  const displayMode = (node.attrs.displayMode as DisplayMode) || "break";
  const align = (node.attrs.align as AlignValue) || "center";
  const storedWidth = node.attrs.width as number | null;
  const storedHeight = node.attrs.height as number | null;

  const [resizing, setResizing] = useState(false);
  const startXRef = useRef(0);
  const startWidthRef = useRef(0);
  const aspectRatioRef = useRef<number | null>(null);
  const imgRef = useRef<HTMLImageElement>(null);

  const layoutClass = getLayoutClasses(displayMode, align);

  const btnBase =
    "p-1 rounded transition-colors text-ns-ink-secondary hover:text-ns-ink";
  const btnActive = "bg-ns-accent text-white hover:text-white";

  const isInline = displayMode === "inline";
  const isWrapLeft = displayMode === "wrap" && align === "left";
  const isWrapRight = displayMode === "wrap" && align === "right";
  const isBreak = displayMode === "break";
  const isAlignLeft = displayMode === "break" && align === "left";
  const isAlignCenter = displayMode === "break" && align === "center";
  const isAlignRight = displayMode === "break" && align === "right";

  const startResize = useCallback(
    (e: React.MouseEvent, xSign: 1 | -1) => {
      e.preventDefault();
      e.stopPropagation();

      const rect = imgRef.current?.getBoundingClientRect();
      const currentWidth = storedWidth ?? rect?.width ?? 200;
      const currentHeight = storedHeight ?? rect?.height ?? 150;

      startXRef.current = e.clientX;
      startWidthRef.current = currentWidth;
      aspectRatioRef.current =
        currentHeight > 0 ? currentWidth / currentHeight : 16 / 9;
      setResizing(true);

      const onMouseMove = (ev: MouseEvent) => {
        const dx = (ev.clientX - startXRef.current) * xSign;
        const ar = aspectRatioRef.current ?? 1;
        const newWidth = Math.max(MIN_WIDTH, startWidthRef.current + dx);
        const newHeight = newWidth / ar;

        if (imgRef.current) {
          imgRef.current.style.width = `${newWidth}px`;
          imgRef.current.style.height = `${newHeight}px`;
        }
      };

      const onMouseUp = (ev: MouseEvent) => {
        const dx = (ev.clientX - startXRef.current) * xSign;
        const ar = aspectRatioRef.current ?? 1;
        const finalWidth = Math.max(
          MIN_WIDTH,
          Math.round(startWidthRef.current + dx),
        );
        const finalHeight = Math.round(finalWidth / ar);

        updateAttributes({ width: finalWidth, height: finalHeight });
        setResizing(false);
        window.removeEventListener("mousemove", onMouseMove);
        window.removeEventListener("mouseup", onMouseUp);
      };

      window.addEventListener("mousemove", onMouseMove);
      window.addEventListener("mouseup", onMouseUp);
    },
    [storedWidth, storedHeight, updateAttributes],
  );

  return (
    <NodeViewWrapper
      as="figure"
      className={`relative max-w-full ${layoutClass}`}
      style={
        isInline
          ? { display: "inline-block", verticalAlign: "middle" }
          : undefined
      }
      data-drag-handle=""
    >
      {selected && (
        <div className="absolute -top-9 left-1/2 -translate-x-1/2 flex items-center gap-0.5 bg-ns-elevated border border-ns-border rounded-ns shadow-md px-1 py-0.5 z-20 whitespace-nowrap">
          {/* Display mode group */}
          <button
            onMouseDown={(e) => {
              e.preventDefault();
              updateAttributes({ displayMode: "inline" });
            }}
            className={`${btnBase} ${isInline ? btnActive : "hover:bg-ns-surface-hover"}`}
            title="Inline"
          >
            <WrapText className="w-3.5 h-3.5" />
          </button>
          <button
            onMouseDown={(e) => {
              e.preventDefault();
              updateAttributes({ displayMode: "wrap", align: "left" });
            }}
            className={`${btnBase} ${isWrapLeft ? btnActive : "hover:bg-ns-surface-hover"}`}
            title="Wrap left"
          >
            <PanelLeft className="w-3.5 h-3.5" />
          </button>
          <button
            onMouseDown={(e) => {
              e.preventDefault();
              updateAttributes({ displayMode: "wrap", align: "right" });
            }}
            className={`${btnBase} ${isWrapRight ? btnActive : "hover:bg-ns-surface-hover"}`}
            title="Wrap right"
          >
            <PanelRight className="w-3.5 h-3.5" />
          </button>
          <button
            onMouseDown={(e) => {
              e.preventDefault();
              updateAttributes({ displayMode: "break" });
            }}
            className={`${btnBase} ${isBreak ? btnActive : "hover:bg-ns-surface-hover"}`}
            title="Break text"
          >
            <SeparatorHorizontal className="w-3.5 h-3.5" />
          </button>

          {/* Alignment group — only in break mode */}
          {isBreak && (
            <>
              <div className="w-px h-4 bg-ns-border mx-0.5" />
              <button
                onMouseDown={(e) => {
                  e.preventDefault();
                  updateAttributes({ align: "left" });
                }}
                className={`${btnBase} ${isAlignLeft ? btnActive : "hover:bg-ns-surface-hover"}`}
                title="Align left"
              >
                <AlignLeft className="w-3.5 h-3.5" />
              </button>
              <button
                onMouseDown={(e) => {
                  e.preventDefault();
                  updateAttributes({ align: "center" });
                }}
                className={`${btnBase} ${isAlignCenter ? btnActive : "hover:bg-ns-surface-hover"}`}
                title="Align center"
              >
                <AlignCenter className="w-3.5 h-3.5" />
              </button>
              <button
                onMouseDown={(e) => {
                  e.preventDefault();
                  updateAttributes({ align: "right" });
                }}
                className={`${btnBase} ${isAlignRight ? btnActive : "hover:bg-ns-surface-hover"}`}
                title="Align right"
              >
                <AlignRight className="w-3.5 h-3.5" />
              </button>
            </>
          )}

          <div className="w-px h-4 bg-ns-border mx-0.5" />
          <button
            onMouseDown={(e) => {
              e.preventDefault();
              deleteNode();
            }}
            className={`${btnBase} hover:bg-ns-destructive/10 hover:text-ns-destructive`}
            title="Remove image"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      <div className="relative inline-block max-w-full">
        <img
          ref={imgRef}
          src={displaySrc(node.attrs.src as string)}
          alt={(node.attrs.alt as string) || ""}
          title={(node.attrs.title as string) || ""}
          style={{
            width: storedWidth ? storedWidth : undefined,
            height: storedHeight ? storedHeight : undefined,
            maxWidth: "100%",
          }}
          className={`block rounded-sm transition-shadow ${
            selected ? "ring-2 ring-ns-accent ring-offset-2" : ""
          } ${resizing ? "select-none" : ""}`}
          draggable="false"
        />

        {selected &&
          HANDLES.map(({ pos, xSign, cursor, cls }) => (
            <div
              key={pos}
              onMouseDown={(e) => startResize(e, xSign)}
              style={{ cursor }}
              className={`absolute w-3 h-3 rounded-full bg-ns-accent opacity-80 hover:opacity-100 shadow-sm z-10 ${cls}`}
              title="Drag to resize"
            />
          ))}
      </div>
    </NodeViewWrapper>
  );
};

export const ImageNode = ImageSchema.extend({
  addNodeView() {
    return ReactNodeViewRenderer(ImageNodeView);
  },
});
