// Vue du canvas : zoom et déplacement d'un monde où les artboards sont posés côte à côte.

export type View = { zoom: number; x: number; y: number };
export type Point = { x: number; y: number };
export type Size = { width: number; height: number };

export const MIN_ZOOM = 0.1;
export const MAX_ZOOM = 4;
/** Écart entre deux artboards et marge du monde, en px du monde. */
export const ARTBOARD_GAP = 64;
/** Hauteur réservée au libellé au-dessus d'un artboard. */
export const LABEL_HEIGHT = 28;

export function clampZoom(zoom: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
}

/** Zoom autour d'un point de l'écran : le point reste sous le curseur. */
export function zoomAt(view: View, factor: number, point: Point): View {
  const zoom = clampZoom(view.zoom * factor);
  const ratio = zoom / view.zoom;
  return {
    zoom,
    x: point.x - (point.x - view.x) * ratio,
    y: point.y - (point.y - view.y) * ratio,
  };
}

export function panBy(view: View, dx: number, dy: number): View {
  return { ...view, x: view.x + dx, y: view.y + dy };
}

/** Position de chaque artboard dans le monde (de gauche à droite). */
export function layoutArtboards(
  widths: number[],
): { width: number; x: number }[] {
  let x = 0;
  return widths.map((width) => {
    const placed = { width, x };
    x += width + ARTBOARD_GAP;
    return placed;
  });
}

export function worldWidth(widths: number[]): number {
  return widths.length === 0
    ? 0
    : widths.reduce((sum, w) => sum + w, 0) +
        ARTBOARD_GAP * (widths.length - 1);
}

/** Vue qui fait tenir la largeur de tous les artboards (sans agrandir au-delà de 100 %). */
export function fitWidth(viewport: Size, widths: number[], margin = 32): View {
  const content = worldWidth(widths);
  if (content === 0 || viewport.width <= 0)
    return { zoom: 1, x: margin, y: margin };
  const zoom = clampZoom(Math.min(1, (viewport.width - 2 * margin) / content));
  return {
    zoom,
    x: Math.max(margin, (viewport.width - content * zoom) / 2),
    y: margin,
  };
}
