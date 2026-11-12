"use client";

import {
  readFrameMessage,
  wrap,
  type HostMessage,
} from "@deep-atelier/canvas-protocol";
import { useTranslations } from "next-intl";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

import { breakpointOfWidth } from "@/lib/editor/breakpoints";
import {
  targetOf,
  type CanvasIndex,
  type Target,
} from "@/lib/editor/canvas-index";
import type { EditorSession } from "@/lib/editor/session";
import { useEditor } from "@/lib/editor/store";
import { isContentNode, isLocked } from "@/lib/editor/tree";
import { cn } from "@/lib/utils";

import { Overlay } from "./overlay";

/** Hauteur minimale d'un artboard, en pixels du monde. */
const MIN_HEIGHT = 640;
/** Distance à partir de laquelle un appui devient un glisser qui déplace la vue. */
const DRAG_THRESHOLD = 4;
/** Délai de réponse d'un test de position. */
const HIT_TIMEOUT_MS = 500;

/** Tests de position en attente de la réponse de l'iframe. */
class HitRequests {
  #next = 0;
  readonly #pending = new Map<number, (key: string | null) => void>();

  request(post: (message: HostMessage) => void, x: number, y: number) {
    const id = ++this.#next;
    return new Promise<string | null>((resolve) => {
      this.#pending.set(id, resolve);
      post({ type: "hit_test", id, x, y });
      setTimeout(() => {
        if (this.#pending.delete(id)) resolve(null);
      }, HIT_TIMEOUT_MS);
    });
  }

  resolve(id: number, key: string | null): void {
    const resolve = this.#pending.get(id);
    if (!resolve) return;
    this.#pending.delete(id);
    resolve(key);
  }
}

type ClientPoint = { clientX: number; clientY: number };

/**
 * Cible d'un clic selon le mode. Contenu : l'élément de contenu (texte, image, lien, bouton) le
 * plus proche sous le pointeur. Design : l'instance ou l'élément, ou son premier ancêtre qui
 * n'est pas verrouillé (un calque verrouillé ne se sélectionne que dans les calques).
 */
function resolveTarget(
  index: CanvasIndex,
  key: string,
  deep: boolean,
): Target | null {
  const { doc, mode } = useEditor.getState();
  if (!doc) return null;
  if (mode === "content") {
    let element = index.get(key);
    while (element) {
      const node = doc.nodes[element.node];
      if (node && isContentNode(node)) {
        return { node: element.node, key: element.key };
      }
      element =
        element.parent === undefined ? undefined : index.get(element.parent);
    }
    return null;
  }
  let target = targetOf(index, key, deep);
  while (target && isLocked(doc, target.node)) {
    const parent = index.get(target.key)?.parent;
    target = parent === undefined ? null : targetOf(index, parent, deep);
  }
  return target;
}

export function Artboard({
  width,
  x,
  session,
}: {
  width: number;
  x: number;
  session: EditorSession;
}) {
  const t = useTranslations("editor");
  const frameRef = useRef<HTMLIFrameElement>(null);
  const glassRef = useRef<HTMLDivElement>(null);
  const hits = useRef(new HitRequests());
  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  const suppressClick = useRef(false);
  const hoverFrame = useRef<number | null>(null);
  const [ready, setReady] = useState(false);
  const canvas = useEditor((state) => state.canvas);
  const mode = useEditor((state) => state.mode);
  const active = useEditor(
    (state) => state.breakpoint === breakpointOfWidth(width),
  );
  const frame = useEditor((state) => state.frames[String(width)]);

  // L'iframe a une origine opaque : `*` est la seule cible possible ; seule cette fenêtre la reçoit.
  const post = useCallback((message: HostMessage) => {
    frameRef.current?.contentWindow?.postMessage(wrap(message), "*");
  }, []);

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const source = frameRef.current?.contentWindow;
      if (!source || event.source !== source) return;
      const message = readFrameMessage(event.data);
      if (!message) return;
      const store = useEditor.getState();
      switch (message.type) {
        case "ready":
          setReady(true);
          break;
        case "layout":
          store.setFrameLayout(width, message.height, message.overflow);
          break;
        case "rects":
          store.setFrameRects(width, message.rects);
          break;
        case "hit":
          hits.current.resolve(message.id, message.key);
          break;
        case "navigate":
          session.setPage(message.page);
          break;
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [width, session]);

  useEffect(() => {
    if (ready && canvas) post({ type: "render", page: canvas });
  }, [ready, canvas, post]);

  useEffect(() => {
    if (ready) {
      post({ type: "mode", mode: mode === "preview" ? "preview" : "design" });
    }
  }, [ready, mode, post]);

  useEffect(
    () => () => {
      if (hoverFrame.current !== null) cancelAnimationFrame(hoverFrame.current);
    },
    [],
  );

  /** Point du document de l'iframe sous le pointeur. */
  const local = (event: ClientPoint) => {
    const box = glassRef.current?.getBoundingClientRect();
    if (!box || box.width === 0) return null;
    const zoom = box.width / width;
    return {
      x: (event.clientX - box.left) / zoom,
      y: (event.clientY - box.top) / zoom,
    };
  };

  const targetAt = async (
    event: ClientPoint,
    deep: boolean,
  ): Promise<Target | null> => {
    const point = local(event);
    if (!point) return null;
    const key = await hits.current.request(post, point.x, point.y);
    return key ? resolveTarget(useEditor.getState().index, key, deep) : null;
  };

  function onPointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    if (event.button !== 0 && event.button !== 1) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { x: event.clientX, y: event.clientY, moved: false };
  }

  function onPointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    const current = drag.current;
    if (current) {
      const dx = event.clientX - current.x;
      const dy = event.clientY - current.y;
      if (current.moved || Math.hypot(dx, dy) > DRAG_THRESHOLD) {
        current.moved = true;
        current.x = event.clientX;
        current.y = event.clientY;
        useEditor.getState().panBy(dx, dy);
      }
      return;
    }
    // Survol : un test de position par image affichée.
    if (hoverFrame.current !== null) return;
    const point = { clientX: event.clientX, clientY: event.clientY };
    hoverFrame.current = requestAnimationFrame(() => {
      hoverFrame.current = null;
      void targetAt(point, false).then((target) =>
        useEditor.getState().setHover(target),
      );
    });
  }

  function onPointerUp(event: ReactPointerEvent<HTMLDivElement>) {
    if (drag.current?.moved) suppressClick.current = true;
    drag.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  async function onClick(event: ReactMouseEvent<HTMLDivElement>) {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    const additive = event.shiftKey;
    const target = await targetAt(event, false);
    const store = useEditor.getState();
    // Le breakpoint édité suit l'artboard où l'on travaille (ADR 0001 § 12, question 11).
    store.activateWidth(width);
    store.select(target, additive);
    // Mode Contenu : les champs s'ouvrent dès qu'un élément est choisi (feuille sous 1024 px).
    if (store.mode === "content" && target) store.setSheet("inspector");
  }

  async function onDoubleClick(event: ReactMouseEvent<HTMLDivElement>) {
    const target = await targetAt(event, true);
    if (target) useEditor.getState().select(target, false);
  }

  const height = Math.max(frame?.height ?? 0, MIN_HEIGHT);
  const title = t("frameTitle", { width });
  return (
    <section
      aria-label={title}
      data-artboard={width}
      data-active={active}
      className="absolute top-0"
      style={{ left: x, width }}
    >
      <p
        className="absolute bottom-full left-0 flex items-center gap-2 whitespace-nowrap pb-1 text-muted-foreground"
        style={{ fontSize: "calc(12px / var(--atl-zoom))" }}
      >
        <span className={cn(active && "font-semibold text-selection")}>
          {width} px · {breakpointOfWidth(width)}
        </span>
        {frame?.overflow ? (
          <span className="font-medium text-warning">{t("overflow")}</span>
        ) : null}
      </p>
      <div
        className="relative overflow-hidden bg-white shadow-md"
        style={{
          width,
          height,
          // Artboard du breakpoint édité : liseré d'épaisseur constante à l'écran.
          outline: active
            ? "calc(2px / var(--atl-zoom)) solid var(--selection)"
            : undefined,
          outlineOffset: "calc(2px / var(--atl-zoom))",
        }}
      >
        <iframe
          ref={frameRef}
          title={title}
          src="/canvas/index.html"
          sandbox="allow-scripts"
          className="absolute inset-0 size-full border-0"
        />
        {mode !== "preview" ? (
          <>
            <div
              ref={glassRef}
              data-testid={`glass-${width}`}
              className="absolute inset-0 cursor-default"
              onPointerDown={onPointerDown}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerCancel={onPointerUp}
              onPointerLeave={() => useEditor.getState().setHover(null)}
              onClick={(event) => void onClick(event)}
              onDoubleClick={(event) => void onDoubleClick(event)}
            />
            <Overlay width={width} />
          </>
        ) : null}
      </div>
    </section>
  );
}
