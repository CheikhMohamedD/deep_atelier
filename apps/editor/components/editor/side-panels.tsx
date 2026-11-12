"use client";

// Panneaux latéraux. À partir de 1024 px, ce sont deux colonnes autour du canvas ; en dessous,
// des feuilles ouvertes à la demande par-dessus le bas du canvas (une seule à la fois).

import { X } from "lucide-react";
import { useTranslations } from "next-intl";
import type { ReactNode } from "react";

import type { EditorSession } from "@/lib/editor/session";
import { useEditor, type LeftTab, type Sheet } from "@/lib/editor/store";
import { cn } from "@/lib/utils";

import { Inspector } from "./inspector";
import { LayersPanel } from "./layers-panel";
import { PagesPanel } from "./pages-panel";
import { TokensPanel } from "./tokens-panel";

const TABS: LeftTab[] = ["layers", "pages", "tokens"];

function Panel({
  sheet,
  label,
  side,
  header,
  children,
}: {
  sheet: Exclude<Sheet, null>;
  label: string;
  side: "left" | "right";
  header?: ReactNode;
  children: ReactNode;
}) {
  const t = useTranslations("editor");
  const open = useEditor((state) => state.sheet === sheet);
  return (
    <aside
      aria-label={label}
      data-panel={sheet}
      data-open={open}
      className={cn(
        "min-h-0 flex-col bg-background lg:static lg:z-auto lg:flex lg:h-auto lg:max-h-none lg:w-72 lg:shrink-0 lg:rounded-none lg:border-t-0 lg:shadow-none",
        side === "left" ? "lg:border-r" : "lg:border-l",
        "border-border",
        open
          ? "absolute inset-x-0 bottom-0 z-20 flex max-h-[62%] rounded-t-xl border-t shadow-[0_-8px_24px_rgb(0_0_0/0.12)]"
          : "hidden",
      )}
    >
      <div className="flex min-h-10 shrink-0 items-center gap-1 border-b border-border pl-1 pr-1">
        <div className="flex min-w-0 flex-1 items-center">
          {header ?? <h2 className="px-2 text-sm font-semibold">{label}</h2>}
        </div>
        <button
          type="button"
          className="flex size-9 shrink-0 items-center justify-center rounded-md outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring lg:hidden"
          aria-label={t("panels.close", { name: label })}
          onClick={() => useEditor.getState().setSheet(null)}
        >
          <X aria-hidden="true" className="size-4" />
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain">
        {children}
      </div>
    </aside>
  );
}

/** Calques, pages et tokens (mode Design). */
export function LeftPanel({ session }: { session: EditorSession }) {
  const t = useTranslations("editor");
  const tab = useEditor((state) => state.leftTab);
  return (
    <Panel
      sheet="left"
      side="left"
      label={t("panels.left")}
      header={
        <div role="tablist" aria-label={t("panels.left")} className="flex">
          {TABS.map((candidate) => (
            <button
              key={candidate}
              type="button"
              role="tab"
              id={`tab-${candidate}`}
              aria-selected={tab === candidate}
              aria-controls="left-panel"
              className={cn(
                "min-h-9 rounded-md px-2.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring",
                tab === candidate
                  ? "font-semibold"
                  : "text-muted-foreground hover:text-foreground",
              )}
              onClick={() => useEditor.getState().setLeftTab(candidate)}
            >
              {t(`panels.${candidate}`)}
            </button>
          ))}
        </div>
      }
    >
      <div id="left-panel" role="tabpanel" aria-labelledby={`tab-${tab}`}>
        {tab === "layers" ? <LayersPanel session={session} /> : null}
        {tab === "pages" ? <PagesPanel session={session} /> : null}
        {tab === "tokens" ? <TokensPanel session={session} /> : null}
      </div>
    </Panel>
  );
}

export function RightPanel({ session }: { session: EditorSession }) {
  const t = useTranslations("editor");
  const content = useEditor((state) => state.mode === "content");
  return (
    <Panel
      sheet="inspector"
      side="right"
      label={t(content ? "panels.content" : "panels.inspector")}
    >
      <Inspector session={session} />
    </Panel>
  );
}
