import { describe, expect, it } from "vitest";

import { locales } from "@/i18n/config";
import { negotiate } from "@/i18n/config";
import en from "@/messages/en.json";
import fr from "@/messages/fr.json";

/** Chemins de toutes les clés (`editor.save.saved`…). */
function keys(value: unknown, prefix = ""): string[] {
  if (typeof value !== "object" || value === null) return [prefix];
  return Object.entries(value).flatMap(([key, child]) =>
    keys(child, prefix ? `${prefix}.${key}` : key),
  );
}

describe("messages", () => {
  it("have the same keys in every language (ADR 0001 § 12, decision 7)", () => {
    const catalogs = { fr, en } as const;
    expect(Object.keys(catalogs).sort()).toEqual([...locales].sort());
    expect(keys(en).sort()).toEqual(keys(fr).sort());
  });

  it("have no empty message", () => {
    for (const catalog of [fr, en]) {
      for (const path of keys(catalog)) {
        const value = path
          .split(".")
          .reduce<unknown>(
            (node, part) => (node as Record<string, unknown>)[part],
            catalog,
          );
        expect(typeof value === "string" && value.trim().length > 0, path).toBe(
          true,
        );
      }
    }
  });

  it("negotiates the language from Accept-Language", () => {
    expect(negotiate("en-US,en;q=0.9,fr;q=0.8")).toBe("en");
    expect(negotiate("de-DE,fr;q=0.5")).toBe("fr");
    expect(negotiate("de-DE")).toBeNull();
    expect(negotiate(null)).toBeNull();
  });
});
