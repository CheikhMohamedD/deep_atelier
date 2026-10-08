"use client";

import { useEffect, useRef, type CSSProperties } from "react";

import type { EditorSession } from "@/lib/editor/session";
import { useEditor } from "@/lib/editor/store";
import { layoutArtboards } from "@/lib/editor/viewport";

import { Artboard } from "./artboard";

/** Zone du canvas : artboards côte à côte, zoom (Ctrl/⌘ + molette) et déplacement. */
export function Viewport({ session }: { session: EditorSession }) {
  const ref = useRef<HTMLDivElement>(null);
  const view = useEditor((state) => state.view);
  const widths = useEditor((state) => state.widths);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) {
        useEditor.getState().setViewport({
          width: entry.contentRect.width,
          height: entry.contentRect.height,
        });
      }
    });
    observer.observe(element);
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const box = element.getBoundingClientRect();
      const store = useEditor.getState();
      if (event.ctrlKey || event.metaKey) {
        store.zoomAt(Math.exp(-event.deltaY * 0.01), {
          x: event.clientX - box.left,
          y: event.clientY - box.top,
        });
      } else {
        store.panBy(-event.deltaX, -event.deltaY);
      }
    };
    element.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      observer.disconnect();
      element.removeEventListener("wheel", onWheel);
    };
  }, []);

  const world = {
    transform: `translate(${view.x}px, ${view.y}px) scale(${view.zoom})`,
    "--atl-zoom": String(view.zoom),
  } as CSSProperties;
  return (
    <div
      ref={ref}
      data-testid="viewport"
      className="relative min-h-0 flex-1 touch-none overflow-hidden bg-canvas"
    >
      <div className="absolute left-0 top-0 origin-top-left" style={world}>
        {layoutArtboards(widths).map(({ width, x }) => (
          <Artboard key={width} width={width} x={x} session={session} />
        ))}
      </div>
    </div>
  );
}
