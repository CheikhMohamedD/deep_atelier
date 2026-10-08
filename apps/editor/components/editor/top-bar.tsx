"use client";

import {
  ArrowLeft,
  Eye,
  Maximize,
  MousePointer2,
  Redo2,
  Undo2,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import Link from "next/link";
import { useFormatter, useTranslations } from "next-intl";

import { Button, buttonVariants } from "@/components/ui/button";
import type { EditorSession } from "@/lib/editor/session";
import { ALL_WIDTHS, useEditor } from "@/lib/editor/store";
import { cn } from "@/lib/utils";

export function TopBar({ session }: { session: EditorSession }) {
  const t = useTranslations("editor");
  const format = useFormatter();
  const projectName = useEditor((state) => state.projectName);
  const pages = useEditor((state) => state.pages);
  const pageId = useEditor((state) => state.pageId);
  const canUndo = useEditor((state) => state.canUndo);
  const canRedo = useEditor((state) => state.canRedo);
  const widths = useEditor((state) => state.widths);
  const mode = useEditor((state) => state.mode);
  const zoom = useEditor((state) => state.view.zoom);
  const save = useEditor((state) => state.save);
  const store = useEditor.getState;

  return (
    <header className="flex flex-wrap items-center gap-x-3 gap-y-1.5 border-b border-border bg-background px-2 py-1.5">
      <Link
        href="/projects"
        aria-label={t("projects")}
        className={cn(buttonVariants({ variant: "ghost", size: "sm" }))}
      >
        <ArrowLeft aria-hidden="true" />
        <span className="hidden md:inline">{t("projects")}</span>
      </Link>
      <h1 className="min-w-0 max-w-40 truncate text-sm font-semibold md:max-w-64">
        {projectName}
      </h1>
      {pages.length > 1 ? (
        <label className="text-sm">
          <span className="sr-only">{t("page")}</span>
          <select
            className="h-9 max-w-40 rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            value={pageId ?? ""}
            onChange={(event) => session.setPage(event.target.value)}
          >
            {pages.map((page) => (
              <option key={page.id} value={page.id}>
                {page.name}
              </option>
            ))}
          </select>
        </label>
      ) : null}

      <div className="flex items-center">
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("undo")}
          title={t("undo")}
          disabled={!canUndo}
          onClick={() => session.undo()}
        >
          <Undo2 aria-hidden="true" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("redo")}
          title={t("redo")}
          disabled={!canRedo}
          onClick={() => session.redo()}
        >
          <Redo2 aria-hidden="true" />
        </Button>
      </div>

      <div
        role="group"
        aria-label={t("widths")}
        className="flex max-w-full items-center gap-1 overflow-x-auto"
      >
        {ALL_WIDTHS.map((width) => {
          const shown = widths.includes(width);
          return (
            <Button
              key={width}
              size="sm"
              variant={shown ? "default" : "outline"}
              aria-pressed={shown}
              className="h-8 px-2 tabular-nums"
              onClick={() => store().toggleWidth(width)}
            >
              {width}
            </Button>
          );
        })}
      </div>

      <div className="flex items-center">
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("zoomOut")}
          title={t("zoomOut")}
          onClick={() => store().zoomCenter(1 / 1.25)}
        >
          <ZoomOut aria-hidden="true" />
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="w-16 px-1 tabular-nums"
          aria-label={t("zoomReset")}
          title={t("zoomReset")}
          onClick={() => store().zoomTo(1)}
        >
          {format.number(zoom, { style: "percent", maximumFractionDigits: 0 })}
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("zoomIn")}
          title={t("zoomIn")}
          onClick={() => store().zoomCenter(1.25)}
        >
          <ZoomIn aria-hidden="true" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("zoomFit")}
          title={t("zoomFit")}
          onClick={() => store().fit()}
        >
          <Maximize aria-hidden="true" />
        </Button>
      </div>

      <div
        role="group"
        aria-label={t("mode")}
        className="flex items-center gap-0.5 rounded-md border border-border p-0.5"
      >
        <Button
          size="sm"
          variant={mode === "design" ? "default" : "ghost"}
          aria-pressed={mode === "design"}
          className="h-8"
          onClick={() => store().setMode("design")}
        >
          <MousePointer2 aria-hidden="true" />
          {t("design")}
        </Button>
        <Button
          size="sm"
          variant={mode === "preview" ? "default" : "ghost"}
          aria-pressed={mode === "preview"}
          className="h-8"
          onClick={() => store().setMode("preview")}
        >
          <Eye aria-hidden="true" />
          {t("preview")}
        </Button>
      </div>

      <p
        role="status"
        data-save={save}
        className={cn(
          "ml-auto text-xs",
          save === "error" || save === "conflict"
            ? "text-destructive"
            : "text-muted-foreground",
        )}
      >
        {t(`save.${save}`)}
      </p>
    </header>
  );
}
