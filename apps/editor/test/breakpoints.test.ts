import { describe, expect, it } from "vitest";

import {
  BREAKPOINTS,
  breakpointOfWidth,
  declaredAt,
  overrides,
  resolveAt,
  widthOf,
} from "@/lib/editor/breakpoints";

describe("breakpoints", () => {
  it("maps each artboard width to its breakpoint (ADR 0001 § 12, decision 11)", () => {
    expect(
      [390, 640, 768, 1024, 1280, 1536].map((w) => breakpointOfWidth(w)),
    ).toEqual(["base", "sm", "md", "lg", "xl", "2xl"]);
    for (const breakpoint of BREAKPOINTS) {
      expect(breakpointOfWidth(widthOf(breakpoint))).toBe(breakpoint);
    }
    // Une largeur libre édite le breakpoint qui s'y applique.
    expect(breakpointOfWidth(320)).toBe("base");
    expect(breakpointOfWidth(900)).toBe("md");
    expect(breakpointOfWidth(2000)).toBe("2xl");
  });

  it("resolves a value mobile-first and tells where it comes from", () => {
    const value = { base: "column", md: "row", xl: "row-reverse" };
    expect(resolveAt(value, "base")).toEqual({
      value: "column",
      source: "base",
    });
    expect(resolveAt(value, "sm")).toEqual({ value: "column", source: "base" });
    expect(resolveAt(value, "md")).toEqual({ value: "row", source: "md" });
    expect(resolveAt(value, "lg")).toEqual({ value: "row", source: "md" });
    expect(resolveAt(value, "2xl")).toEqual({
      value: "row-reverse",
      source: "xl",
    });
    expect(resolveAt(undefined, "md")).toBeNull();
  });

  it("keeps a false value as a declared value", () => {
    const visibility = { base: false, md: true };
    expect(resolveAt(visibility, "base")?.value).toBe(false);
    expect(resolveAt(visibility, "lg")?.value).toBe(true);
    expect(declaredAt({ base: true, md: false }, "md")).toBe(true);
  });

  it("tells declared values from inherited ones", () => {
    const value = { base: "4", lg: "8" };
    expect(declaredAt(value, "base")).toBe(true);
    expect(declaredAt(value, "md")).toBe(false);
    expect(declaredAt(value, "lg")).toBe(true);
    expect(declaredAt(undefined, "base")).toBe(false);
    expect(overrides(value)).toEqual(["lg"]);
    expect(overrides({ base: "4" })).toEqual([]);
  });
});
