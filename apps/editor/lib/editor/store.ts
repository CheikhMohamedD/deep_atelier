// État de l'éditeur (Zustand + Immer). Le moteur WebAssembly reste hors de l'état : la session
// (session.ts) le pilote et y reporte ce que l'interface affiche.

import type { CanvasMode, Rect } from "@deep-atelier/canvas-protocol";
import type { CanvasPage } from "@deep-atelier/ir-types";
import { enableMapSet } from "immer";
import { create } from "zustand";
import { immer } from "zustand/middleware/immer";

import type { SaveStatus } from "./autosave";
import {
  indexCanvas,
  prune,
  select,
  type CanvasIndex,
  type Target,
} from "./canvas-index";
import {
  fitWidth,
  panBy,
  zoomAt,
  type Point,
  type Size,
  type View,
} from "./viewport";

enableMapSet();

/** Largeurs proposées (ADR 0001 § 8) ; mobile d'abord. */
export const ALL_WIDTHS = [390, 640, 768, 1024, 1280, 1536] as const;
export const DEFAULT_WIDTHS = [390, 768, 1280];

export type FrameState = {
  height: number;
  overflow: boolean;
  rects: Record<string, Rect>;
};

export type PageInfo = { id: string; name: string };

type Data = {
  phase: "loading" | "ready" | "failed";
  /** Code d'erreur de l'ouverture. */
  failure: string | null;
  projectName: string;
  pages: PageInfo[];
  pageId: string | null;
  canvas: CanvasPage | null;
  index: CanvasIndex;
  selection: Target[];
  hover: Target | null;
  mode: CanvasMode;
  widths: number[];
  frames: Record<string, FrameState>;
  view: View;
  viewport: Size;
  /** La vue a déjà été ajustée une fois à la taille de l'écran. */
  fitted: boolean;
  canUndo: boolean;
  canRedo: boolean;
  save: SaveStatus;
  /** Code de la dernière commande refusée. */
  notice: string | null;
};

type Actions = {
  reset: () => void;
  failed: (code: string) => void;
  ready: (data: {
    projectName: string;
    pages: PageInfo[];
    pageId: string | null;
    canvas: CanvasPage | null;
  }) => void;
  setCanvas: (
    canvas: CanvasPage | null,
    history: { canUndo: boolean; canRedo: boolean },
    pages?: PageInfo[],
  ) => void;
  setPage: (pageId: string, canvas: CanvasPage | null) => void;
  setHover: (target: Target | null) => void;
  select: (target: Target | null, additive: boolean) => void;
  clearSelection: () => void;
  setMode: (mode: CanvasMode) => void;
  toggleWidth: (width: number) => void;
  setFrameLayout: (width: number, height: number, overflow: boolean) => void;
  setFrameRects: (width: number, rects: Record<string, Rect>) => void;
  setViewport: (size: Size) => void;
  zoomAt: (factor: number, point: Point) => void;
  zoomCenter: (factor: number) => void;
  zoomTo: (zoom: number) => void;
  panBy: (dx: number, dy: number) => void;
  fit: () => void;
  setSave: (status: SaveStatus) => void;
  setNotice: (code: string | null) => void;
};

export type EditorState = Data & Actions;

const initial = (): Data => ({
  phase: "loading",
  failure: null,
  projectName: "",
  pages: [],
  pageId: null,
  canvas: null,
  index: new Map(),
  selection: [],
  hover: null,
  mode: "design",
  widths: [...DEFAULT_WIDTHS],
  frames: {},
  view: { zoom: 1, x: 32, y: 32 },
  viewport: { width: 0, height: 0 },
  fitted: false,
  canUndo: false,
  canRedo: false,
  save: "saved",
  notice: null,
});

const emptyFrame = (): FrameState => ({
  height: 0,
  overflow: false,
  rects: {},
});

export const useEditor = create<EditorState>()(
  immer((set) => ({
    ...initial(),
    reset: () => set(() => initial()),
    failed: (code) =>
      set((state) => {
        state.phase = "failed";
        state.failure = code;
      }),
    ready: ({ projectName, pages, pageId, canvas }) =>
      set((state) => {
        state.phase = "ready";
        state.failure = null;
        state.projectName = projectName;
        state.pages = pages;
        state.pageId = pageId;
        state.canvas = canvas;
        state.index = indexCanvas(canvas);
      }),
    setCanvas: (canvas, history, pages) =>
      set((state) => {
        state.canvas = canvas;
        state.index = indexCanvas(canvas);
        state.selection = prune(state.selection, state.index);
        if (state.hover && !state.index.has(state.hover.key))
          state.hover = null;
        state.canUndo = history.canUndo;
        state.canRedo = history.canRedo;
        if (pages) state.pages = pages;
      }),
    setPage: (pageId, canvas) =>
      set((state) => {
        state.pageId = pageId;
        state.canvas = canvas;
        state.index = indexCanvas(canvas);
        state.selection = [];
        state.hover = null;
        state.frames = {};
      }),
    setHover: (target) =>
      set((state) => {
        state.hover = target;
      }),
    select: (target, additive) =>
      set((state) => {
        state.selection = select(state.selection, target, additive);
      }),
    clearSelection: () =>
      set((state) => {
        state.selection = [];
      }),
    setMode: (mode) =>
      set((state) => {
        state.mode = mode;
        state.hover = null;
      }),
    toggleWidth: (width) =>
      set((state) => {
        const on = state.widths.includes(width);
        // Au moins un artboard reste affiché.
        if (on && state.widths.length === 1) return;
        state.widths = on
          ? state.widths.filter((w) => w !== width)
          : [...state.widths, width].sort((a, b) => a - b);
      }),
    setFrameLayout: (width, height, overflow) =>
      set((state) => {
        const frame = (state.frames[String(width)] ??= emptyFrame());
        frame.height = height;
        frame.overflow = overflow;
      }),
    setFrameRects: (width, rects) =>
      set((state) => {
        const frame = (state.frames[String(width)] ??= emptyFrame());
        frame.rects = rects;
      }),
    setViewport: (size) =>
      set((state) => {
        state.viewport = size;
        if (!state.fitted && size.width > 0) {
          state.view = fitWidth(size, state.widths);
          state.fitted = true;
        }
      }),
    zoomAt: (factor, point) =>
      set((state) => {
        state.view = zoomAt(state.view, factor, point);
      }),
    zoomCenter: (factor) =>
      set((state) => {
        state.view = zoomAt(state.view, factor, {
          x: state.viewport.width / 2,
          y: state.viewport.height / 2,
        });
      }),
    zoomTo: (zoom) =>
      set((state) => {
        state.view = zoomAt(state.view, zoom / state.view.zoom, {
          x: state.viewport.width / 2,
          y: state.viewport.height / 2,
        });
      }),
    panBy: (dx, dy) =>
      set((state) => {
        state.view = panBy(state.view, dx, dy);
      }),
    fit: () =>
      set((state) => {
        state.view = fitWidth(state.viewport, state.widths);
      }),
    setSave: (status) =>
      set((state) => {
        state.save = status;
      }),
    setNotice: (code) =>
      set((state) => {
        state.notice = code;
      }),
  })),
);
