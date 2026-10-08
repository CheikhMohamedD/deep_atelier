"use client";

import type { Rect } from "@deep-atelier/canvas-protocol";

import { useEditor } from "@/lib/editor/store";

/** Cadres du survol et de la sélection, dessinés par-dessus l'iframe d'un artboard. */
export function Overlay({ width }: { width: number }) {
  const rects = useEditor((state) => state.frames[String(width)]?.rects);
  const selection = useEditor((state) => state.selection);
  const hover = useEditor((state) => state.hover);
  const index = useEditor((state) => state.index);
  if (!rects) return null;
  const selected = new Set(selection.map((target) => target.key));
  const hovered =
    hover && !selected.has(hover.key) ? rects[hover.key] : undefined;
  return (
    <div aria-hidden="true" className="pointer-events-none absolute inset-0">
      {hovered ? <Box rect={hovered} kind="hover" /> : null}
      {selection.map((target) => {
        const rect = rects[target.key];
        return rect ? (
          <Box
            key={target.key}
            rect={rect}
            kind="selected"
            label={index.get(target.key)?.tag}
          />
        ) : null;
      })}
    </div>
  );
}

function Box({
  rect,
  kind,
  label,
}: {
  rect: Rect;
  kind: "hover" | "selected";
  label?: string | undefined;
}) {
  // Épaisseur constante à l'écran, quel que soit le zoom.
  const thickness = kind === "selected" ? 2 : 1;
  return (
    <div
      data-overlay={kind}
      className="absolute"
      style={{
        left: rect.x,
        top: rect.y,
        width: rect.width,
        height: rect.height,
        boxShadow: `0 0 0 calc(${thickness}px / var(--atl-zoom)) var(--selection)`,
      }}
    >
      {label ? (
        <span
          className="absolute bottom-full left-0 rounded-sm bg-selection px-1 text-background"
          style={{ fontSize: "calc(11px / var(--atl-zoom))", lineHeight: 1.4 }}
        >
          {label}
        </span>
      ) : null}
    </div>
  );
}
