// Côté iframe du protocole : reçoit les rendus et le mode, applique le thème, mesure les
// éléments, répond aux tests de position et joue le site en mode Preview (bascules, liens).

import {
  readHostMessage,
  wrap,
  type CanvasMode,
  type FrameMessage,
  type Rect,
} from "@deep-atelier/canvas-protocol";
import type { CanvasPage } from "@deep-atelier/ir-types";

const KEY = "data-atl-key";

/** Clé de l'élément sélectionnable le plus proche d'un élément du DOM. */
export function keyOf(element: Element | null): string | null {
  return element?.closest(`[${KEY}]`)?.getAttribute(KEY) ?? null;
}

/** Rectangles de tous les éléments sélectionnables, dans le document. */
export function measure(doc: Document): Record<string, Rect> {
  const view = doc.defaultView;
  const scrollX = view?.scrollX ?? 0;
  const scrollY = view?.scrollY ?? 0;
  const rects: Record<string, Rect> = {};
  for (const element of doc.querySelectorAll(`[${KEY}]`)) {
    const key = element.getAttribute(KEY);
    if (key === null) continue;
    const box = element.getBoundingClientRect();
    rects[key] = {
      x: box.left + scrollX,
      y: box.top + scrollY,
      width: box.width,
      height: box.height,
    };
  }
  return rects;
}

/** Thème (`@tailwindcss/browser`), polices et langue de la page. */
export function applyTheme(doc: Document, page: CanvasPage): void {
  doc.documentElement.lang = page.lang;
  const theme = doc.getElementById("atl-theme");
  if (theme && theme.textContent !== page.css) theme.textContent = page.css;
  for (const href of page.font_stylesheets) {
    const present = [...doc.querySelectorAll("link[data-atl-font]")].some(
      (link) => link.getAttribute("href") === href,
    );
    if (present) continue;
    const link = doc.createElement("link");
    link.rel = "stylesheet";
    link.href = href;
    link.setAttribute("data-atl-font", "");
    doc.head.append(link);
  }
}

/** Bascule la cible d'un bouton (mode Preview). */
export function toggle(doc: Document, button: Element): void {
  const key = button.getAttribute("data-atl-toggles");
  if (key === null) return;
  const target = [...doc.querySelectorAll(`[${KEY}]`)].find(
    (element) => element.getAttribute(KEY) === key,
  );
  if (!target) return;
  const open = target.getAttribute("data-open") !== "true";
  target.setAttribute("data-open", String(open));
  button.setAttribute("aria-expanded", String(open));
}

export class Frame {
  mode: CanvasMode = "design";
  #scheduled = false;
  readonly #win: Window;
  readonly #render: (page: CanvasPage) => void;
  /** Origine de l'éditeur : celle qui a servi l'iframe (son origine propre est opaque). */
  readonly #hostOrigin: string;

  constructor(win: Window, render: (page: CanvasPage) => void) {
    this.#win = win;
    this.#render = render;
    this.#hostOrigin = new URL(win.location.href).origin;
  }

  start(): void {
    const doc = this.#win.document;
    this.#win.addEventListener("message", this.#onMessage);
    doc.addEventListener("click", this.#onClick, true);
    doc.addEventListener("submit", (event) => event.preventDefault(), true);
    this.#win.addEventListener("resize", () => this.schedule());
    doc.addEventListener("load", () => this.schedule(), true);
    // `@tailwindcss/browser` écrit ses styles dans <head> : la mise en page change ensuite.
    new MutationObserver(() => this.schedule()).observe(doc.head, {
      childList: true,
      subtree: true,
      characterData: true,
    });
    if (typeof ResizeObserver !== "undefined") {
      new ResizeObserver(() => this.schedule()).observe(doc.documentElement);
    }
    void doc.fonts?.ready.then(() => this.schedule());
    this.post({ type: "ready" });
  }

  post(message: FrameMessage): void {
    const target = this.#hostOrigin === "null" ? "*" : this.#hostOrigin;
    this.#win.parent.postMessage(wrap(message), target);
  }

  /** Mesure au prochain affichage (une fois par image). */
  schedule(): void {
    if (this.#scheduled) return;
    this.#scheduled = true;
    this.#win.requestAnimationFrame(() => {
      this.#scheduled = false;
      const doc = this.#win.document;
      const root = doc.documentElement;
      this.post({ type: "rects", rects: measure(doc) });
      this.post({
        type: "layout",
        height: Math.ceil(root.scrollHeight),
        overflow: root.scrollWidth > root.clientWidth + 1,
      });
    });
  }

  #onMessage = (event: MessageEvent): void => {
    if (event.source !== this.#win.parent) return;
    const message = readHostMessage(event.data);
    if (!message) return;
    const doc = this.#win.document;
    switch (message.type) {
      case "render":
        applyTheme(doc, message.page);
        this.#render(message.page);
        break;
      case "mode":
        this.mode = message.mode;
        doc.documentElement.setAttribute("data-atl-mode", message.mode);
        break;
      case "hit_test": {
        const view = this.#win;
        const element = doc.elementFromPoint(
          message.x - view.scrollX,
          message.y - view.scrollY,
        );
        this.post({ type: "hit", id: message.id, key: keyOf(element) });
        break;
      }
    }
  };

  #onClick = (event: MouseEvent): void => {
    const target = event.target instanceof Element ? event.target : null;
    const anchor = target?.closest("a");
    if (anchor) {
      // Aucun lien ne quitte le canvas ; en Preview, ils mènent aux pages et aux ancres du site.
      event.preventDefault();
      if (this.mode !== "preview") return;
      const page = anchor.getAttribute("data-page");
      if (page) {
        this.post({ type: "navigate", page });
        return;
      }
      const href = anchor.getAttribute("href") ?? "";
      if (href.startsWith("#") && href.length > 1) {
        this.#win.document
          .getElementById(href.slice(1))
          ?.scrollIntoView({ behavior: "smooth" });
      }
      return;
    }
    const button = target?.closest("[data-atl-toggles]");
    if (button && this.mode === "preview") {
      toggle(this.#win.document, button);
      this.schedule();
    }
  };
}
