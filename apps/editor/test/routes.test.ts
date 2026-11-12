import { describe, expect, it } from "vitest";

import { formatRoute, parseRoute, slugOf } from "@/lib/editor/routes";

describe("routes", () => {
  it("formats and parses static and dynamic segments", () => {
    expect(formatRoute([])).toBe("/");
    const route = parseRoute("/blog/[slug]");
    expect(route).toEqual([
      { kind: "Static", name: "blog" },
      { kind: "Param", name: "slug" },
    ]);
    expect(formatRoute(route ?? [])).toBe("/blog/[slug]");
    expect(parseRoute("/")).toEqual([]);
    expect(parseRoute("  a-propos/ ")).toEqual([
      { kind: "Static", name: "a-propos" },
    ]);
  });

  it("rejects malformed segments", () => {
    expect(parseRoute("/blog/[slug")).toBeNull();
    expect(parseRoute("/blog/slug]")).toBeNull();
    expect(parseRoute("/mon blog")).toBeNull();
    expect(parseRoute("/[]")).toBeNull();
  });

  it("proposes a route from a page name", () => {
    expect(slugOf("À propos")).toBe("a-propos");
    expect(slugOf("  Nos  tarifs ! ")).toBe("nos-tarifs");
    expect(slugOf("!!!")).toBe("");
  });
});
