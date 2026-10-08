import { describe, expect, it } from "vitest";

import {
  ARTBOARD_GAP,
  MAX_ZOOM,
  MIN_ZOOM,
  fitWidth,
  layoutArtboards,
  panBy,
  worldWidth,
  zoomAt,
} from "@/lib/editor/viewport";

describe("viewport", () => {
  it("zooms around the cursor and clamps the zoom", () => {
    const view = { zoom: 1, x: 100, y: 50 };
    const point = { x: 400, y: 300 };
    const zoomed = zoomAt(view, 2, point);
    expect(zoomed.zoom).toBe(2);
    // Le point du monde sous le curseur ne bouge pas.
    const before = {
      x: (point.x - view.x) / view.zoom,
      y: (point.y - view.y) / view.zoom,
    };
    const after = {
      x: (point.x - zoomed.x) / zoomed.zoom,
      y: (point.y - zoomed.y) / zoomed.zoom,
    };
    expect(after).toEqual(before);
    expect(zoomAt(view, 100, point).zoom).toBe(MAX_ZOOM);
    expect(zoomAt(view, 0.001, point).zoom).toBe(MIN_ZOOM);
    expect(panBy(view, 10, -20)).toEqual({ zoom: 1, x: 110, y: 30 });
  });

  it("lays artboards side by side and fits their width", () => {
    expect(layoutArtboards([390, 768])).toEqual([
      { width: 390, x: 0 },
      { width: 768, x: 390 + ARTBOARD_GAP },
    ]);
    expect(worldWidth([390, 768, 1280])).toBe(
      390 + 768 + 1280 + 2 * ARTBOARD_GAP,
    );
    const fitted = fitWidth({ width: 1200, height: 800 }, [390, 768, 1280]);
    expect(fitted.zoom).toBeLessThan(1);
    expect(fitted.x).toBeGreaterThanOrEqual(32);
    // Jamais agrandi au-delà de 100 %.
    expect(fitWidth({ width: 3000, height: 800 }, [390]).zoom).toBe(1);
  });
});
