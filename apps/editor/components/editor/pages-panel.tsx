"use client";

// Pages et routes : liste des pages, création, route (statique ou `[paramètre]`), layout partagé,
// métadonnées SEO ; layouts ; réglages du site.

import type { Command, Document, Layout, Page } from "@deep-atelier/ir-types";
import { Plus, Trash2 } from "lucide-react";
import { useTranslations } from "next-intl";
import { useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { formatRoute, parseRoute, slugOf } from "@/lib/editor/routes";
import type { EditorSession } from "@/lib/editor/session";
import { useEditor } from "@/lib/editor/store";
import { cn } from "@/lib/utils";

import {
  CommitInput,
  CommitTextarea,
  Field,
  Section,
  Select,
} from "./controls";

export function PagesPanel({ session }: { session: EditorSession }) {
  const t = useTranslations("editor.pages");
  const doc = useEditor((state) => state.doc);
  const pageId = useEditor((state) => state.pageId);
  if (!doc) return null;
  const page = doc.pages.find((candidate) => candidate.id === pageId);
  return (
    <div>
      <Section title={t("title")}>
        <ul className="flex flex-col gap-0.5">
          {doc.pages.map((candidate) => (
            <li key={candidate.id}>
              <button
                type="button"
                aria-current={candidate.id === pageId ? "page" : undefined}
                className={cn(
                  "flex min-h-9 w-full items-baseline justify-between gap-2 rounded-md px-2 text-left text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring",
                  candidate.id === pageId
                    ? "bg-selection/15 font-medium"
                    : "hover:bg-accent/60",
                )}
                onClick={() => session.setPage(candidate.id)}
              >
                <span className="truncate">{candidate.name}</span>
                <code className="shrink-0 text-xs text-muted-foreground">
                  {formatRoute(candidate.route)}
                </code>
              </button>
            </li>
          ))}
        </ul>
        <NewPage session={session} />
      </Section>
      {page ? (
        <PageSettings key={page.id} session={session} doc={doc} page={page} />
      ) : null}
      <Layouts session={session} layouts={doc.layouts} />
      <Section title={t("site")} defaultOpen={false}>
        <Field label={t("siteName")}>
          <CommitInput
            value={doc.settings.site_name}
            onCommit={(site_name) =>
              session.edit("Rename site", [
                { op: "update_settings", site_name: site_name.trim() },
              ])
            }
          />
        </Field>
        <Field label={t("siteLang")}>
          <CommitInput
            spellCheck={false}
            value={doc.settings.lang}
            onCommit={(lang) =>
              session.edit("Change site language", [
                { op: "update_settings", lang: lang.trim() },
              ])
            }
          />
        </Field>
      </Section>
    </div>
  );
}

function NewPage({ session }: { session: EditorSession }) {
  const t = useTranslations("editor.pages");
  const [name, setName] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const trimmed = name.trim();
    if (trimmed === "") return;
    const slug = slugOf(trimmed);
    const created = session.createPage({
      op: "create_page",
      name: trimmed,
      route: slug === "" ? [] : [{ kind: "Static", name: slug }],
      seo: { title: trimmed, description: "" },
    });
    if (created) setName("");
  };
  return (
    <form className="flex items-end gap-2" onSubmit={submit}>
      <Field label={t("newName")} className="flex-1">
        <input
          className="h-8 w-full min-w-0 rounded-md border border-border bg-background px-2 text-base outline-none focus-visible:ring-2 focus-visible:ring-ring sm:text-sm"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
      </Field>
      <Button
        type="submit"
        size="sm"
        variant="outline"
        className="h-8"
        disabled={name.trim() === ""}
      >
        <Plus aria-hidden="true" />
        {t("add")}
      </Button>
    </form>
  );
}

function PageSettings({
  session,
  doc,
  page,
}: {
  session: EditorSession;
  doc: Document;
  page: Page;
}) {
  const t = useTranslations("editor.pages");
  const [routeError, setRouteError] = useState(false);
  const update = (
    label: string,
    patch: Omit<Extract<Command, { op: "update_page" }>, "op" | "page">,
  ) => session.edit(label, [{ op: "update_page", page: page.id, ...patch }]);
  return (
    <Section title={t("settings", { name: page.name })}>
      <Field label={t("name")}>
        <CommitInput
          value={page.name}
          onCommit={(name) => {
            if (name.trim() !== "")
              update("Rename page", { name: name.trim() });
          }}
        />
      </Field>
      <Field label={t("route")}>
        <CommitInput
          spellCheck={false}
          aria-invalid={routeError}
          value={formatRoute(page.route)}
          onCommit={(path) => {
            const route = parseRoute(path);
            setRouteError(route === null);
            if (route) update("Change route", { route });
          }}
        />
      </Field>
      <p
        className={cn(
          "text-xs",
          routeError ? "text-destructive" : "text-muted-foreground",
        )}
      >
        {t(routeError ? "routeInvalid" : "routeHint")}
      </p>
      <Field label={t("layout")}>
        <Select
          value={page.layout ?? ""}
          onChange={(event) =>
            update("Change layout", {
              layout: event.target.value === "" ? null : event.target.value,
            })
          }
        >
          <option value="">{t("noLayout")}</option>
          {doc.layouts.map((layout) => (
            <option key={layout.id} value={layout.id}>
              {layout.name}
            </option>
          ))}
        </Select>
      </Field>
      <Field label={t("seoTitle")}>
        <CommitInput
          value={page.seo.title}
          onCommit={(title) =>
            update("Edit SEO title", { seo: { ...page.seo, title } })
          }
        />
      </Field>
      <Field label={t("seoDescription")}>
        <CommitTextarea
          value={page.seo.description}
          onCommit={(description) =>
            update("Edit SEO description", {
              seo: { ...page.seo, description },
            })
          }
        />
      </Field>
      <p className="text-xs text-muted-foreground">
        {t("seoLength", { count: page.seo.description.length })}
      </p>
      <Button
        size="sm"
        variant="outline"
        className="h-8 self-start text-destructive"
        disabled={doc.pages.length <= 1}
        onClick={() => {
          if (window.confirm(t("deleteConfirm", { name: page.name }))) {
            session.edit("Delete page", [{ op: "delete_page", page: page.id }]);
          }
        }}
      >
        <Trash2 aria-hidden="true" />
        {t("delete")}
      </Button>
    </Section>
  );
}

function Layouts({
  session,
  layouts,
}: {
  session: EditorSession;
  layouts: Layout[];
}) {
  const t = useTranslations("editor.pages");
  const [name, setName] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const trimmed = name.trim();
    if (trimmed === "") return;
    if (session.edit("Create layout", [{ op: "create_layout", name: trimmed }]))
      setName("");
  };
  return (
    <Section title={t("layouts")} defaultOpen={layouts.length > 0}>
      {layouts.length === 0 ? (
        <p className="text-xs text-muted-foreground">{t("layoutsEmpty")}</p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {layouts.map((layout) => (
            <li key={layout.id} className="flex items-center gap-1.5">
              <CommitInput
                aria-label={t("layoutName")}
                value={layout.name}
                onCommit={(next) => {
                  if (next.trim() !== "") {
                    session.edit("Rename layout", [
                      {
                        op: "update_layout",
                        layout: layout.id,
                        name: next.trim(),
                      },
                    ]);
                  }
                }}
              />
              <Button
                size="icon-sm"
                variant="ghost"
                className="size-8"
                aria-label={t("layoutDelete", { name: layout.name })}
                title={t("layoutDelete", { name: layout.name })}
                onClick={() =>
                  session.edit("Delete layout", [
                    { op: "delete_layout", layout: layout.id },
                  ])
                }
              >
                <Trash2 aria-hidden="true" />
              </Button>
            </li>
          ))}
        </ul>
      )}
      <form className="flex items-end gap-2" onSubmit={submit}>
        <Field label={t("layoutNew")} className="flex-1">
          <input
            className="h-8 w-full min-w-0 rounded-md border border-border bg-background px-2 text-base outline-none focus-visible:ring-2 focus-visible:ring-ring sm:text-sm"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </Field>
        <Button
          type="submit"
          size="sm"
          variant="outline"
          className="h-8"
          disabled={name.trim() === ""}
        >
          <Plus aria-hidden="true" />
          {t("add")}
        </Button>
      </form>
      <p className="text-xs text-muted-foreground">{t("layoutsHint")}</p>
    </Section>
  );
}
