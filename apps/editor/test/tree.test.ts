import { Engine, loadEngine } from "@deep-atelier/engine";
import type { Document } from "@deep-atelier/ir-types";
import { readFile } from "node:fs/promises";
import { beforeAll, describe, expect, it } from "vitest";

import { indexCanvas } from "@/lib/editor/canvas-index";
import { userTransaction } from "@/lib/editor/session";
import {
  ancestors,
  isContentNode,
  isDetached,
  isLocked,
  kindKey,
  layerLabel,
  layerRoots,
  rootNodes,
  targetOfNode,
  visibilityState,
} from "@/lib/editor/tree";

let engine: Engine;
let doc: Document;
let ids: Record<string, string>;

beforeAll(async () => {
  await loadEngine(
    await readFile(
      new URL("../../../packages/engine/pkg/engine_bg.wasm", import.meta.url),
    ),
  );
  engine = Engine.blank("Site");
  const root = engine.document().pages[0]?.root ?? "";
  const result = engine.apply(
    userTransaction("seed", [
      {
        op: "insert_nodes",
        parent: root,
        nodes: [
          {
            ref: "$hero",
            kind: { type: "Stack", role: { kind: "Section" } },
            meta: { name: "Hero" },
          },
          {
            ref: "$title",
            parent: "$hero",
            kind: {
              type: "Text",
              role: { kind: "Heading", level: "h1" },
              content: [
                {
                  text: "Un titre assez long pour être coupé dans les calques",
                },
              ],
            },
          },
          {
            ref: "$cta",
            parent: "$hero",
            kind: {
              type: "Link",
              href: { kind: "External", url: "https://example.com" },
              label: "Commencer",
              new_tab: false,
            },
          },
          {
            ref: "$box",
            parent: "$hero",
            kind: { type: "Box", role: { kind: "Generic" } },
          },
        ],
      },
    ]),
  );
  if (!result.ok) throw new Error(result.error.message);
  ids = result.applied.created;
  doc = engine.document();
});

describe("layer tree", () => {
  it("lists the page root, and the layout before it when the page has one", () => {
    const page = doc.pages[0];
    expect(layerRoots(doc, page?.id ?? null)).toEqual([
      { kind: "page", name: page?.name, root: page?.root },
    ]);
    expect(layerRoots(doc, "p_missing")).toEqual([]);

    const withLayout = Engine.open(engine.documentJson());
    const pageId = withLayout.document().pages[0]?.id ?? "";
    expect(
      withLayout.apply(
        userTransaction("layout", [{ op: "create_layout", name: "Main" }]),
      ).ok,
    ).toBe(true);
    const layout = withLayout.document().layouts[0];
    expect(
      withLayout.apply(
        userTransaction("use", [
          { op: "update_page", page: pageId, layout: layout?.id ?? null },
        ]),
      ).ok,
    ).toBe(true);
    expect(
      layerRoots(withLayout.document(), pageId).map((root) => root.kind),
    ).toEqual(["layout", "page"]);
    expect(rootNodes(withLayout.document()).has(layout?.root ?? "")).toBe(true);
    withLayout.dispose();
  });

  it("labels layers by name, then by what describes them", () => {
    const node = (ref: string) => {
      const found = doc.nodes[ids[ref] ?? ""];
      if (!found) throw new Error(`nœud ${ref} absent`);
      return found;
    };
    expect(layerLabel(doc, node("$hero"))).toBe("Hero");
    expect(kindKey(node("$hero"))).toBe("Section");
    const title = layerLabel(doc, node("$title"));
    expect(title?.endsWith("…")).toBe(true);
    expect(title?.length).toBe(32);
    expect(kindKey(node("$title"))).toBe("Heading");
    expect(layerLabel(doc, node("$cta"))).toBe("Commencer");
    // Sans nom ni contenu : l'interface affiche le type traduit.
    expect(layerLabel(doc, node("$box"))).toBeNull();
    expect(kindKey(node("$box"))).toBe("Box");
    expect(isContentNode(node("$title"))).toBe(true);
    expect(isContentNode(node("$cta"))).toBe(true);
    expect(isContentNode(node("$box"))).toBe(false);
  });

  it("reads visibility as visible, hidden or per breakpoint", () => {
    expect(visibilityState(undefined)).toBe("visible");
    expect(visibilityState({ base: true })).toBe("visible");
    expect(visibilityState({ base: false })).toBe("hidden");
    expect(visibilityState({ base: false, md: true })).toBe("partial");
  });

  it("finds the canvas target of a node, or a detached target", () => {
    const pageId = doc.pages[0]?.id ?? "";
    const index = indexCanvas(engine.canvasPage(pageId));
    const hero = ids.$hero ?? "";
    const target = targetOfNode(index, hero);
    expect(target.node).toBe(hero);
    expect(index.get(target.key)?.node).toBe(hero);
    expect(isDetached(target.key)).toBe(false);
    const missing = targetOfNode(index, "n_missing");
    expect(isDetached(missing.key)).toBe(true);
  });

  it("treats the children of a locked layer as locked", () => {
    const hero = ids.$hero ?? "";
    const title = ids.$title ?? "";
    expect(ancestors(doc, title)[0]).toBe(hero);
    expect(isLocked(doc, title)).toBe(false);
    const locked = Engine.open(engine.documentJson());
    expect(
      locked.apply(
        userTransaction("lock", [
          { op: "set_meta", node: hero, meta: { locked: true } },
        ]),
      ).ok,
    ).toBe(true);
    expect(isLocked(locked.document(), title)).toBe(true);
    expect(isLocked(locked.document(), hero)).toBe(true);
    locked.dispose();
  });

  it("only lets content through in content mode", () => {
    const scoped = Engine.open(engine.documentJson());
    const title = ids.$title ?? "";
    expect(
      scoped.apply(
        userTransaction(
          "text",
          [
            {
              op: "set_props",
              node: title,
              props: { type: "Text", content: [{ text: "Nouveau titre" }] },
            },
          ],
          true,
        ),
      ).ok,
    ).toBe(true);
    const refused = scoped.apply(
      userTransaction(
        "style",
        [
          {
            op: "set_style",
            node: title,
            style: { font_size: { base: "4xl" } },
          },
        ],
        true,
      ),
    );
    expect(refused.ok).toBe(false);
    if (!refused.ok) expect(refused.error.code).toBe("OUT_OF_SCOPE");
    scoped.dispose();
  });
});
