// Routes des pages : chemin saisi (`/blog/[slug]`) ⇄ segments de l'IR.

import type { RouteSegment } from "@deep-atelier/ir-types";

export function formatRoute(route: RouteSegment[]): string {
  if (route.length === 0) return "/";
  return route
    .map((segment) =>
      segment.kind === "Static" ? `/${segment.name}` : `/[${segment.name}]`,
    )
    .join("");
}

/**
 * Segments d'un chemin saisi ; `null` si un segment est mal formé (crochets non fermés, vide).
 * Les règles de nommage (kebab-case, unicité) sont celles de l'IR : le moteur les vérifie.
 */
export function parseRoute(path: string): RouteSegment[] | null {
  const parts = path
    .trim()
    .split("/")
    .filter((part) => part !== "");
  const route: RouteSegment[] = [];
  for (const part of parts) {
    const param = /^\[([^[\]]+)\]$/.exec(part);
    if (param?.[1] !== undefined) {
      route.push({ kind: "Param", name: param[1] });
    } else if (/[[\]\s]/.test(part)) {
      return null;
    } else {
      route.push({ kind: "Static", name: part });
    }
  }
  return route;
}

/** Proposition de route à partir d'un nom de page (« À propos » → `/a-propos`). */
export function slugOf(name: string): string {
  return name
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}
