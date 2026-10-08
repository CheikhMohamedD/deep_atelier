"use client";

import { useTranslations } from "next-intl";

import { useEditor } from "@/lib/editor/store";

export function StatusBar() {
  const t = useTranslations("editor");
  const count = useEditor((state) => state.selection.length);
  const notice = useEditor((state) => state.notice);
  return (
    <footer className="flex min-h-8 flex-wrap items-center justify-between gap-2 border-t border-border bg-background px-3 py-1 text-xs text-muted-foreground">
      <span data-testid="selection-count">{t("selection", { count })}</span>
      {notice ? (
        <span role="alert" className="text-destructive">
          {t("commandFailed", { code: notice })}
        </span>
      ) : null}
    </footer>
  );
}
