import type { CanvasPage } from "@deep-atelier/ir-types";
import { describe, expect, it } from "vitest";

import {
  indexCanvas,
  prune,
  select,
  targetOf,
} from "@/lib/editor/canvas-index";

const element = (
  key: string,
  node: string,
  children: CanvasPage["nodes"] = [],
  instance?: string,
): CanvasPage["nodes"][number] => ({
  type: "element",
  key,
  node,
  ...(instance === undefined ? {} : { instance }),
  tag: "div",
  class: "",
  attrs: {},
  children,
});

// Page : section > carte (instance n_inst, racine du composant n_root) > titre (n_title).
const page: CanvasPage = {
  page: "p_1",
  lang: "fr",
  css: "",
  font_stylesheets: [],
  truncated: false,
  nodes: [
    element("n_section", "n_section", [
      element(
        "n_inst/n_root",
        "n_root",
        [
          element("n_inst/n_title", "n_title", [], "n_inst"),
          {
            type: "element",
            tag: "strong",
            class: "",
            attrs: {},
            children: [{ type: "text", text: "gras" }],
          },
        ],
        "n_inst",
      ),
      {
        type: "raw",
        key: "n_raw",
        node: "n_raw",
        code: "<hr />",
        imports: [],
        client: false,
      },
    ]),
  ],
};

describe("canvas index", () => {
  const index = indexCanvas(page);

  it("indexes selectable elements with their parent", () => {
    expect([...index.keys()]).toEqual([
      "n_section",
      "n_inst/n_root",
      "n_inst/n_title",
      "n_raw",
    ]);
    expect(index.get("n_inst/n_title")?.parent).toBe("n_inst/n_root");
    expect(index.get("n_raw")?.parent).toBe("n_section");
  });

  it("selects the page's instance first, its inner element on double click", () => {
    expect(targetOf(index, "n_inst/n_title", false)).toEqual({
      node: "n_inst",
      key: "n_inst/n_root",
    });
    expect(targetOf(index, "n_inst/n_title", true)).toEqual({
      node: "n_title",
      key: "n_inst/n_title",
    });
    expect(targetOf(index, "n_section", false)).toEqual({
      node: "n_section",
      key: "n_section",
    });
    expect(targetOf(index, "missing", false)).toBeNull();
  });

  it("replaces, extends and prunes the selection", () => {
    const a = { node: "n_section", key: "n_section" };
    const b = { node: "n_raw", key: "n_raw" };
    expect(select([a], b, false)).toEqual([b]);
    expect(select([a], b, true)).toEqual([a, b]);
    expect(select([a, b], b, true)).toEqual([a]);
    expect(select([a], null, false)).toEqual([]);
    expect(select([a], null, true)).toEqual([a]);
    const kept = [a, { node: "n_gone", key: "n_gone" }];
    expect(prune(kept, index)).toEqual([a]);
    const same = [a];
    expect(prune(same, index)).toBe(same);
  });
});
