// Design tokens côté éditeur. Étape (d) : le catalogue de polices embarqué, une sélection de
// familles Google Fonts courantes avec leurs graisses, sans clé d'API ni appel réseau
// (ADR 0001 § 12, question 14).

import type { FontFamily } from "@deep-atelier/ir-types";

import { CATALOG } from "./catalog";

export type FontCategory = "sans" | "serif" | "mono" | "display";

export type CatalogFont = {
  family: string;
  category: FontCategory;
  /** Graisses que Google Fonts sert pour cette famille. */
  weights: readonly number[];
};

export const FONT_CATEGORIES: readonly FontCategory[] = [
  "sans",
  "serif",
  "mono",
  "display",
];

export const FONT_CATALOG: readonly CatalogFont[] = CATALOG;

export function findFont(family: string): CatalogFont | undefined {
  return CATALOG.find((font) => font.family === family);
}

/** Graisses chargées par défaut : normal et gras, sinon ce que la famille propose de plus proche. */
export function defaultWeights(font: CatalogFont): number[] {
  const wanted = [400, 700].filter((weight) => font.weights.includes(weight));
  if (wanted.length > 0) return wanted;
  const first = font.weights[0];
  return first === undefined ? [] : [first];
}

/** Token de police pour une famille du catalogue, limité aux graisses qu'elle propose. */
export function googleFont(
  font: CatalogFont,
  weights: readonly number[] = defaultWeights(font),
): FontFamily {
  const kept = [...new Set(weights)]
    .filter((weight) => font.weights.includes(weight))
    .sort((a, b) => a - b);
  return {
    kind: "Google",
    family: font.family,
    weights: kept.length > 0 ? kept : defaultWeights(font),
  };
}
