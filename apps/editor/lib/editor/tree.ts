// Arbre des calques d'une page : nœuds de l'IR (layout puis page), libellés, état de
// visibilité, et correspondance entre un nœud et ses rendus dans le canvas.

import type {
  ContainerRole,
  Document,
  Node,
  Responsive,
} from "@deep-atelier/ir-types";

import type { CanvasIndex, Target } from "./canvas-index";

export type LayerRoot = {
  kind: "layout" | "page";
  /** Nom du layout ou de la page. */
  name: string;
  root: string;
};

/** Racines affichées pour une page : son layout (s'il y en a un), puis la page. */
export function layerRoots(doc: Document, pageId: string | null): LayerRoot[] {
  const page = doc.pages.find((candidate) => candidate.id === pageId);
  if (!page) return [];
  const roots: LayerRoot[] = [];
  const layout = doc.layouts.find((candidate) => candidate.id === page.layout);
  if (layout) {
    roots.push({ kind: "layout", name: layout.name, root: layout.root });
  }
  roots.push({ kind: "page", name: page.name, root: page.root });
  return roots;
}

/** Racines de page, de layout et de composant : ni supprimées, ni déplacées. */
export function rootNodes(doc: Document): Set<string> {
  return new Set([
    ...doc.pages.map((page) => page.root),
    ...doc.layouts.map((layout) => layout.root),
    ...doc.components.map((component) => component.root),
  ]);
}

/** Texte brut d'un nœud Text. */
export function plainText(node: Node): string {
  return node.kind.type === "Text"
    ? node.kind.content.map((run) => run.text).join("")
    : "";
}

const EXCERPT = 32;

function excerpt(text: string): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > EXCERPT ? `${flat.slice(0, EXCERPT - 1)}…` : flat;
}

/**
 * Libellé d'un calque : son nom s'il en a un, sinon ce qui le décrit le mieux (extrait du texte,
 * libellé du bouton, nom du composant…). `null` : l'interface affiche le type traduit.
 */
export function layerLabel(doc: Document, node: Node): string | null {
  if (node.meta.name) return node.meta.name;
  const kind = node.kind;
  switch (kind.type) {
    case "Text":
      return excerpt(plainText(node)) || null;
    case "Button":
    case "Link":
      return kind.label ? excerpt(kind.label) : null;
    case "Image":
      return kind.alt ? excerpt(kind.alt) : null;
    case "Icon":
      return kind.name;
    case "Input":
      return kind.name;
    case "Slot":
      return kind.name;
    case "ComponentInstance":
      return (
        doc.components.find((component) => component.id === kind.component)
          ?.name ?? null
      );
    default:
      return null;
  }
}

/** Type d'un calque, tel que l'interface le nomme. */
export type LayerKind =
  Node["kind"]["type"] | Exclude<ContainerRole["kind"], "Generic"> | "Heading";

/** Type affiché d'un nœud (rôle sémantique pour un conteneur, titre pour un texte). */
export function kindKey(node: Node): LayerKind {
  const kind = node.kind;
  if (kind.type === "Box" || kind.type === "Stack" || kind.type === "Grid") {
    return kind.role.kind === "Generic" ? kind.type : kind.role.kind;
  }
  if (kind.type === "Text") {
    return kind.role.kind === "Heading" ? "Heading" : "Text";
  }
  return kind.type;
}

export type VisibilityState = "visible" | "hidden" | "partial";

/** Visible partout, masqué partout, ou selon le breakpoint. */
export function visibilityState(
  visibility: Responsive<boolean> | undefined,
): VisibilityState {
  if (!visibility) return "visible";
  const values = [
    visibility.base,
    visibility.sm,
    visibility.md,
    visibility.lg,
    visibility.xl,
    visibility["2xl"],
  ].filter((value) => value !== undefined);
  if (values.every((value) => value)) return "visible";
  if (values.every((value) => !value)) return "hidden";
  return "partial";
}

/** Préfixe de la clé d'une cible qui n'a pas de rendu dans la page affichée. */
const DETACHED = "node:";

export function isDetached(key: string): boolean {
  return key.startsWith(DETACHED);
}

/**
 * Cible de sélection d'un nœud choisi dans les calques : son premier rendu dans le canvas (pour
 * une instance, l'élément le plus haut qu'elle produit), sinon une cible sans rendu.
 */
export function targetOfNode(index: CanvasIndex, node: string): Target {
  for (const element of index.values()) {
    if (element.node === node) return { node, key: element.key };
  }
  for (const element of index.values()) {
    if (element.instance !== node) continue;
    const parent =
      element.parent === undefined ? undefined : index.get(element.parent);
    if (!parent || parent.instance !== node) return { node, key: element.key };
  }
  return { node, key: `${DETACHED}${node}` };
}

/** Ancêtres d'un nœud, du parent à la racine. */
export function ancestors(doc: Document, node: string): string[] {
  const chain: string[] = [];
  let current = doc.nodes[node]?.parent ?? null;
  while (current !== null && !chain.includes(current)) {
    chain.push(current);
    current = doc.nodes[current]?.parent ?? null;
  }
  return chain;
}

/** Vrai si le nœud ou l'un de ses ancêtres est verrouillé. */
export function isLocked(doc: Document, node: string): boolean {
  return [node, ...ancestors(doc, node)].some(
    (id) => doc.nodes[id]?.meta.locked === true,
  );
}

/** Types dont le mode Contenu édite un champ (texte, image, lien, bouton à libellé). */
export function isContentNode(node: Node): boolean {
  const kind = node.kind;
  return (
    kind.type === "Text" ||
    kind.type === "Image" ||
    kind.type === "Link" ||
    (kind.type === "Button" && kind.label !== undefined)
  );
}
