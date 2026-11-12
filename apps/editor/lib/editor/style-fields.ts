// Champs de style de l'inspecteur : groupes, propriétés de l'IR et valeurs proposées. Les valeurs
// sont celles de l'IR (crans Tailwind), affichées telles quelles : l'inspecteur montre à côté la
// classe qu'elles produisent.

import type { StyleProp } from "@deep-atelier/ir-types";

/** Crans d'espacement de l'IR (`Space`). */
export const SPACES = [
  "0",
  "px",
  "0.5",
  "1",
  "1.5",
  "2",
  "2.5",
  "3",
  "3.5",
  "4",
  "5",
  "6",
  "7",
  "8",
  "9",
  "10",
  "11",
  "12",
  "14",
  "16",
  "20",
  "24",
  "28",
  "32",
  "36",
  "40",
  "44",
  "48",
  "52",
  "56",
  "60",
  "64",
  "72",
  "80",
  "96",
] as const;

const FRACTIONS = ["1/2", "1/3", "2/3", "1/4", "3/4"];
const CONTAINERS = [
  "3xs",
  "2xs",
  "xs",
  "sm",
  "md",
  "lg",
  "xl",
  "2xl",
  "3xl",
  "4xl",
  "5xl",
  "6xl",
  "7xl",
].map((name) => `container.${name}`);

/** Dimensions proposées ; une valeur en pixels (`320px`) se saisit aussi. */
const SIZES = [
  "auto",
  "full",
  "screen",
  "fit",
  "min",
  "max",
  ...FRACTIONS,
  ...CONTAINERS,
  "prose",
  ...SPACES,
];

const GRID_COLUMNS = Array.from({ length: 12 }, (_, i) => String(i + 1));

const OPACITIES = Array.from({ length: 21 }, (_, i) => String(i * 5));

/** Couleurs hors tokens que l'IR accepte par mot-clé. */
export const COLOR_KEYWORDS = ["white", "black", "transparent", "current"];

export const GRADIENT_DIRECTIONS = [
  "to-t",
  "to-tr",
  "to-r",
  "to-br",
  "to-b",
  "to-bl",
  "to-l",
  "to-tl",
] as const;

/**
 * Contrôle d'un champ. `options` : liste fermée. `combo` : liste de suggestions et saisie libre
 * (le moteur valide la valeur). `color` et `background` : tokens de couleur du projet. `font` :
 * tokens de police.
 */
export type FieldControl =
  | { kind: "options"; options: readonly string[] }
  | { kind: "combo"; options: readonly string[] }
  | { kind: "bool" }
  | { kind: "color" }
  | { kind: "background" }
  | { kind: "font" };

export type StyleField = { prop: StyleProp; control: FieldControl };

export type StyleGroup = {
  id:
    | "layout"
    | "child"
    | "spacing"
    | "size"
    | "position"
    | "typography"
    | "fill"
    | "border"
    | "effects";
  fields: StyleField[];
};

const options = (...values: string[]): FieldControl => ({
  kind: "options",
  options: values,
});
const spaces: FieldControl = { kind: "options", options: SPACES };
const margins: FieldControl = { kind: "options", options: [...SPACES, "auto"] };
const insets: FieldControl = {
  kind: "options",
  options: ["auto", "full", ...SPACES],
};
const size: FieldControl = { kind: "combo", options: SIZES };
const maxSize: FieldControl = { kind: "combo", options: ["none", ...SIZES] };
const borderWidth = options("0", "1", "2", "4", "8");

export const STYLE_GROUPS: StyleGroup[] = [
  {
    id: "layout",
    fields: [
      {
        prop: "direction",
        control: options("row", "column", "row-reverse", "column-reverse"),
      },
      { prop: "wrap", control: { kind: "bool" } },
      { prop: "columns", control: options(...GRID_COLUMNS) },
      { prop: "gap", control: spaces },
      {
        prop: "align",
        control: options("start", "center", "end", "stretch", "baseline"),
      },
      {
        prop: "justify",
        control: options(
          "start",
          "center",
          "end",
          "between",
          "around",
          "evenly",
        ),
      },
    ],
  },
  {
    id: "child",
    fields: [
      { prop: "grow", control: { kind: "bool" } },
      { prop: "shrink", control: { kind: "bool" } },
      {
        prop: "align_self",
        control: options(
          "auto",
          "start",
          "center",
          "end",
          "stretch",
          "baseline",
        ),
      },
      { prop: "col_span", control: options(...GRID_COLUMNS, "full") },
      { prop: "order", control: options("default", "first", "last") },
    ],
  },
  {
    id: "spacing",
    fields: [
      { prop: "padding_top", control: spaces },
      { prop: "padding_right", control: spaces },
      { prop: "padding_bottom", control: spaces },
      { prop: "padding_left", control: spaces },
      { prop: "margin_top", control: margins },
      { prop: "margin_right", control: margins },
      { prop: "margin_bottom", control: margins },
      { prop: "margin_left", control: margins },
    ],
  },
  {
    id: "size",
    fields: [
      { prop: "width", control: size },
      { prop: "min_width", control: size },
      { prop: "max_width", control: maxSize },
      { prop: "height", control: size },
      { prop: "min_height", control: size },
      { prop: "max_height", control: maxSize },
      {
        prop: "aspect_ratio",
        control: options(
          "auto",
          "square",
          "video",
          "portrait",
          "landscape",
          "wide",
        ),
      },
      {
        prop: "overflow",
        control: options("visible", "hidden", "clip", "auto", "scroll"),
      },
      {
        prop: "object_fit",
        control: options("cover", "contain", "fill", "none", "scale-down"),
      },
    ],
  },
  {
    id: "position",
    fields: [
      {
        prop: "position",
        control: options("static", "relative", "absolute", "fixed", "sticky"),
      },
      { prop: "top", control: insets },
      { prop: "right", control: insets },
      { prop: "bottom", control: insets },
      { prop: "left", control: insets },
      {
        prop: "z_index",
        control: options("auto", "0", "10", "20", "30", "40", "50"),
      },
    ],
  },
  {
    id: "typography",
    fields: [
      { prop: "font_family", control: { kind: "font" } },
      {
        prop: "font_size",
        control: options(
          "xs",
          "sm",
          "base",
          "lg",
          "xl",
          "2xl",
          "3xl",
          "4xl",
          "5xl",
          "6xl",
          "7xl",
          "8xl",
          "9xl",
        ),
      },
      {
        prop: "font_weight",
        control: options(
          "thin",
          "extralight",
          "light",
          "normal",
          "medium",
          "semibold",
          "bold",
          "extrabold",
          "black",
        ),
      },
      {
        prop: "line_height",
        control: options("none", "tight", "snug", "normal", "relaxed", "loose"),
      },
      {
        prop: "letter_spacing",
        control: options(
          "tighter",
          "tight",
          "normal",
          "wide",
          "wider",
          "widest",
        ),
      },
      {
        prop: "text_align",
        control: options("start", "center", "end", "justify"),
      },
      { prop: "text_color", control: { kind: "color" } },
      {
        prop: "text_transform",
        control: options("none", "uppercase", "lowercase", "capitalize"),
      },
      {
        prop: "text_decoration",
        control: options("none", "underline", "line-through"),
      },
      {
        prop: "text_wrap",
        control: options("wrap", "nowrap", "balance", "pretty"),
      },
    ],
  },
  {
    id: "fill",
    fields: [{ prop: "background", control: { kind: "background" } }],
  },
  {
    id: "border",
    fields: [
      { prop: "border_top_width", control: borderWidth },
      { prop: "border_right_width", control: borderWidth },
      { prop: "border_bottom_width", control: borderWidth },
      { prop: "border_left_width", control: borderWidth },
      { prop: "border_color", control: { kind: "color" } },
      {
        prop: "border_style",
        control: options("solid", "dashed", "dotted", "none"),
      },
      {
        prop: "radius",
        control: options("none", "sm", "md", "lg", "xl", "2xl", "3xl", "full"),
      },
    ],
  },
  {
    id: "effects",
    fields: [
      {
        prop: "shadow",
        control: options("none", "xs", "sm", "md", "lg", "xl", "2xl"),
      },
      { prop: "opacity", control: options(...OPACITIES) },
      {
        prop: "transition",
        control: options(
          "none",
          "colors",
          "opacity",
          "shadow",
          "transform",
          "all",
        ),
      },
      {
        prop: "duration",
        control: options(
          "75",
          "100",
          "150",
          "200",
          "300",
          "500",
          "700",
          "1000",
        ),
      },
    ],
  },
];

/** Propriétés propres aux états d'interaction : hors de l'inspecteur de l'étape (d). */
export const STATE_ONLY_PROPS: readonly StyleProp[] = ["scale", "ring"];

/** Toutes les propriétés qu'un groupe de l'inspecteur édite. */
export const FIELD_PROPS: StyleProp[] = STYLE_GROUPS.flatMap((group) =>
  group.fields.map((field) => field.prop),
);
