import { Engine, loadEngine } from "@deep-atelier/engine";
import { readFile } from "node:fs/promises";
import { beforeAll, describe, expect, it } from "vitest";

import { userTransaction } from "@/lib/editor/session";
import {
  FIELD_PROPS,
  STATE_ONLY_PROPS,
  STYLE_GROUPS,
} from "@/lib/editor/style-fields";
import en from "@/messages/en.json";
import fr from "@/messages/fr.json";

beforeAll(async () => {
  await loadEngine(
    await readFile(new URL("../public/engine/engine_bg.wasm", import.meta.url)),
  );
});

describe("inspector style fields", () => {
  it("edits each style property exactly once, and names it in both languages", () => {
    expect(new Set(FIELD_PROPS).size).toBe(FIELD_PROPS.length);
    for (const prop of FIELD_PROPS) {
      expect(fr.editor.props, prop).toHaveProperty(prop);
      expect(en.editor.props, prop).toHaveProperty(prop);
      expect(STATE_ONLY_PROPS).not.toContain(prop);
    }
    expect(Object.keys(fr.editor.props).sort()).toEqual(
      [...FIELD_PROPS].sort(),
    );
    for (const group of STYLE_GROUPS) {
      expect(fr.editor.inspector.groups).toHaveProperty(group.id);
    }
  });

  it("covers every property the engine allows on containers, texts and images", () => {
    const engine = Engine.blank("Site");
    const root = engine.document().pages[0]?.root ?? "";
    const result = engine.apply(
      userTransaction("seed", [
        {
          op: "insert_nodes",
          parent: root,
          nodes: [
            {
              ref: "$stack",
              kind: { type: "Stack", role: { kind: "Generic" } },
            },
            {
              ref: "$grid",
              parent: "$stack",
              kind: { type: "Grid", role: { kind: "Generic" } },
            },
            {
              ref: "$text",
              parent: "$grid",
              kind: {
                type: "Text",
                role: { kind: "Paragraph" },
                content: [{ text: "Texte" }],
              },
            },
            {
              ref: "$image",
              parent: "$stack",
              kind: {
                type: "Image",
                source: { kind: "Placeholder", label: "Photo", ratio: "video" },
                alt: "Photo",
                priority: false,
              },
            },
          ],
        },
      ]),
    );
    if (!result.ok) throw new Error(result.error.message);
    const allowed = new Set<string>();
    for (const id of Object.values(result.applied.created)) {
      for (const prop of engine.inspect(id)?.allowed ?? []) allowed.add(prop);
    }
    expect(allowed.size).toBeGreaterThan(40);
    for (const prop of allowed) {
      expect(FIELD_PROPS, prop).toContain(prop);
    }
    engine.dispose();
  });

  it("proposes only values the engine accepts", () => {
    const engine = Engine.blank("Site");
    const root = engine.document().pages[0]?.root ?? "";
    const seeded = engine.apply(
      userTransaction("seed", [
        {
          op: "insert_nodes",
          parent: root,
          nodes: [
            { ref: "$row", kind: { type: "Stack", role: { kind: "Generic" } } },
            {
              ref: "$grid",
              parent: "$row",
              kind: { type: "Grid", role: { kind: "Generic" } },
            },
            {
              ref: "$cell",
              parent: "$grid",
              kind: {
                type: "Text",
                role: { kind: "Paragraph" },
                content: [{ text: "Texte" }],
              },
            },
            {
              ref: "$image",
              parent: "$row",
              kind: {
                type: "Image",
                source: { kind: "Placeholder", label: "Photo", ratio: "video" },
                alt: "Photo",
                priority: false,
              },
            },
          ],
        },
      ]),
    );
    if (!seeded.ok) throw new Error(seeded.error.message);
    const nodes = Object.values(seeded.applied.created);
    const allowedBy = new Map(
      nodes.map((id) => [id, new Set<string>(engine.inspect(id)?.allowed)]),
    );
    for (const group of STYLE_GROUPS) {
      for (const field of group.fields) {
        const control = field.control;
        if (control.kind !== "options" && control.kind !== "combo") continue;
        const node = nodes.find((id) => allowedBy.get(id)?.has(field.prop));
        expect(node, field.prop).toBeDefined();
        for (const option of control.options) {
          // `none` n'est une dimension que pour les bornes maximales.
          const result = engine.apply(
            userTransaction("value", [
              {
                op: "set_style",
                node: node ?? "",
                style: { [field.prop]: { md: option } },
              },
            ]),
          );
          expect(
            result.ok,
            `${field.prop} = ${option}${result.ok ? "" : ` : ${result.error.message}`}`,
          ).toBe(true);
        }
      }
    }
    engine.dispose();
  });
});
