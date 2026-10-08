// Protocole entre l'éditeur et l'iframe du canvas (ADR 0001 § 8). L'iframe est isolée
// (`sandbox="allow-scripts"`, origine opaque) : les deux côtés vérifient la fenêtre émettrice,
// puis la forme du message avec les gardes ci-dessous. Chaque message voyage dans une enveloppe
// versionnée, pour ignorer les autres messages de la page.

import type { CanvasPage } from "@deep-atelier/ir-types";

export const CHANNEL = "deep-atelier/canvas@1";

/** Design : l'éditeur capte la souris (sélection, survol). Preview : le site est interactif. */
export type CanvasMode = "design" | "preview";

/** Rectangle d'un élément, en pixels CSS, dans le document de l'iframe. */
export type Rect = { x: number; y: number; width: number; height: number };

/** Messages de l'éditeur vers l'iframe. */
export type HostMessage =
  | { type: "render"; page: CanvasPage }
  | { type: "mode"; mode: CanvasMode }
  /** Élément sous un point (coordonnées du document de l'iframe). */
  | { type: "hit_test"; id: number; x: number; y: number };

/** Messages de l'iframe vers l'éditeur. */
export type FrameMessage =
  | { type: "ready" }
  /** Fin d'un rendu ou d'une mise en page : hauteur du document, débordement horizontal. */
  | { type: "layout"; height: number; overflow: boolean }
  /** Rectangles de tous les éléments sélectionnables, par clé. */
  | { type: "rects"; rects: Record<string, Rect> }
  | { type: "hit"; id: number; key: string | null }
  /** Lien vers une page du site, suivi en mode Preview. */
  | { type: "navigate"; page: string };

export type Envelope<M> = { channel: typeof CHANNEL; message: M };

export function wrap<M extends HostMessage | FrameMessage>(
  message: M,
): Envelope<M> {
  return { channel: CHANNEL, message };
}

type Record_ = Record<string, unknown>;

function isRecord(value: unknown): value is Record_ {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isRect(value: unknown): value is Rect {
  return (
    isRecord(value) &&
    isFiniteNumber(value.x) &&
    isFiniteNumber(value.y) &&
    isFiniteNumber(value.width) &&
    isFiniteNumber(value.height)
  );
}

/** Message d'une enveloppe du canal, ou `null`. */
function unwrap(data: unknown): Record_ | null {
  if (!isRecord(data) || data.channel !== CHANNEL) return null;
  return isRecord(data.message) ? data.message : null;
}

/** Message de l'éditeur reçu par l'iframe, ou `null` s'il est mal formé. */
export function readHostMessage(data: unknown): HostMessage | null {
  const message = unwrap(data);
  if (!message) return null;
  switch (message.type) {
    case "render":
      return isRecord(message.page) &&
        typeof message.page.page === "string" &&
        typeof message.page.css === "string" &&
        Array.isArray(message.page.nodes)
        ? (message as HostMessage)
        : null;
    case "mode":
      return message.mode === "design" || message.mode === "preview"
        ? (message as HostMessage)
        : null;
    case "hit_test":
      return isFiniteNumber(message.id) &&
        isFiniteNumber(message.x) &&
        isFiniteNumber(message.y)
        ? (message as HostMessage)
        : null;
    default:
      return null;
  }
}

/** Message de l'iframe reçu par l'éditeur, ou `null` s'il est mal formé. */
export function readFrameMessage(data: unknown): FrameMessage | null {
  const message = unwrap(data);
  if (!message) return null;
  switch (message.type) {
    case "ready":
      return { type: "ready" };
    case "layout":
      return isFiniteNumber(message.height) &&
        message.height >= 0 &&
        typeof message.overflow === "boolean"
        ? { type: "layout", height: message.height, overflow: message.overflow }
        : null;
    case "rects": {
      if (!isRecord(message.rects)) return null;
      const rects: Record<string, Rect> = {};
      for (const [key, rect] of Object.entries(message.rects)) {
        if (!isRect(rect)) return null;
        rects[key] = rect;
      }
      return { type: "rects", rects };
    }
    case "hit":
      return isFiniteNumber(message.id) &&
        (message.key === null || typeof message.key === "string")
        ? { type: "hit", id: message.id, key: message.key }
        : null;
    case "navigate":
      return typeof message.page === "string" && message.page.length > 0
        ? { type: "navigate", page: message.page }
        : null;
    default:
      return null;
  }
}
