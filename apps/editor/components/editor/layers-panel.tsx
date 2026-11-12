"use client";

// Panneau des calques : l'arbre de l'IR de la page affichée (son layout, puis la page). La
// sélection est celle du canvas ; chaque action est une commande de l'IR.

import type { Document, Node } from "@deep-atelier/ir-types";
import {
  ArrowDown,
  ArrowUp,
  ChevronRight,
  Eye,
  EyeOff,
  Lock,
  LockOpen,
  MonitorSmartphone,
} from "lucide-react";
import { useTranslations } from "next-intl";
import { useState, type KeyboardEvent, type MouseEvent } from "react";

import type { EditorSession } from "@/lib/editor/session";
import { useEditor } from "@/lib/editor/store";
import {
  kindKey,
  layerLabel,
  layerRoots,
  targetOfNode,
  visibilityState,
} from "@/lib/editor/tree";
import { cn } from "@/lib/utils";

import { CommitInput } from "./controls";

const ICON_BUTTON =
  "flex size-7 shrink-0 items-center justify-center rounded outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-30 [&_svg]:size-3.5";

export function LayersPanel({ session }: { session: EditorSession }) {
  const t = useTranslations("editor.layers");
  const doc = useEditor((state) => state.doc);
  const pageId = useEditor((state) => state.pageId);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const [renaming, setRenaming] = useState<string | null>(null);
  if (!doc) return null;
  const roots = layerRoots(doc, pageId);
  const toggle = (id: string) =>
    setCollapsed((current) => {
      const next = new Set(current);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  return (
    <div className="flex flex-col gap-3 py-2">
      {roots.map((root) => (
        <section key={root.root} aria-label={t(root.kind, { name: root.name })}>
          <h3 className="px-3 pb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            {t(root.kind, { name: root.name })}
          </h3>
          <ul role="tree" aria-label={t(root.kind, { name: root.name })}>
            <LayerRow
              doc={doc}
              id={root.root}
              depth={0}
              isRoot
              session={session}
              collapsed={collapsed}
              onToggle={toggle}
              renaming={renaming}
              onRename={setRenaming}
            />
          </ul>
        </section>
      ))}
    </div>
  );
}

type RowProps = {
  doc: Document;
  id: string;
  depth: number;
  isRoot: boolean;
  session: EditorSession;
  collapsed: ReadonlySet<string>;
  onToggle: (id: string) => void;
  renaming: string | null;
  onRename: (id: string | null) => void;
};

function LayerRow(props: RowProps) {
  const { doc, id, depth, isRoot, session, collapsed, onToggle } = props;
  const t = useTranslations("editor.layers");
  const kinds = useTranslations("editor.kinds");
  const selected = useEditor((state) =>
    state.selection.some((target) => target.node === id),
  );
  const design = useEditor((state) => state.mode === "design");
  const node: Node | undefined = doc.nodes[id];
  if (!node) return null;
  const children = node.children ?? [];
  const open = !collapsed.has(id);
  const kind = kinds(kindKey(node));
  const label = layerLabel(doc, node) ?? kind;
  const visibility = visibilityState(node.visibility);
  const siblings = node.parent ? (doc.nodes[node.parent]?.children ?? []) : [];
  const position = siblings.indexOf(id);
  const editable = design && !isRoot;

  const select = (event: MouseEvent | KeyboardEvent) => {
    const store = useEditor.getState();
    store.select(targetOfNode(store.index, id), event.shiftKey);
  };
  const move = (delta: number) => {
    if (!node.parent) return;
    session.edit("Reorder layer", [
      {
        op: "move_node",
        node: id,
        parent: node.parent,
        index: position + delta,
      },
    ]);
  };
  const toggleVisibility = () => {
    // Une visibilité par breakpoint se règle dans l'inspecteur : le bouton sélectionne le calque.
    if (visibility === "partial") {
      const store = useEditor.getState();
      store.select(targetOfNode(store.index, id), false);
      return;
    }
    session.edit(visibility === "visible" ? "Hide layer" : "Show layer", [
      {
        op: "set_visibility",
        node: id,
        visibility: visibility === "visible" ? { base: false } : null,
      },
    ]);
  };

  return (
    <li
      role="treeitem"
      aria-selected={selected}
      aria-expanded={children.length > 0 ? open : undefined}
      aria-label={label}
      data-layer={id}
    >
      <div
        className={cn(
          "group flex min-h-8 items-center gap-0.5 pr-1 text-sm",
          selected ? "bg-selection/15" : "hover:bg-accent/60",
          visibility === "hidden" && "text-muted-foreground",
        )}
        style={{ paddingLeft: 4 + depth * 12 }}
        onMouseEnter={() => {
          const store = useEditor.getState();
          const target = targetOfNode(store.index, id);
          if (store.index.has(target.key)) store.setHover(target);
        }}
        onMouseLeave={() => useEditor.getState().setHover(null)}
      >
        <button
          type="button"
          className={cn(
            ICON_BUTTON,
            "size-6",
            children.length === 0 && "invisible",
          )}
          aria-label={t(open ? "collapse" : "expand", { name: label })}
          tabIndex={children.length === 0 ? -1 : 0}
          onClick={() => onToggle(id)}
        >
          <ChevronRight
            aria-hidden="true"
            className={cn("transition-transform", open && "rotate-90")}
          />
        </button>
        {props.renaming === id ? (
          <CommitInput
            autoFocus
            aria-label={t("name")}
            className="h-7 flex-1"
            value={node.meta.name ?? ""}
            placeholder={kind}
            onCommit={(name) =>
              session.edit("Rename layer", [
                {
                  op: "set_meta",
                  node: id,
                  meta: { name: name.trim() === "" ? null : name.trim() },
                },
              ])
            }
            onBlurCapture={() => props.onRename(null)}
            onKeyDownCapture={(event) => {
              if (event.key === "Enter" || event.key === "Escape") {
                // Le champ valide (ou annule) dans son propre gestionnaire, puis se ferme.
                queueMicrotask(() => props.onRename(null));
              }
            }}
          />
        ) : (
          <button
            type="button"
            className="flex min-w-0 flex-1 items-baseline gap-1.5 self-stretch rounded px-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
            onClick={select}
            onDoubleClick={() => {
              if (design) props.onRename(id);
            }}
            onKeyDown={(event) => {
              if (event.key === "F2" && design) {
                event.preventDefault();
                props.onRename(id);
              }
            }}
          >
            <span className="truncate">{label}</span>
            {label !== kind ? (
              <span className="shrink-0 text-xs text-muted-foreground">
                {kind}
              </span>
            ) : null}
          </button>
        )}
        {editable ? (
          <span className="flex shrink-0 items-center">
            <button
              type="button"
              className={ICON_BUTTON}
              aria-label={t("moveUp", { name: label })}
              title={t("moveUp", { name: label })}
              disabled={position <= 0}
              onClick={() => move(-1)}
            >
              <ArrowUp aria-hidden="true" />
            </button>
            <button
              type="button"
              className={ICON_BUTTON}
              aria-label={t("moveDown", { name: label })}
              title={t("moveDown", { name: label })}
              disabled={position < 0 || position >= siblings.length - 1}
              onClick={() => move(1)}
            >
              <ArrowDown aria-hidden="true" />
            </button>
            <button
              type="button"
              className={ICON_BUTTON}
              aria-pressed={node.meta.locked}
              aria-label={t(node.meta.locked ? "unlock" : "lock", {
                name: label,
              })}
              title={t(node.meta.locked ? "unlock" : "lock", { name: label })}
              onClick={() =>
                session.edit(node.meta.locked ? "Unlock layer" : "Lock layer", [
                  {
                    op: "set_meta",
                    node: id,
                    meta: { locked: !node.meta.locked },
                  },
                ])
              }
            >
              {node.meta.locked ? (
                <Lock aria-hidden="true" />
              ) : (
                <LockOpen aria-hidden="true" className="opacity-40" />
              )}
            </button>
            <button
              type="button"
              className={ICON_BUTTON}
              aria-pressed={visibility !== "visible"}
              aria-label={t(`visibility.${visibility}`, { name: label })}
              title={t(`visibility.${visibility}`, { name: label })}
              onClick={toggleVisibility}
            >
              {visibility === "visible" ? (
                <Eye aria-hidden="true" className="opacity-40" />
              ) : visibility === "hidden" ? (
                <EyeOff aria-hidden="true" />
              ) : (
                <MonitorSmartphone aria-hidden="true" />
              )}
            </button>
          </span>
        ) : null}
      </div>
      {children.length > 0 && open ? (
        <ul role="group">
          {children.map((child) => (
            <LayerRow
              key={child}
              {...props}
              id={child}
              depth={depth + 1}
              isRoot={false}
            />
          ))}
        </ul>
      ) : null}
    </li>
  );
}
