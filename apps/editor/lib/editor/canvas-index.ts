// Index des éléments sélectionnables d'une page du canvas, et ce qu'un clic désigne.

import type { CanvasNode, CanvasPage } from "@deep-atelier/ir-types";

/** Élément sélectionnable (clé unique dans la page). */
export type ElementInfo = {
  key: string;
  node: string;
  /** Instance de la page qui contient l'élément. */
  instance: string | undefined;
  tag: string;
  /** Clé de l'élément sélectionnable parent. */
  parent: string | undefined;
};

export type CanvasIndex = Map<string, ElementInfo>;

/** Cible d'une sélection : un nœud de l'IR et l'occurrence (clé) qui le montre. */
export type Target = { node: string; key: string };

export function indexCanvas(page: CanvasPage | null): CanvasIndex {
  const index: CanvasIndex = new Map();
  const visit = (nodes: CanvasNode[], parent: string | undefined) => {
    for (const node of nodes) {
      if (node.type === "element") {
        if (node.key !== undefined && node.node !== undefined) {
          index.set(node.key, {
            key: node.key,
            node: node.node,
            instance: node.instance,
            tag: node.tag,
            parent,
          });
          visit(node.children, node.key);
        } else {
          visit(node.children, parent);
        }
      } else if (node.type === "raw") {
        index.set(node.key, {
          key: node.key,
          node: node.node,
          instance: node.instance,
          tag: "code",
          parent,
        });
      }
    }
  };
  if (page) visit(page.nodes, undefined);
  return index;
}

/**
 * Ce que désigne l'élément `key` : au premier clic, l'instance de la page qui le contient (sa
 * boîte est l'élément le plus haut de cette instance) ; au double-clic (`deep`), l'élément.
 */
export function targetOf(
  index: CanvasIndex,
  key: string,
  deep: boolean,
): Target | null {
  const element = index.get(key);
  if (!element) return null;
  if (deep || element.instance === undefined) {
    return { node: element.node, key };
  }
  let top = element;
  while (top.parent !== undefined) {
    const parent = index.get(top.parent);
    if (!parent || parent.instance !== element.instance) break;
    top = parent;
  }
  return { node: element.instance, key: top.key };
}

/** Sélection après un clic : remplacée, ou complétée et retirée avec Maj. */
export function select(
  selection: Target[],
  target: Target | null,
  additive: boolean,
): Target[] {
  if (!target) return additive ? selection : [];
  if (!additive) return [target];
  return selection.some((t) => t.key === target.key)
    ? selection.filter((t) => t.key !== target.key)
    : [...selection, target];
}

/** Sélection gardée après un rendu : les occurrences qui existent encore. */
export function prune(selection: Target[], index: CanvasIndex): Target[] {
  const kept = selection.filter((t) => index.has(t.key));
  return kept.length === selection.length ? selection : kept;
}
