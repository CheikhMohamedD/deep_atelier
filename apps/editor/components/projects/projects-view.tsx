"use client";

import type { Project } from "@deep-atelier/ir-types";
import { Plus, Trash2 } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useFormatter, useTranslations } from "next-intl";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { api, errorCode } from "@/lib/api";
import { useErrorText } from "@/lib/use-error-text";

export function ProjectsView() {
  const t = useTranslations("projects");
  const tc = useTranslations("common");
  const errorText = useErrorText();
  const format = useFormatter();
  const router = useRouter();
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [confirming, setConfirming] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setProjects((await api.listProjects()).projects);
      setError(null);
    } catch (caught) {
      setError(errorCode(caught));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setCreating(true);
    try {
      const created = await api.createProject({ name: name.trim() });
      router.push(`/projects/${created.project.id}`);
    } catch (caught) {
      setError(errorCode(caught));
      setCreating(false);
    }
  }

  async function remove(id: string) {
    try {
      await api.deleteProject(id);
      setConfirming(null);
      await load();
    } catch (caught) {
      setError(errorCode(caught));
    }
  }

  return (
    <div className="flex flex-col gap-8">
      <h1 className="text-2xl font-semibold tracking-tight">{t("title")}</h1>

      <form
        onSubmit={create}
        className="flex flex-col gap-3 rounded-xl border border-border p-4 sm:flex-row sm:items-end"
      >
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <Label htmlFor="project-name">{t("name")}</Label>
          <Input
            id="project-name"
            required
            maxLength={120}
            placeholder={t("namePlaceholder")}
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </div>
        <Button type="submit" disabled={creating || name.trim() === ""}>
          <Plus aria-hidden="true" />
          {creating ? t("creating") : t("create")}
        </Button>
      </form>

      {error ? (
        <p role="alert" className="text-sm text-destructive">
          {errorText(error)}{" "}
          <button
            type="button"
            className="underline"
            onClick={() => void load()}
          >
            {tc("retry")}
          </button>
        </p>
      ) : null}

      {projects === null && !error ? (
        <p className="text-sm text-muted-foreground">{tc("loading")}</p>
      ) : null}
      {projects?.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t("empty")}</p>
      ) : null}

      {projects && projects.length > 0 ? (
        <ul className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {projects.map((project) => (
            <li
              key={project.id}
              className="flex flex-col gap-3 rounded-xl border border-border p-4"
            >
              <Link
                href={`/projects/${project.id}`}
                className="truncate text-base font-medium underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                {project.name}
              </Link>
              <p className="text-sm text-muted-foreground">
                {t("updated", {
                  date: format.dateTime(new Date(project.updated_at), {
                    dateStyle: "medium",
                    timeStyle: "short",
                  }),
                })}
              </p>
              {confirming === project.id ? (
                <div className="flex flex-col gap-2">
                  <p className="text-sm">
                    {t("deleteConfirm", { name: project.name })}
                  </p>
                  <div className="flex gap-2">
                    <Button
                      variant="destructive"
                      size="sm"
                      onClick={() => void remove(project.id)}
                    >
                      {tc("delete")}
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setConfirming(null)}
                    >
                      {tc("cancel")}
                    </Button>
                  </div>
                </div>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  className="self-start"
                  onClick={() => setConfirming(project.id)}
                >
                  <Trash2 aria-hidden="true" />
                  {tc("delete")}
                </Button>
              )}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
