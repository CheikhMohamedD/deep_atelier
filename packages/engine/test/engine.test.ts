import { readFile } from "node:fs/promises";

import type { CanvasNode, Transaction } from "@deep-atelier/ir-types";
import { beforeAll, describe, expect, it } from "vitest";

import { Engine, loadEngine } from "../src/index";

beforeAll(async () => {
  const bytes = await readFile(
    new URL("../pkg/engine_bg.wasm", import.meta.url),
  );
  await loadEngine(bytes);
});

function texts(nodes: CanvasNode[]): string[] {
  return nodes.flatMap((node) =>
    node.type === "text"
      ? [node.text]
      : node.type === "element"
        ? texts(node.children)
        : [],
  );
}

function insertHeading(root: string, text: string): Transaction {
  return {
    label: "Titre",
    origin: { kind: "user" },
    commands: [
      {
        op: "insert_nodes",
        parent: root,
        nodes: [
          {
            ref: "$title",
            kind: {
              type: "Text",
              role: { kind: "Heading", level: "h1" },
              content: [{ text }],
            },
          },
        ],
      },
    ],
  } as Transaction;
}

describe("Engine", () => {
  it("edits a document through transactions, with undo and redo", () => {
    const engine = Engine.blank("Mon site");
    const doc = engine.document();
    const page = doc.pages[0]!;
    expect(engine.canUndo).toBe(false);

    const result = engine.apply(insertHeading(page.root, "Bonjour"));
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    const title = result.applied.created["$title"];
    expect(title).toMatch(/^n_/);
    expect(engine.canUndo).toBe(true);

    const canvas = engine.canvasPage(page.id);
    expect(canvas).not.toBeNull();
    expect(texts(canvas!.nodes)).toContain("Bonjour");

    const undone = engine.undo();
    expect(undone.ok && undone.changes !== null).toBe(true);
    expect(texts(engine.canvasPage(page.id)!.nodes)).not.toContain("Bonjour");
    expect(engine.redo().ok).toBe(true);
    expect(texts(engine.canvasPage(page.id)!.nodes)).toContain("Bonjour");
    engine.dispose();
  });

  it("refuses invalid transactions without changing anything", () => {
    const engine = Engine.blank("Site");
    const before = engine.documentJson();
    const result = engine.apply({
      label: "Fantôme",
      origin: { kind: "user" },
      commands: [{ op: "delete_nodes", nodes: ["n_zzzzzzzzzz"] }],
    } as Transaction);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error.code).toMatch(/^[A-Z_]+$/);
    expect(engine.documentJson()).toBe(before);
    expect(engine.canUndo).toBe(false);
  });

  it("reopens a saved document and validates it", () => {
    const engine = Engine.blank("Site");
    const page = engine.document().pages[0]!;
    engine.apply(insertHeading(page.root, "Persisté"));
    const reopened = Engine.open(engine.documentJson());
    expect(reopened.document()).toEqual(engine.document());
    expect(texts(reopened.canvasPage(page.id)!.nodes)).toContain("Persisté");
    const issues = reopened.validate();
    expect(Array.isArray(issues)).toBe(true);
    expect(issues.every((issue) => typeof issue.code === "string")).toBe(true);
    expect(reopened.canvasPage("p_zzzzzzzzzz")).toBeNull();
    expect(() => Engine.open("{")).toThrow(/INVALID_JSON/);
  });

  it("groups a gesture into a single undo entry", () => {
    const engine = Engine.blank("Site");
    const root = engine.document().pages[0]!.root;
    engine.beginGesture("Glisser");
    for (let i = 0; i < 3; i += 1) {
      const result = engine.apply({
        label: "Boîte",
        origin: { kind: "user" },
        commands: [
          {
            op: "insert_nodes",
            parent: root,
            nodes: [{ kind: { type: "Box" } }],
          },
        ],
      } as Transaction);
      expect(result.ok).toBe(true);
    }
    engine.endGesture();
    expect(engine.undo().ok).toBe(true);
    expect(engine.canUndo).toBe(false);
    expect(Object.keys(engine.document().nodes)).toHaveLength(1);
  });
});
