"use client";

import Link from "next/link";
import { useTranslations } from "next-intl";
import { useEffect, useState, type ReactNode } from "react";

import { Button, buttonVariants } from "@/components/ui/button";
import { EditorSession } from "@/lib/editor/session";
import { useEditor } from "@/lib/editor/store";
import { useErrorText } from "@/lib/use-error-text";

import { StatusBar } from "./status-bar";
import { TopBar } from "./top-bar";
import { Viewport } from "./viewport";

/** Raccourcis : annuler, rétablir, supprimer la sélection, la vider. */
function useShortcuts(session: EditorSession) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target &&
        (target.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
      ) {
        return;
      }
      const key = event.key.toLowerCase();
      const command = event.metaKey || event.ctrlKey;
      if (command && key === "z") {
        event.preventDefault();
        if (event.shiftKey) session.redo();
        else session.undo();
      } else if (command && key === "y") {
        event.preventDefault();
        session.redo();
      } else if (
        (event.key === "Delete" || event.key === "Backspace") &&
        useEditor.getState().selection.length > 0
      ) {
        event.preventDefault();
        session.deleteSelection();
      } else if (event.key === "Escape") {
        useEditor.getState().clearSelection();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [session]);
}

/** Avertit avant de quitter une page dont des changements ne sont pas enregistrés. */
function useUnsavedWarning(session: EditorSession) {
  useEffect(() => {
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      if (session.unsaved) event.preventDefault();
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, [session]);
}

function Centered({ children }: { children: ReactNode }) {
  return (
    <main className="flex min-h-dvh flex-col items-center justify-center gap-4 px-4 text-center">
      {children}
    </main>
  );
}

export function EditorView({ projectId }: { projectId: string }) {
  const t = useTranslations("editor");
  const errorText = useErrorText();
  const [session] = useState(() => new EditorSession(projectId));
  const phase = useEditor((state) => state.phase);
  const failure = useEditor((state) => state.failure);
  const save = useEditor((state) => state.save);

  useEffect(() => {
    void session.open();
    return () => session.dispose();
  }, [session]);
  useShortcuts(session);
  useUnsavedWarning(session);

  if (phase === "loading") {
    return (
      <Centered>
        <p role="status" className="text-sm text-muted-foreground">
          {t("loading")}
        </p>
      </Centered>
    );
  }
  if (phase === "failed") {
    return (
      <Centered>
        <p role="alert" className="text-sm">
          {t("loadFailed")} {errorText(failure ?? "INTERNAL")}
        </p>
        <Link
          href="/projects"
          className={buttonVariants({ variant: "outline" })}
        >
          {t("projects")}
        </Link>
      </Centered>
    );
  }
  return (
    <div className="flex h-dvh flex-col">
      <TopBar session={session} />
      {save === "conflict" ? (
        <div
          role="alert"
          className="flex flex-wrap items-center gap-3 border-b border-border bg-muted px-3 py-2 text-sm"
        >
          <span>{t("conflict")}</span>
          <Button size="sm" onClick={() => void session.open()}>
            {t("reload")}
          </Button>
        </div>
      ) : null}
      <Viewport session={session} />
      <StatusBar />
    </div>
  );
}
