// Breakpoints de l'IR (alignés sur Tailwind), largeur d'artboard de chacun, et lecture d'une
// valeur responsive : valeur effective à un breakpoint et breakpoint qui la porte.

import type { Breakpoint, Responsive } from "@deep-atelier/ir-types";

/** Du plus étroit au plus large (mobile d'abord). */
export const BREAKPOINTS: readonly Breakpoint[] = [
  "base",
  "sm",
  "md",
  "lg",
  "xl",
  "2xl",
];

/** Largeur de l'artboard qui montre chaque breakpoint (ADR 0001 § 12, question 11). */
const WIDTHS: Record<Breakpoint, number> = {
  base: 390,
  sm: 640,
  md: 768,
  lg: 1024,
  xl: 1280,
  "2xl": 1536,
};

export function widthOf(breakpoint: Breakpoint): number {
  return WIDTHS[breakpoint];
}

/** Breakpoint édité quand on travaille dans un artboard de cette largeur. */
export function breakpointOfWidth(width: number): Breakpoint {
  let found: Breakpoint = "base";
  for (const breakpoint of BREAKPOINTS) {
    if (WIDTHS[breakpoint] <= width) found = breakpoint;
  }
  return found;
}

/** Valeur effective à un breakpoint, et breakpoint qui la déclare (héritage mobile-first). */
export type Resolved<T> = { value: T; source: Breakpoint };

export function resolveAt<T>(
  responsive: Responsive<T> | undefined,
  breakpoint: Breakpoint,
): Resolved<T> | null {
  if (!responsive) return null;
  let resolved: Resolved<T> = { value: responsive.base, source: "base" };
  for (const candidate of BREAKPOINTS) {
    if (candidate !== "base") {
      const value = responsive[candidate];
      if (value !== undefined) resolved = { value, source: candidate };
    }
    if (candidate === breakpoint) break;
  }
  return resolved;
}

/** Vrai si la valeur est déclarée à ce breakpoint précis (et pas héritée d'un plus étroit). */
export function declaredAt<T>(
  responsive: Responsive<T> | undefined,
  breakpoint: Breakpoint,
): boolean {
  return responsive !== undefined && responsive[breakpoint] !== undefined;
}

/** Breakpoints au-dessus de `base` qui surchargent la valeur. */
export function overrides<T>(
  responsive: Responsive<T> | undefined,
): Breakpoint[] {
  if (!responsive) return [];
  return BREAKPOINTS.filter(
    (breakpoint) =>
      breakpoint !== "base" && responsive[breakpoint] !== undefined,
  );
}
