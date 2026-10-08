// Rendu React d'une page du canvas : chaque élément de l'arbre (crates/compiler-web, canvas.rs)
// devient l'élément HTML de l'export, avec les mêmes classes, et porte sa clé (`data-atl-key`)
// pour la mesure et les tests de position.

import type {
  CanvasElement,
  CanvasNode,
  CanvasPage,
  CanvasRaw,
} from "@deep-atelier/ir-types";
import { icons, type LucideIcon } from "lucide-react";
import { createElement, useLayoutEffect, type ReactNode } from "react";

/** Éléments HTML vides. */
const VOID = new Set([
  "area",
  "base",
  "br",
  "col",
  "embed",
  "hr",
  "img",
  "input",
  "link",
  "meta",
  "source",
  "track",
  "wbr",
]);

/** Attributs booléens : présents dès qu'ils figurent dans l'arbre. */
const BOOLEAN = new Set(["disabled", "required", "readonly", "multiple"]);

/** Attributs HTML dont React attend un autre nom. */
const RENAMED: Record<string, string> = {
  for: "htmlFor",
  readonly: "readOnly",
  tabindex: "tabIndex",
  datetime: "dateTime",
};

/** Composant lucide d'une icône de l'IR (`arrow-right` → `ArrowRight`). */
export function iconComponent(name: string): LucideIcon | undefined {
  const pascal = name
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
  return (icons as Record<string, LucideIcon | undefined>)[pascal];
}

function elementProps(element: CanvasElement): Record<string, unknown> {
  const props: Record<string, unknown> = {};
  for (const [name, value] of Object.entries(element.attrs)) {
    props[RENAMED[name] ?? name] = BOOLEAN.has(name) ? true : value;
  }
  if (element.class) props.className = element.class;
  if (element.key !== undefined) props["data-atl-key"] = element.key;
  if (element.toggles !== undefined)
    props["data-atl-toggles"] = element.toggles;
  return props;
}

/** Code libre : affiché tel quel ; son exécution dans le canvas viendra avec l'éditeur de code. */
function RawBlock({ raw }: { raw: CanvasRaw }) {
  return (
    <pre
      data-atl-key={raw.key}
      data-atl-raw=""
      style={{
        margin: 0,
        padding: "8px 10px",
        border: "1px dashed #a3a3a3",
        borderRadius: 6,
        background: "#fafafa",
        color: "#525252",
        font: "12px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace",
        whiteSpace: "pre-wrap",
        overflowWrap: "anywhere",
      }}
    >
      {raw.code}
    </pre>
  );
}

function renderNode(node: CanvasNode, index: number): ReactNode {
  switch (node.type) {
    case "text":
      return node.text;
    case "raw":
      return <RawBlock key={node.key} raw={node} />;
    case "element": {
      const key = node.key ?? index;
      const props = elementProps(node);
      if (node.icon !== undefined) {
        const Icon = iconComponent(node.icon);
        return Icon ? (
          <Icon key={key} {...props} />
        ) : (
          createElement("svg", { key, ...props })
        );
      }
      if (VOID.has(node.tag)) return createElement(node.tag, { key, ...props });
      return createElement(
        node.tag,
        { key, ...props },
        ...renderNodes(node.children),
      );
    }
  }
}

export function renderNodes(nodes: CanvasNode[]): ReactNode[] {
  return nodes.map((node, index) => renderNode(node, index));
}

/** Page du canvas ; `onCommit` après chaque rendu appliqué au DOM (mesure). */
export function CanvasRoot({
  page,
  onCommit,
}: {
  page: CanvasPage | null;
  onCommit?: () => void;
}) {
  useLayoutEffect(() => {
    onCommit?.();
  });
  return page ? <>{renderNodes(page.nodes)}</> : null;
}
