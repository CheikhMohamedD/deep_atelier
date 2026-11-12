import { describe, expect, it } from "vitest";

import {
  FONT_CATALOG,
  FONT_CATEGORIES,
  defaultWeights,
  findFont,
  googleFont,
} from "../src/index";

describe("font catalogue", () => {
  it("lists about sixty unique families in every category", () => {
    expect(FONT_CATALOG.length).toBeGreaterThanOrEqual(55);
    const names = FONT_CATALOG.map((font) => font.family);
    expect(new Set(names).size).toBe(names.length);
    for (const category of FONT_CATEGORIES) {
      expect(FONT_CATALOG.some((font) => font.category === category)).toBe(
        true,
      );
    }
  });

  it("only lists valid, sorted weights", () => {
    for (const font of FONT_CATALOG) {
      expect(font.weights.length, font.family).toBeGreaterThan(0);
      expect([...font.weights].sort((a, b) => a - b)).toEqual([
        ...font.weights,
      ]);
      for (const weight of font.weights) {
        expect(weight % 100, font.family).toBe(0);
        expect(weight).toBeGreaterThanOrEqual(100);
        expect(weight).toBeLessThanOrEqual(900);
      }
      // Un nom de famille passe tel quel dans l'URL de la feuille de style.
      expect(font.family).toMatch(/^[A-Za-z0-9]+( [A-Za-z0-9]+)*$/);
    }
  });

  it("builds a font token with the weights the family serves", () => {
    const inter = findFont("Inter");
    const anton = findFont("Anton");
    if (!inter || !anton) throw new Error("catalogue incomplet");
    expect(googleFont(inter)).toEqual({
      kind: "Google",
      family: "Inter",
      weights: [400, 700],
    });
    expect(googleFont(inter, [700, 300, 700])).toEqual({
      kind: "Google",
      family: "Inter",
      weights: [300, 700],
    });
    // Une graisse que la famille n'a pas ne part jamais vers Google Fonts.
    expect(googleFont(anton, [700])).toEqual({
      kind: "Google",
      family: "Anton",
      weights: [400],
    });
    expect(defaultWeights(anton)).toEqual([400]);
    expect(findFont("Comic Sans")).toBeUndefined();
  });
});
