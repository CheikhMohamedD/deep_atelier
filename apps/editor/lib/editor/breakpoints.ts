// Breakpoints de l'IR côté éditeur : largeur d'artboard → breakpoint édité, cascade mobile-first
// d'une valeur responsive (même règle que `Responsive::resolve` en Rust).

import type { Breakpoint, Responsive } from "@deep-atelier/ir-types";

/** Du plus petit au plus grand. */
export const BREAKPOINTS: readonly Breakpoint[] = [
  "base",
  "sm",
  "md",
  "lg",
  "xl",
  "2xl",
];

/** Largeur minimale de chaque breakpoint (Tailwind). */
export const MIN_WIDTH: Record<Breakpoint, number> = {
  base: 0,
  sm: 640,
  md: 768,
  lg: 1024,
  xl: 1280,
  "2xl": 1536,
};

/** Breakpoint qui s'applique à une largeur d'artboard (390 → base, 768 → md…). */
export function breakpointOf(width: number): Breakpoint {
  let found: Breakpoint = "base";
  for (const bp of BREAKPOINTS) {
    if (width >= MIN_WIDTH[bp]) found = bp;
  }
  return found;
}

/** Valeur déclarée exactement à ce breakpoint. */
export function declaredAt<T>(
  value: Responsive<T> | null | undefined,
  bp: Breakpoint,
): T | undefined {
  if (!value) return undefined;
  return bp === "base" ? value.base : value[bp];
}

/** Breakpoint d'où vient la valeur effective à `bp` (cascade mobile-first). */
export function originOf<T>(value: Responsive<T>, bp: Breakpoint): Breakpoint {
  const upto = BREAKPOINTS.indexOf(bp);
  for (let i = upto; i > 0; i -= 1) {
    const candidate = BREAKPOINTS[i];
    if (
      candidate !== undefined &&
      value[candidate as Exclude<Breakpoint, "base">] !== undefined
    ) {
      return candidate;
    }
  }
  return "base";
}

/** Valeur effective à `bp` ; `undefined` si la propriété n'est pas définie. */
export function resolveAt<T>(
  value: Responsive<T> | null | undefined,
  bp: Breakpoint,
): T | undefined {
  if (!value) return undefined;
  return declaredAt(value, originOf(value, bp));
}
