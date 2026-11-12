"use client";

// Design tokens : couleurs (clair et sombre), polices (catalogue embarqué), unité d'espacement,
// rayons et ombres. Chaque modification est une commande `set_token` ; le canvas se redessine
// avec le nouveau thème.

import type {
  ColorToken,
  Document,
  FontFamily,
  FontToken,
  ShadowLayer,
  ShadowToken,
  SystemFont,
  TokenEdit,
} from "@deep-atelier/ir-types";
import {
  FONT_CATALOG,
  FONT_CATEGORIES,
  findFont,
  googleFont,
} from "@deep-atelier/tokens";
import { Plus, Trash2 } from "lucide-react";
import { useTranslations } from "next-intl";
import { useRef, useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import type { EditorSession } from "@/lib/editor/session";
import { useEditor } from "@/lib/editor/store";

import {
  Checkbox,
  ColorSwatch,
  CommitInput,
  CommitNumber,
  Field,
  Section,
  Select,
} from "./controls";

const NEW_FIELD =
  "h-8 w-full min-w-0 rounded-md border border-border bg-background px-2 text-base outline-none focus-visible:ring-2 focus-visible:ring-ring sm:text-sm";

function setToken(session: EditorSession, label: string, edit: TokenEdit) {
  return session.edit(label, [{ op: "set_token", edit }]);
}

/** Couleur `#rrggbb` ou `#rrggbbaa` saisie à la main. */
function isHex(value: string): boolean {
  return /^#([0-9a-f]{6}|[0-9a-f]{8})$/i.test(value);
}

export function TokensPanel({ session }: { session: EditorSession }) {
  const t = useTranslations("editor.tokens");
  const doc = useEditor((state) => state.doc);
  if (!doc) return null;
  const tokens = doc.tokens;
  return (
    <div data-testid="tokens-panel">
      <Section title={t("colors")}>
        <ul className="flex flex-col gap-2">
          {tokens.colors.map((color) => (
            <ColorRow key={color.name} session={session} color={color} />
          ))}
        </ul>
        <NewToken
          label={t("newColor")}
          onCreate={(name) =>
            setToken(session, "Add color token", {
              kind: "Color",
              name,
              value: { light: "#3b82f6" },
            })
          }
        />
        <p className="text-xs text-muted-foreground">{t("nameHint")}</p>
      </Section>
      <Section title={t("fonts")}>
        <ul className="flex flex-col gap-3">
          {tokens.fonts.map((font) => (
            <FontRow key={font.name} session={session} font={font} />
          ))}
        </ul>
        <NewToken
          label={t("newFont")}
          onCreate={(name) =>
            setToken(session, "Add font token", {
              kind: "Font",
              name,
              value: { kind: "System", stack: "sans" },
            })
          }
        />
      </Section>
      <Section title={t("spacing")} defaultOpen={false}>
        <Field label={t("spacingUnit")}>
          <CommitNumber
            min={1}
            max={16}
            value={tokens.spacing_unit}
            onCommit={(px) =>
              setToken(session, "Change spacing unit", {
                kind: "SpacingUnit",
                px,
              })
            }
          />
        </Field>
        <p className="text-xs text-muted-foreground">
          {t("spacingHint", { px: tokens.spacing_unit * 4 })}
        </p>
      </Section>
      <Section title={t("radii")} defaultOpen={false}>
        <div className="grid grid-cols-2 gap-2">
          {tokens.radii.map((radius) => (
            <Field
              key={radius.step}
              label={t("radiusStep", { step: radius.step })}
            >
              <CommitNumber
                min={0}
                max={9999}
                value={radius.px}
                onCommit={(px) =>
                  setToken(session, "Change radius", {
                    kind: "Radius",
                    step: radius.step,
                    px,
                  })
                }
              />
            </Field>
          ))}
        </div>
      </Section>
      <Section title={t("shadows")} defaultOpen={false}>
        {tokens.shadows.map((shadow) => (
          <ShadowRow key={shadow.step} session={session} shadow={shadow} />
        ))}
      </Section>
      <ThemeNote doc={doc} />
    </div>
  );
}

function ThemeNote({ doc }: { doc: Document }) {
  const t = useTranslations("editor.tokens");
  const dark = doc.tokens.colors.filter((color) => color.dark !== undefined);
  return (
    <p className="px-3 py-3 text-xs text-muted-foreground">
      {t("darkNote", { count: dark.length, total: doc.tokens.colors.length })}
    </p>
  );
}

/** Nom d'un nouveau token : kebab-case, vérifié par le moteur. */
function NewToken({
  label,
  onCreate,
}: {
  label: string;
  onCreate: (name: string) => boolean;
}) {
  const t = useTranslations("editor.tokens");
  const [name, setName] = useState("");
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const trimmed = name.trim();
    if (trimmed !== "" && onCreate(trimmed)) setName("");
  };
  return (
    <form className="flex items-end gap-2" onSubmit={submit}>
      <Field label={label} className="flex-1">
        <input
          className={NEW_FIELD}
          spellCheck={false}
          autoCapitalize="none"
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

/** Champ d'une couleur : pastille (sélecteur natif, un seul undo par choix) et valeur hexadécimale. */
function HexField({
  session,
  label,
  value,
  onChange,
}: {
  session: EditorSession;
  label: string;
  value: string;
  onChange: (hex: string) => void;
}) {
  const t = useTranslations("editor.tokens");
  const picking = useRef(false);
  return (
    <div className="flex min-w-0 items-center gap-1">
      <ColorSwatch
        label={t("picker", { label })}
        value={value}
        onPick={(hex) => {
          if (!picking.current) {
            picking.current = true;
            session.beginGesture("Pick color");
          }
          onChange(hex);
        }}
        onDone={() => {
          if (picking.current) {
            picking.current = false;
            session.endGesture();
          }
        }}
      />
      <CommitInput
        aria-label={label}
        spellCheck={false}
        className="font-mono"
        value={value}
        onCommit={(text) => {
          const hex = text.trim().toLowerCase();
          if (isHex(hex)) onChange(hex);
        }}
      />
    </div>
  );
}

function ColorRow({
  session,
  color,
}: {
  session: EditorSession;
  color: ColorToken;
}) {
  const t = useTranslations("editor.tokens");
  const set = (value: { light: string; dark?: string }) =>
    setToken(session, "Change color token", {
      kind: "Color",
      name: color.name,
      value,
    });
  const current =
    color.dark === undefined
      ? { light: color.light }
      : { light: color.light, dark: color.dark };
  return (
    <li data-token={color.name} className="flex flex-col gap-1">
      <div className="flex items-center justify-between gap-2">
        <code className="truncate text-xs">{color.name}</code>
        <button
          type="button"
          className="flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
          aria-label={t("delete", { name: color.name })}
          title={t("delete", { name: color.name })}
          onClick={() =>
            setToken(session, "Delete color token", {
              kind: "Color",
              name: color.name,
              value: null,
            })
          }
        >
          <Trash2 aria-hidden="true" className="size-3.5" />
        </button>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <HexField
          session={session}
          label={t("light", { name: color.name })}
          value={color.light}
          onChange={(light) => set({ ...current, light })}
        />
        {color.dark === undefined ? (
          <Button
            size="sm"
            variant="outline"
            className="h-8 px-2 text-xs"
            onClick={() => set({ light: color.light, dark: color.light })}
          >
            {t("addDark")}
          </Button>
        ) : (
          <HexField
            session={session}
            label={t("dark", { name: color.name })}
            value={color.dark}
            onChange={(dark) => set({ light: color.light, dark })}
          />
        )}
      </div>
    </li>
  );
}

const SYSTEM_STACKS: SystemFont[] = ["sans", "serif", "mono"];
const SYSTEM_PREFIX = "system:";

function FontRow({
  session,
  font,
}: {
  session: EditorSession;
  font: FontToken;
}) {
  const t = useTranslations("editor.tokens");
  const family = font.family;
  const set = (value: FontFamily) =>
    setToken(session, "Change font token", {
      kind: "Font",
      name: font.name,
      value,
    });
  const catalog =
    family.kind === "Google" ? findFont(family.family) : undefined;
  const value =
    family.kind === "System"
      ? `${SYSTEM_PREFIX}${family.stack}`
      : family.family;
  return (
    <li data-token={font.name} className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between gap-2">
        <code className="truncate text-xs">{font.name}</code>
        <button
          type="button"
          className="flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
          aria-label={t("delete", { name: font.name })}
          title={t("delete", { name: font.name })}
          onClick={() =>
            setToken(session, "Delete font token", {
              kind: "Font",
              name: font.name,
              value: null,
            })
          }
        >
          <Trash2 aria-hidden="true" className="size-3.5" />
        </button>
      </div>
      <Select
        aria-label={t("family", { name: font.name })}
        value={value}
        onChange={(event) => {
          const next = event.target.value;
          if (next.startsWith(SYSTEM_PREFIX)) {
            set({
              kind: "System",
              stack: next.slice(SYSTEM_PREFIX.length) as SystemFont,
            });
          } else {
            const picked = findFont(next);
            if (picked) set(googleFont(picked));
          }
        }}
      >
        <optgroup label={t("system")}>
          {SYSTEM_STACKS.map((stack) => (
            <option key={stack} value={`${SYSTEM_PREFIX}${stack}`}>
              {t(`stacks.${stack}`)}
            </option>
          ))}
        </optgroup>
        {/* Une famille hors catalogue (document importé) reste affichée telle quelle. */}
        {family.kind === "Google" && !catalog ? (
          <option value={family.family}>{family.family}</option>
        ) : null}
        {FONT_CATEGORIES.map((category) => (
          <optgroup key={category} label={t(`categories.${category}`)}>
            {FONT_CATALOG.filter((entry) => entry.category === category).map(
              (entry) => (
                <option key={entry.family} value={entry.family}>
                  {entry.family}
                </option>
              ),
            )}
          </optgroup>
        ))}
      </Select>
      {family.kind === "Google" && catalog ? (
        <fieldset className="flex flex-wrap gap-x-3">
          <legend className="text-xs text-muted-foreground">
            {t("weights")}
          </legend>
          {catalog.weights.map((weight) => {
            const on = family.weights.includes(weight);
            return (
              <Checkbox
                key={weight}
                label={String(weight)}
                checked={on}
                // Au moins une graisse reste chargée.
                disabled={on && family.weights.length === 1}
                onChange={(checked) =>
                  set(
                    googleFont(
                      catalog,
                      checked
                        ? [...family.weights, weight]
                        : family.weights.filter((w) => w !== weight),
                    ),
                  )
                }
              />
            );
          })}
        </fieldset>
      ) : null}
    </li>
  );
}

const NEW_LAYER: ShadowLayer = {
  x: 0,
  y: 4,
  blur: 8,
  spread: 0,
  color: "#0000001a",
  inset: false,
};

function ShadowRow({
  session,
  shadow,
}: {
  session: EditorSession;
  shadow: ShadowToken;
}) {
  const t = useTranslations("editor.tokens");
  const set = (layers: ShadowLayer[]) =>
    setToken(session, "Change shadow", {
      kind: "Shadow",
      step: shadow.step,
      layers,
    });
  const patch = (index: number, change: Partial<ShadowLayer>) =>
    set(
      shadow.layers.map((layer, i) =>
        i === index ? { ...layer, ...change } : layer,
      ),
    );
  return (
    <fieldset
      data-token={`shadow-${shadow.step}`}
      className="flex flex-col gap-2 border-t border-border pt-2 first:border-t-0 first:pt-0"
    >
      <legend className="float-left w-full text-xs font-medium">
        {t("shadowStep", { step: shadow.step })}
      </legend>
      {shadow.layers.map((layer, index) => (
        <div key={index} className="flex flex-col gap-1.5">
          <div className="grid grid-cols-4 gap-1.5">
            {(
              [
                ["x", -500, 500],
                ["y", -500, 500],
                ["blur", 0, 500],
                ["spread", -500, 500],
              ] as const
            ).map(([key, min, max]) => (
              <Field key={key} label={t(`shadow.${key}`)}>
                <CommitNumber
                  min={min}
                  max={max}
                  value={layer[key]}
                  onCommit={(value) => patch(index, { [key]: value })}
                />
              </Field>
            ))}
          </div>
          <div className="flex items-center gap-2">
            <div className="min-w-0 flex-1">
              <HexField
                session={session}
                label={t("shadow.color")}
                value={layer.color}
                onChange={(color) => patch(index, { color })}
              />
            </div>
            <Checkbox
              label={t("shadow.inset")}
              checked={layer.inset}
              onChange={(inset) => patch(index, { inset })}
            />
            <button
              type="button"
              className="flex size-8 shrink-0 items-center justify-center rounded text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
              aria-label={t("shadow.removeLayer")}
              title={t("shadow.removeLayer")}
              onClick={() => set(shadow.layers.filter((_, i) => i !== index))}
            >
              <Trash2 aria-hidden="true" className="size-3.5" />
            </button>
          </div>
        </div>
      ))}
      <Button
        size="sm"
        variant="outline"
        className="h-8 self-start text-xs"
        onClick={() => set([...shadow.layers, NEW_LAYER])}
      >
        <Plus aria-hidden="true" />
        {t("shadow.addLayer")}
      </Button>
    </fieldset>
  );
}
