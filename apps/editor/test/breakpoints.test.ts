import { describe, expect, it } from "vitest";

import {
  breakpointOf,
  declaredAt,
  originOf,
  resolveAt,
} from "@/lib/editor/breakpoints";

describe("breakpoints", () => {
  it("maps artboard widths to the breakpoint they show", () => {
    expect(breakpointOf(390)).toBe("base");
    expect(breakpointOf(640)).toBe("sm");
    expect(breakpointOf(768)).toBe("md");
    expect(breakpointOf(1024)).toBe("lg");
    expect(breakpointOf(1280)).toBe("xl");
    expect(breakpointOf(1536)).toBe("2xl");
  });

  it("resolves values mobile-first and reports their origin", () => {
    const gap = { base: "4", md: "8", xl: "12" };
    expect(resolveAt(gap, "base")).toBe("4");
    expect(resolveAt(gap, "sm")).toBe("4");
    expect(resolveAt(gap, "lg")).toBe("8");
    expect(originOf(gap, "lg")).toBe("md");
    expect(resolveAt(gap, "2xl")).toBe("12");
    expect(declaredAt(gap, "lg")).toBeUndefined();
    expect(declaredAt(gap, "md")).toBe("8");
    expect(resolveAt(undefined, "md")).toBeUndefined();
  });
});
