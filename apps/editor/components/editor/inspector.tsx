"use client";

// Inspecteur : contenu, calque, visibilité et style du nœud sélectionné. Chaque propriété de
// style s'édite au breakpoint actif, montre d'où vient sa valeur (posée ici, héritée d'un
// breakpoint plus étroit) et la classe Tailwind qu'elle produit.

import type {
  Background,
  Breakpoint,
  Document,
  GradientDir,
  Node,
  NodeInspection,
  Responsive,
  Style,
  StylePatch,
} from "@deep-atelier/ir-types";
import { RotateCcw } from "lucide-react";
import { useTranslations } from "next-intl";
import { useMemo, type ReactNode } from "react";

import {
  BREAKPOINTS,
  declaredAt,
  overrides,
  resolveAt,
} from "@/lib/editor/breakpoints";
import type { EditorSession } from "@/lib/editor/session";
import { useEditor } from "@/lib/editor/store";
import {
  COLOR_KEYWORDS,
  GRADIENT_DIRECTIONS,
  STYLE_GROUPS,
  type StyleField,
} from "@/lib/editor/style-fields";
import { isLocked, kindKey, layerLabel } from "@/lib/editor/tree";
import { cn } from "@/lib/utils";

import { ContentFields, hasContentFields } from "./content-fields";
import {
  Checkbox,
  Combo,
  CommitInput,
  Field,
  Section,
  Select,
} from "./controls";

type StyleKey = keyof Style;

export function Inspector({ session }: { session: EditorSession }) {
  const t = useTranslations("editor.inspector");
  const kinds = useTranslations("editor.kinds");
  const doc = useEditor((state) => state.doc);
  const selection = useEditor((state) => state.selection);
  const mode = useEditor((state) => state.mode);
  const breakpoint = useEditor((state) => state.breakpoint);
  const nodeId = selection.length === 1 ? selection[0]?.node : undefined;
  const node = nodeId && doc ? doc.nodes[nodeId] : undefined;
  // Les classes et les problèmes viennent du moteur ; relus quand le document change.
  const inspection = useMemo(
    () => (nodeId && doc ? session.inspect(nodeId) : null),
    [session, nodeId, doc],
  );

  if (!doc || !node) {
    return (
      <p className="px-3 py-4 text-sm text-muted-foreground">
        {selection.length > 1
          ? t("multiple", { count: selection.length })
          : t(mode === "content" ? "emptyContent" : "empty")}
      </p>
    );
  }
  const design = mode === "design";
  const locked = design && isLocked(doc, node.id);
  const kind = kinds(kindKey(node));
  const label = layerLabel(doc, node) ?? kind;
  return (
    <div data-testid="inspector" data-node={node.id}>
      <header className="flex flex-col gap-0.5 border-b border-border px-3 py-2">
        <p className="truncate text-sm font-semibold">{label}</p>
        <p className="text-xs text-muted-foreground">
          {kind}
          {inspection?.owner
            ? ` · ${t(`owner.${inspection.owner.kind}`, { name: inspection.owner.name })}`
            : null}
        </p>
        {locked ? (
          <p role="status" className="text-xs text-warning">
            {t("locked")}
          </p>
        ) : null}
      </header>
      <fieldset disabled={locked} className="min-w-0 disabled:opacity-60">
        {hasContentFields(node, design) ? (
          <Section title={t("content")}>
            <ContentFields session={session} doc={doc} node={node} />
          </Section>
        ) : mode === "content" ? (
          <p className="px-3 py-4 text-sm text-muted-foreground">
            {t("noContent")}
          </p>
        ) : null}
        {design ? (
          <>
            <LayerSection session={session} node={node} />
            <VisibilitySection
              session={session}
              node={node}
              breakpoint={breakpoint}
            />
            {inspection ? (
              <StyleSections
                session={session}
                doc={doc}
                node={node}
                inspection={inspection}
                breakpoint={breakpoint}
              />
            ) : null}
          </>
        ) : null}
      </fieldset>
      {design && inspection && inspection.issues.length > 0 ? (
        <Section title={t("issues", { count: inspection.issues.length })}>
          <ul className="flex flex-col gap-1.5">
            {inspection.issues.map((issue, index) => (
              <li
                key={`${issue.code}-${index}`}
                data-severity={issue.severity}
                className={cn(
                  "rounded-md border border-border px-2 py-1.5 text-xs",
                  issue.severity === "Error" && "border-destructive/50",
                )}
              >
                <span className="font-medium">
                  {t(`severity.${issue.severity}`)}
                  {issue.breakpoint ? ` · ${issue.breakpoint}` : ""}
                </span>
                <span className="block text-muted-foreground">
                  {issue.message}
                </span>
                <code className="text-[11px] text-muted-foreground">
                  {issue.code}
                </code>
              </li>
            ))}
          </ul>
        </Section>
      ) : null}
    </div>
  );
}

function LayerSection({
  session,
  node,
}: {
  session: EditorSession;
  node: Node;
}) {
  const t = useTranslations("editor.inspector");
  const meta = (
    label: string,
    patch: Record<string, string | boolean | null>,
  ) => session.edit(label, [{ op: "set_meta", node: node.id, meta: patch }]);
  const text = (value: string) => (value.trim() === "" ? null : value.trim());
  return (
    <Section title={t("layer")} defaultOpen={false}>
      <Field label={t("name")}>
        <CommitInput
          value={node.meta.name ?? ""}
          onCommit={(name) => meta("Rename layer", { name: text(name) })}
        />
      </Field>
      <Field label={t("anchor")}>
        <CommitInput
          spellCheck={false}
          value={node.meta.anchor ?? ""}
          onCommit={(anchor) => meta("Set anchor", { anchor: text(anchor) })}
        />
      </Field>
      <Field label={t("a11yLabel")}>
        <CommitInput
          value={node.a11y?.label ?? ""}
          onCommit={(label) =>
            meta("Set accessible name", { a11y_label: text(label) })
          }
        />
      </Field>
      <Checkbox
        label={t("a11yHidden")}
        checked={node.a11y?.hidden ?? false}
        onChange={(hidden) =>
          meta("Set accessibility", { a11y_hidden: hidden })
        }
      />
    </Section>
  );
}

/** Pastille d'un champ : valeur posée à ce breakpoint, héritée, ou non définie. */
function Origin({
  responsive,
  breakpoint,
}: {
  responsive: Responsive<unknown> | undefined;
  breakpoint: Breakpoint;
}) {
  const t = useTranslations("editor.inspector");
  const resolved = resolveAt(responsive, breakpoint);
  if (!resolved) return null;
  if (declaredAt(responsive, breakpoint)) {
    return (
      <span
        data-origin="set"
        title={t("setAt", { breakpoint })}
        className="flex items-center gap-1 text-[11px] text-selection"
      >
        <span
          aria-hidden="true"
          className="size-1.5 rounded-full bg-selection"
        />
        <span className="sr-only">{t("setAt", { breakpoint })}</span>
        <span aria-hidden="true">{breakpoint}</span>
      </span>
    );
  }
  return (
    <span data-origin="inherited" className="text-[11px] text-muted-foreground">
      {t("inheritedFrom", { breakpoint: resolved.source })}
    </span>
  );
}

function ResetButton({
  label,
  onClick,
}: {
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className="flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
      onClick={onClick}
    >
      <RotateCcw aria-hidden="true" className="size-3.5" />
    </button>
  );
}

function VisibilitySection({
  session,
  node,
  breakpoint,
}: {
  session: EditorSession;
  node: Node;
  breakpoint: Breakpoint;
}) {
  const t = useTranslations("editor.inspector");
  const visibility = node.visibility;
  const visible = resolveAt(visibility, breakpoint)?.value ?? true;
  const declared = declaredAt(visibility, breakpoint);
  const set = (value: boolean) =>
    session.edit(value ? "Show layer" : "Hide layer", [
      {
        op: "set_visibility",
        node: node.id,
        visibility: { [breakpoint]: value },
      },
    ]);
  const reset = () =>
    session.edit("Reset visibility", [
      {
        op: "set_visibility",
        node: node.id,
        visibility:
          breakpoint === "base" || !visibility ? null : { [breakpoint]: null },
      },
    ]);
  return (
    <Section title={t("visibility")}>
      <div className="flex items-center gap-2">
        <div className="flex-1">
          <Checkbox
            label={t("visibleAt", { breakpoint })}
            checked={visible}
            onChange={set}
          />
        </div>
        <Origin responsive={visibility} breakpoint={breakpoint} />
        {declared ? (
          <ResetButton
            label={t(breakpoint === "base" ? "alwaysVisible" : "reset", {
              prop: t("visibility"),
              breakpoint,
            })}
            onClick={reset}
          />
        ) : null}
      </div>
      {visibility ? (
        <p className="text-xs text-muted-foreground">
          {BREAKPOINTS.map((candidate) => {
            const at = resolveAt(visibility, candidate)?.value ?? true;
            return `${candidate} ${at ? "✓" : "✕"}`;
          }).join("  ")}
        </p>
      ) : null}
    </Section>
  );
}

function StyleSections({
  session,
  doc,
  node,
  inspection,
  breakpoint,
}: {
  session: EditorSession;
  doc: Document;
  node: Node;
  inspection: NodeInspection;
  breakpoint: Breakpoint;
}) {
  const t = useTranslations("editor.inspector");
  const allowed = new Set<string>(inspection.allowed);
  return (
    <>
      {STYLE_GROUPS.map((group) => {
        const fields = group.fields.filter((field) => allowed.has(field.prop));
        if (fields.length === 0) return null;
        const style = node.style ?? {};
        const used = fields.some(
          (field) => style[field.prop as StyleKey] !== undefined,
        );
        return (
          <Section
            key={`${node.id}-${group.id}`}
            title={t(`groups.${group.id}`)}
            defaultOpen={used || group.id === "layout"}
          >
            {fields.map((field) => (
              <StyleRow
                key={field.prop}
                session={session}
                doc={doc}
                node={node}
                field={field}
                inspection={inspection}
                breakpoint={breakpoint}
              />
            ))}
          </Section>
        );
      })}
    </>
  );
}

function StyleRow({
  session,
  doc,
  node,
  field,
  inspection,
  breakpoint,
}: {
  session: EditorSession;
  doc: Document;
  node: Node;
  field: StyleField;
  inspection: NodeInspection;
  breakpoint: Breakpoint;
}) {
  const t = useTranslations("editor.inspector");
  const props = useTranslations("editor.props");
  const prop = field.prop as StyleKey;
  const responsive = node.style?.[prop] as Responsive<unknown> | undefined;
  const resolved = resolveAt(responsive, breakpoint);
  const declared = declaredAt(responsive, breakpoint);
  const name = props(prop);

  const patch = (value: unknown) =>
    session.edit(`Set ${prop}`, [
      {
        op: "set_style",
        node: node.id,
        style: { [prop]: { [breakpoint]: value } } as StylePatch,
      },
    ]);
  // À `base`, effacer la valeur retire la propriété (et ses surcharges) ; ailleurs, la surcharge.
  const reset = () =>
    session.edit(`Reset ${prop}`, [
      {
        op: "set_style",
        node: node.id,
        style: {
          [prop]: breakpoint === "base" ? null : { [breakpoint]: null },
        } as StylePatch,
      },
    ]);
  const classes = resolved
    ? inspection.classes
        .filter(
          (entry) =>
            entry.prop === field.prop &&
            entry.state === undefined &&
            entry.breakpoint === resolved.source,
        )
        .flatMap((entry) => entry.classes)
    : [];
  const above = overrides(responsive).filter(
    (candidate) =>
      BREAKPOINTS.indexOf(candidate) > BREAKPOINTS.indexOf(breakpoint),
  );

  return (
    <div data-prop={field.prop} className="flex flex-col gap-1">
      <div className="flex min-h-6 items-center gap-2">
        <span className="flex-1 truncate text-xs text-muted-foreground">
          {name}
        </span>
        <Origin responsive={responsive} breakpoint={breakpoint} />
        {declared ? (
          <ResetButton
            label={t(breakpoint === "base" ? "remove" : "reset", {
              prop: name,
              breakpoint,
            })}
            onClick={reset}
          />
        ) : null}
      </div>
      <StyleControl
        doc={doc}
        field={field}
        label={name}
        value={resolved?.value}
        onChange={patch}
        onClear={reset}
        clearable={declared}
      />
      {classes.length > 0 || above.length > 0 ? (
        <p className="flex flex-wrap items-center gap-1 text-[11px]">
          {classes.map((name) => (
            <code
              key={name}
              data-class={name}
              className={cn(
                "rounded bg-muted px-1 py-0.5",
                !declared && "text-muted-foreground",
              )}
            >
              {name}
            </code>
          ))}
          {above.length > 0 ? (
            <span className="text-muted-foreground">
              {t("overriddenAt", { breakpoints: above.join(", ") })}
            </span>
          ) : null}
        </p>
      ) : null}
    </div>
  );
}

function ColorCombo({
  doc,
  label,
  value,
  onCommit,
}: {
  doc: Document;
  label: string;
  value: string;
  onCommit: (value: string) => void;
}) {
  const names = doc.tokens.colors.map((token) => token.name);
  const token = doc.tokens.colors.find(
    (candidate) => candidate.name === value.split("/")[0],
  );
  return (
    <div className="flex items-center gap-1.5">
      <span
        aria-hidden="true"
        className="size-6 shrink-0 rounded border border-border"
        style={{ background: token?.light ?? "transparent" }}
      />
      <Combo
        aria-label={label}
        value={value}
        options={[...names, ...COLOR_KEYWORDS]}
        onCommit={(next) => onCommit(next.trim())}
      />
    </div>
  );
}

function StyleControl({
  doc,
  field,
  label,
  value,
  onChange,
  onClear,
  clearable,
}: {
  doc: Document;
  field: StyleField;
  label: string;
  value: unknown;
  onChange: (value: unknown) => void;
  onClear: () => void;
  clearable: boolean;
}): ReactNode {
  const t = useTranslations("editor.inspector");
  const control = field.control;
  // Champ vidé : la valeur posée à ce breakpoint est effacée ; une valeur héritée reste.
  const commitText = (text: string) => {
    if (text === "") {
      if (clearable) onClear();
    } else {
      onChange(text);
    }
  };
  switch (control.kind) {
    case "options":
    case "font": {
      const options =
        control.kind === "font"
          ? doc.tokens.fonts.map((font) => font.name)
          : control.options;
      const current = value === undefined ? "" : String(value);
      return (
        <Select
          aria-label={label}
          value={current}
          onChange={(event) => commitText(event.target.value)}
        >
          <option value="">{t("unset")}</option>
          {current !== "" && !options.includes(current) ? (
            <option value={current}>{current}</option>
          ) : null}
          {options.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </Select>
      );
    }
    case "bool":
      return (
        <Select
          aria-label={label}
          value={value === undefined ? "" : String(value)}
          onChange={(event) => {
            if (event.target.value === "") commitText("");
            else onChange(event.target.value === "true");
          }}
        >
          <option value="">{t("unset")}</option>
          <option value="true">{t("yes")}</option>
          <option value="false">{t("no")}</option>
        </Select>
      );
    case "combo":
      return (
        <Combo
          aria-label={label}
          value={value === undefined ? "" : String(value)}
          options={control.options}
          placeholder={t("unset")}
          onCommit={(text) => commitText(text.trim())}
        />
      );
    case "color":
      return (
        <ColorCombo
          doc={doc}
          label={label}
          value={typeof value === "string" ? value : ""}
          onCommit={commitText}
        />
      );
    case "background":
      return (
        <BackgroundControl
          doc={doc}
          label={label}
          value={value as Background | undefined}
          onChange={onChange}
          onClear={() => commitText("")}
        />
      );
  }
}

function BackgroundControl({
  doc,
  label,
  value,
  onChange,
  onClear,
}: {
  doc: Document;
  label: string;
  value: Background | undefined;
  onChange: (value: Background) => void;
  onClear: () => void;
}) {
  const t = useTranslations("editor.inspector");
  const first = doc.tokens.colors[0]?.name ?? "white";
  return (
    <div className="flex flex-col gap-1.5">
      <Select
        aria-label={t("backgroundKind")}
        value={value?.kind ?? ""}
        onChange={(event) => {
          const kind = event.target.value;
          if (kind === "") onClear();
          else if (kind === "Color") {
            onChange({
              kind: "Color",
              color: value?.kind === "Gradient" ? value.from : first,
            });
          } else {
            const from = value?.kind === "Color" ? value.color : first;
            onChange({ kind: "Gradient", direction: "to-b", from, to: from });
          }
        }}
      >
        <option value="">{t("unset")}</option>
        <option value="Color">{t("backgroundColor")}</option>
        <option value="Gradient">{t("backgroundGradient")}</option>
      </Select>
      {value?.kind === "Color" ? (
        <ColorCombo
          doc={doc}
          label={label}
          value={value.color}
          onCommit={(color) => {
            if (color !== "") onChange({ kind: "Color", color });
          }}
        />
      ) : null}
      {value?.kind === "Gradient" ? (
        <>
          <Select
            aria-label={t("gradientDirection")}
            value={value.direction}
            onChange={(event) =>
              onChange({
                ...value,
                direction: event.target.value as GradientDir,
              })
            }
          >
            {GRADIENT_DIRECTIONS.map((direction) => (
              <option key={direction} value={direction}>
                {direction}
              </option>
            ))}
          </Select>
          <ColorCombo
            doc={doc}
            label={t("gradientFrom")}
            value={value.from}
            onCommit={(from) => {
              if (from !== "") onChange({ ...value, from });
            }}
          />
          <ColorCombo
            doc={doc}
            label={t("gradientVia")}
            value={value.via ?? ""}
            onCommit={(via) => {
              const next: Background = {
                kind: "Gradient",
                direction: value.direction,
                from: value.from,
                to: value.to,
              };
              onChange(via === "" ? next : { ...next, via });
            }}
          />
          <ColorCombo
            doc={doc}
            label={t("gradientTo")}
            value={value.to}
            onCommit={(to) => {
              if (to !== "") onChange({ ...value, to });
            }}
          />
        </>
      ) : null}
    </div>
  );
}
