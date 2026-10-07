# ADR 0001 — Architecture de la Phase 1 (MVP web)

- **Statut** : accepté le 2026-10-06 ; questions ouvertes posées étape par étape (§ 12)
- **Date** : 2026-10-06
- **Référence** : [`docs/SPEC.md`](../SPEC.md) (prompt maître et spécifications)

Ce document consolide la proposition d'architecture faite à l'étape 1 de la méthode de travail
(arborescence, schéma de l'IR, commandes de mutation, outils LLM), après relecture de la spec.
Il sert de référence à toutes les PR (a) → (i).

---

## 1. Principes retenus

1. **L'IR est la seule source de vérité.** Canvas, calques, inspecteur, code et IA en sont des clients.
   Toute mutation est une `Command`, abaissée en `Op`s primitives inversibles (undo/redo).
2. **Aucun concept DOM/CSS dans le cœur de l'IR** : primitives et rôles sémantiques, valeurs de style
   abstraites (tokens et échelles). Les échappatoires propres au web sont isolées dans
   `platform_overrides.web` et marquées comme telles.
3. **Les valeurs par défaut sont explicites dans l'IR**, jamais implicites dans le compilateur
   (ex. le focus ring d'un `Button` créé est écrit dans `states.focus_visible`). Condition de
   stabilité de l'aller-retour code ⇄ IR.
4. **Mobile-first** : chaque propriété de style est `{ base, sm?, md?, lg?, xl?, 2xl? }`.
5. **L'IA n'émet que des commandes**, validées par le moteur Rust, appliquées dans un brouillon,
   revues par l'utilisateur, puis validées en une seule transaction d'undo.

---

## 2. Arborescence du monorepo

```
deep_atelier/
├─ Cargo.toml                    # workspace : crates/*, xtask
├─ rust-toolchain.toml           # stable + wasm32-unknown-unknown, clippy, rustfmt
├─ package.json                  # scripts turbo : dev, build, test, lint, codegen
├─ pnpm-workspace.yaml           # apps/*, packages/*
├─ turbo.json
├─ tsconfig.base.json            # strict + noUncheckedIndexedAccess + exactOptionalPropertyTypes
├─ .github/workflows/ci.yml      # cargo test/clippy · wasm · vitest · playwright ×3 · build de l'export · lighthouse
├─ xtask/                        # `cargo xtask codegen` : ts-rs → packages/ir-types, schemars → packages/ai/tools
├─ crates/
│  ├─ ir/                        # schéma, commandes, ops, historique, validation
│  │  ├─ src/
│  │  │  ├─ id.rs  document.rs  node.rs  tokens.rs
│  │  │  ├─ style/{responsive.rs, style.rs, values.rs, color.rs}
│  │  │  ├─ command/{command.rs, spec.rs, lower.rs}
│  │  │  ├─ op.rs  history.rs  scope.rs  query.rs  migrate.rs
│  │  │  └─ validate/{schema.rs, a11y.rs, contrast.rs, responsive.rs}
│  │  └─ tests/                  # commandes, undo/redo proptest (150 actions), validation
│  ├─ compiler-web/              # IR → TSX + Tailwind (Next.js App Router)
│  │  ├─ src/{lib.rs, classes.rs, jsx.rs, theme.rs, scaffold.rs, printer.rs, source_map.rs}
│  │  └─ tests/ + snapshots/     # insta
│  ├─ parser-web/                # TSX → IR via oxc, diff → commandes
│  │  └─ src/{lib.rs, jsx.rs, classes_rev.rs, diff.rs, raw.rs}
│  ├─ engine-wasm/               # façade #[wasm_bindgen] (serde-wasm-bindgen)
│  ├─ api/                       # Axum + Tokio
│  │  └─ src/{main.rs, config.rs, error.rs, auth.rs, db.rs,
│  │          routes/{projects, documents, versions, assets, ai, export, github}.rs,
│  │          ai/{gateway.rs, anthropic.rs, router.rs, agent.rs, context.rs, sse.rs}}
│  └─ compiler-native/           # Phase 4 — nom réservé, non créé en Phase 1
├─ apps/editor/                  # Next.js 15 · React 19 · Tailwind v4 · shadcn/ui · Zustand+Immer · dnd-kit · Monaco
│  ├─ app/{(auth)/login, projects, projects/[id]}/page.tsx
│  ├─ components/ui/
│  ├─ components/editor/{canvas, overlay, layers, inspector, library, pages, tokens,
│  │                     prompt-bar, review, code-panel, problems, toolbar, command-palette}/
│  ├─ lib/{engine.ts, store/, canvas-bridge.ts, api.ts, supabase.ts, shortcuts.ts}
│  └─ e2e/                       # Playwright : projets 390 / 768 / 1280
├─ packages/
│  ├─ ir-types/                  # types TS générés par ts-rs (ne pas éditer)
│  ├─ engine/                    # build wasm + wrapper TS typé
│  ├─ canvas-runtime/            # code de l'iframe : RenderTree → React, mesures, hit-test, actions
│  ├─ canvas-protocol/           # messages parent ↔ iframe + gardes runtime
│  ├─ tokens/                    # préréglages par défaut (JSON) : palette, polices, échelles
│  ├─ ai/{prompts/, tools/ (généré), evals/{cases/ (30), run.ts, graders/}}
│  └─ config/                    # eslint, tsconfig partagés
├─ supabase/{config.toml, migrations/}
└─ docs/{SPEC.md, adr/}
```

Les types TypeScript (ts-rs) et le JSON Schema des outils LLM (schemars) sont générés depuis les
mêmes types Rust ; la CI échoue si `cargo xtask codegen` produit un diff.

---

## 3. Schéma de l'IR (`crates/ir`)

Conventions :

- Toutes les structures dérivent `Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema`
  (+ `Eq, Ord, Hash` pour les ids et les enums). Omis ci-dessous.
- Les scalaires de style se sérialisent en **chaînes compactes** (`"4"`, `"1/2"`, `"container.7xl"`,
  `"primary/80"`) : lisibles, économes en tokens pour le LLM, parsées et validées en Rust.

### 3.1 Identifiants

```rust
pub struct NodeId(String);       // "n_k3f9x2a7qz" — préfixe + 10 car. base36, aléatoire, stable à vie
pub struct PageId(String);       // "p_…"
pub struct LayoutId(String);     // "l_…"
pub struct ComponentId(String);  // "c_…"
pub struct AssetId(String);      // "a_…"
pub struct TokenName(String);    // kebab-case : "primary", "muted-foreground"
```

### 3.2 Document

```rust
pub const IR_VERSION: u32 = 1;

pub struct Document {
    pub version: u32,
    pub name: String,
    pub targets: Vec<Target>,                  // [Web] en v1 ; activer Native = ajouter un compilateur
    pub settings: SiteSettings,
    pub tokens: DesignTokens,
    pub layouts: Vec<Layout>,
    pub pages: Vec<Page>,                      // ordre = ordre de navigation
    pub components: Vec<Component>,
    pub nodes: BTreeMap<NodeId, Node>,         // arène unique : pages, layouts, composants
    pub assets: Vec<Asset>,
}
pub enum Target { Web, Native }
pub struct SiteSettings { pub lang: String, pub site_name: String, pub favicon: Option<AssetId> }

pub struct Layout {                            // → app/(<nom>)/layout.tsx
    pub id: LayoutId,
    pub name: String,
    pub root: NodeId,                          // contient exactement un Slot { name: "page" }
}
pub struct Page {
    pub id: PageId,
    pub name: String,
    pub route: Vec<RouteSegment>,              // [] = "/" ; graphe de routes → App Router / Expo Router
    pub layout: Option<LayoutId>,
    pub root: NodeId,                          // conteneur racine ; sans style ni rôle → fragment
    pub seo: Seo,
}
#[serde(tag = "kind", content = "name")]
pub enum RouteSegment { Static(String), Param(String) }       // "blog" / "[slug]"
pub struct Seo { pub title: String, pub description: String, pub og_image: Option<AssetId> }

pub struct Component {
    pub id: ComponentId,
    pub name: String,                          // PascalCase unique → components/<Name>.tsx
    pub root: NodeId,
    pub props: Vec<ComponentProp>,
    pub variants: Vec<VariantAxis>,
}
pub struct ComponentProp { pub name: String, pub default: PropValue, pub binding: PropBinding }
pub struct PropBinding { pub node: NodeId, pub field: BindableField }
pub enum BindableField { Text, ImageSource, ImageAlt, Href, Label, Visible }
#[serde(tag = "kind", content = "value")]
pub enum PropValue { Text(String), Bool(bool), Href(Href), Image(ImageSource) }

pub struct VariantAxis { pub name: String, pub options: Vec<VariantOption>, pub default: String }
pub struct VariantOption { pub name: String, pub overrides: Vec<VariantOverride> }
pub struct VariantOverride { pub node: NodeId, pub style: StylePatch, pub states: StateStylesPatch }
// Export : props typées `intent?: "primary" | "ghost"` + table de classes en const.

pub struct Asset {
    pub id: AssetId, pub file_name: String, pub mime: String,
    pub storage_path: String, pub width: Option<u32>, pub height: Option<u32>, pub bytes: u64,
}
```

### 3.3 Tokens

```rust
pub struct DesignTokens {
    pub colors: Vec<ColorToken>,
    pub fonts: Vec<FontToken>,
    pub spacing_unit: u8,                      // px par cran (4 par défaut) → --spacing
    pub radii: Vec<RadiusToken>,               // valeur de chaque cran (défauts Tailwind)
    pub shadows: Vec<ShadowToken>,
}
pub struct ColorToken { pub name: TokenName, pub light: Hex, pub dark: Option<Hex> }
pub struct Hex(String);                        // "#1d4ed8" | "#1d4ed8cc"
pub struct FontToken { pub name: TokenName, pub family: FontFamily }
#[serde(tag = "kind")]
pub enum FontFamily { Google { family: String, weights: Vec<u16> }, System { stack: SystemFont } }
pub enum SystemFont { Sans, Serif, Mono }
pub struct RadiusToken { pub step: Radius, pub px: u16 }
pub struct ShadowToken { pub step: Shadow, pub layers: Vec<ShadowLayer> }
pub struct ShadowLayer { pub x: i16, pub y: i16, pub blur: u16, pub spread: i16, pub color: Hex, pub inset: bool }
// Jeu de couleurs par défaut (sémantique shadcn) : background, foreground, primary(-foreground),
// secondary(-foreground), muted(-foreground), accent(-foreground), border, card(-foreground), destructive.
```

### 3.4 Nœuds

```rust
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,                // None ⇔ racine de page, de layout ou de composant
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
    pub style: Style,
    pub states: StateStyles,                   // hover / focus-visible / active (WebOnly)
    pub visibility: Option<Responsive<bool>>,  // → `hidden md:flex`
    pub platform: PlatformScope,
    pub meta: NodeMeta,
    pub a11y: A11y,
    pub platform_overrides: PlatformOverrides,
}
pub enum PlatformScope { All, WebOnly, NativeOnly }
pub enum NodeSource { Visual, Code, Ai }
pub struct NodeMeta {
    pub name: Option<String>,                  // nom du calque
    pub anchor: Option<String>,                // ancre de navigation (#pricing), unique par page
    pub locked: bool,
    pub source: NodeSource,
    pub slot: Option<String>,                  // enfant d'instance : slot ciblé (None = "children")
}
pub struct A11y { pub label: Option<String>, pub hidden: bool }

#[serde(tag = "type")]
pub enum NodeKind {
    Box(ContainerProps),                       // flux
    Stack(ContainerProps),                     // flex 1D
    Grid(ContainerProps),                      // grille ; en natif → lignes de Stack
    Text(TextProps),
    Image(ImageProps),
    Icon(IconProps),
    Button(ButtonProps),
    Input(InputProps),
    Link(LinkProps),
    ComponentInstance(InstanceProps),
    Slot(SlotProps),                           // seulement dans un Component ou un Layout
    RawCode(RawCodeProps),
}

pub struct ContainerProps { pub role: ContainerRole }
#[serde(tag = "kind")]
pub enum ContainerRole {
    Generic, Section, Header, Footer, Nav, Main, Article, Aside, List, ListItem,
    Form { action: Option<String>, method: FormMethod },
}
pub enum FormMethod { Get, Post }

pub struct TextProps { pub role: TextRole, pub content: Vec<TextRun> }   // "\n" = saut de ligne
#[serde(tag = "kind")]
pub enum TextRole {
    Heading { level: HeadingLevel },
    Paragraph, Inline, Caption, Quote, Code,
    Label { for_input: Option<NodeId> },
}
pub enum HeadingLevel { H1, H2, H3, H4, H5, H6 }
pub struct TextRun { pub text: String, pub strong: bool, pub em: bool, pub code: bool, pub color: Option<ColorRef> }

pub struct ImageProps {
    pub source: ImageSource,
    pub alt: String,
    pub intrinsic: Option<Dimensions>,         // obligatoire pour Asset/Url (CLS, next/image)
    pub priority: bool,                        // image LCP
}
#[serde(tag = "kind")]
pub enum ImageSource {
    Asset { id: AssetId },
    Url { url: String },
    Placeholder { label: String, ratio: AspectRatio },   // → SVG local à l'export
}
pub struct Dimensions { pub width: u32, pub height: u32 }

pub struct IconProps { pub name: String }      // nom lucide kebab-case, validé contre le catalogue

pub struct ButtonProps {
    pub label: Option<String>,                 // None ⇒ enfants (Icon + Text) ; a11y.label si icône seule
    pub button_type: ButtonType,
    pub disabled: bool,
    pub action: Option<Action>,
}
pub enum ButtonType { Button, Submit, Reset }
#[serde(tag = "kind")]
pub enum Action {
    ToggleVisibility { target: NodeId },       // menu burger ; web : composant client minimal (useState)
}

pub struct InputProps {
    pub input_type: InputType,
    pub name: String,
    pub placeholder: Option<String>,
    pub required: bool,
    pub autocomplete: Option<Autocomplete>,
}
pub enum InputType { Text, Email, Password, Number, Tel, Url, Search, Multiline }
pub enum Autocomplete { Off, Name, Email, Tel, Organization, StreetAddress, PostalCode, Country }

pub struct LinkProps {
    pub href: Href,
    pub label: Option<String>,                 // Some ⇒ feuille ; None ⇒ conteneur
    pub new_tab: bool,
}
#[serde(tag = "kind")]
pub enum Href {
    Page { page: PageId, anchor: Option<String> },        // résiste au renommage de route
    Anchor { anchor: String },
    External { url: String },
    Email { address: String },
    Phone { number: String },
}

pub struct InstanceProps {
    pub component: ComponentId,
    pub overrides: Vec<PropOverride>,
    pub variants: Vec<VariantChoice>,
}
pub struct PropOverride { pub prop: String, pub value: PropValue }
pub struct VariantChoice { pub axis: String, pub option: String }

pub struct SlotProps { pub name: String }      // "children" | "page" (layout) | nom libre

pub struct RawCodeProps {
    pub code: String,                          // JSX conservé à l'identique, jamais réécrit
    pub imports: Vec<ImportDecl>,
    pub client: bool,                          // "use client" → extrait en composant client
}
pub struct ImportDecl { pub module: String, pub default: Option<String>, pub named: Vec<String> }

pub struct PlatformOverrides { pub web: Option<WebOverrides> }
pub struct WebOverrides {
    pub extra_classes: Vec<String>,            // classes Tailwind non modélisées, conservées telles quelles
    pub extra_attributes: Vec<Attribute>,      // data-*, aria-* non modélisés (jamais on*, style, className)
}
pub struct Attribute { pub name: String, pub value: String }
```

### 3.5 Style responsive

```rust
pub enum Breakpoint { Base, Sm, Md, Lg, Xl, #[serde(rename = "2xl")] Xxl }
// min-width : sm 640 · md 768 · lg 1024 · xl 1280 · 2xl 1536

pub struct Responsive<T> {
    pub base: T,
    pub sm: Option<T>, pub md: Option<T>, pub lg: Option<T>, pub xl: Option<T>,
    #[serde(rename = "2xl")] pub xxl: Option<T>,
}
impl<T> Responsive<T> {
    pub fn get(&self, bp: Breakpoint) -> Option<&T>;              // valeur déclarée
    pub fn resolve(&self, bp: Breakpoint) -> &T;                  // valeur effective (cascade mobile-first)
    pub fn origin(&self, bp: Breakpoint) -> Breakpoint;           // provenance (affichée par l'inspecteur)
}
```

Chaque champ de `Style` est un `Option<Responsive<T>>` (absent = non défini). Poser une valeur en
`lg` sur une propriété absente crée `{ base: <valeur neutre>, lg: v }` : le rendu en `base` ne change
pas, et le compilateur n'émet pas la valeur neutre. Chaque propriété porte un marqueur statique de
plateforme (`StyleProp::scope() -> PlatformScope`).

```rust
pub struct Style {
    // conteneur
    pub direction: Option<Responsive<Direction>>,     // Stack
    pub wrap: Option<Responsive<bool>>,               // Stack
    pub columns: Option<Responsive<GridColumns>>,     // Grid
    pub gap: Option<Responsive<Space>>,
    pub align: Option<Responsive<Align>>,
    pub justify: Option<Responsive<Justify>>,
    // enfant de layout
    pub grow: Option<Responsive<bool>>,
    pub shrink: Option<Responsive<bool>>,
    pub align_self: Option<Responsive<Align>>,
    pub col_span: Option<Responsive<GridSpan>>,       // enfant de Grid
    pub order: Option<Responsive<Order>>,
    // espacement (par côté : surcharger un côté ne recopie pas les autres)
    pub padding_top: Option<Responsive<Space>>,    pub padding_right: Option<Responsive<Space>>,
    pub padding_bottom: Option<Responsive<Space>>, pub padding_left: Option<Responsive<Space>>,
    pub margin_top: Option<Responsive<Margin>>,    pub margin_right: Option<Responsive<Margin>>,
    pub margin_bottom: Option<Responsive<Margin>>, pub margin_left: Option<Responsive<Margin>>,
    // dimensions
    pub width: Option<Responsive<Size>>,  pub min_width: Option<Responsive<Size>>,  pub max_width: Option<Responsive<Size>>,
    pub height: Option<Responsive<Size>>, pub min_height: Option<Responsive<Size>>, pub max_height: Option<Responsive<Size>>,
    pub aspect_ratio: Option<Responsive<AspectRatio>>,
    // position
    pub position: Option<Responsive<Position>>,
    pub top: Option<Responsive<Inset>>,    pub right: Option<Responsive<Inset>>,
    pub bottom: Option<Responsive<Inset>>, pub left: Option<Responsive<Inset>>,
    pub z_index: Option<Responsive<ZIndex>>,
    // typographie (héritée par les descendants)
    pub font_family: Option<Responsive<TokenName>>,
    pub font_size: Option<Responsive<FontSize>>,
    pub font_weight: Option<Responsive<FontWeight>>,
    pub line_height: Option<Responsive<LineHeight>>,
    pub letter_spacing: Option<Responsive<LetterSpacing>>,
    pub text_align: Option<Responsive<TextAlign>>,
    pub text_color: Option<Responsive<ColorRef>>,
    pub text_transform: Option<Responsive<TextTransform>>,
    pub text_decoration: Option<Responsive<TextDecoration>>,
    pub text_wrap: Option<Responsive<TextWrap>>,
    // visuel
    pub background: Option<Responsive<Background>>,
    pub border_top_width: Option<Responsive<BorderWidth>>,    pub border_right_width: Option<Responsive<BorderWidth>>,
    pub border_bottom_width: Option<Responsive<BorderWidth>>, pub border_left_width: Option<Responsive<BorderWidth>>,
    pub border_color: Option<Responsive<ColorRef>>,
    pub border_style: Option<Responsive<BorderStyle>>,
    pub radius: Option<Responsive<Radius>>,
    pub shadow: Option<Responsive<Shadow>>,
    pub opacity: Option<Responsive<Opacity>>,
    pub overflow: Option<Responsive<Overflow>>,
    pub object_fit: Option<Responsive<ObjectFit>>,    // Image
    // mouvement (WebOnly)
    pub transition: Option<Responsive<Transition>>,
    pub duration: Option<Responsive<Duration>>,
}

pub struct StateStyles { pub hover: StateStyle, pub focus_visible: StateStyle, pub active: StateStyle }
pub struct StateStyle {
    pub text_color: Option<Responsive<ColorRef>>, pub background: Option<Responsive<Background>>,
    pub border_color: Option<Responsive<ColorRef>>, pub opacity: Option<Responsive<Opacity>>,
    pub shadow: Option<Responsive<Shadow>>, pub text_decoration: Option<Responsive<TextDecoration>>,
    pub scale: Option<Responsive<Scale>>, pub ring: Option<Responsive<Ring>>,
}
pub struct Ring { pub width: RingWidth, pub color: ColorRef, pub offset: RingWidth }
```

### 3.6 Valeurs (forme sérialisée)

```rust
pub enum Space { /* "0" "px" "0.5" "1" "1.5" "2" "2.5" "3" "3.5" "4" "5" "6" "7" "8" "9" "10" "11" "12"
                    "14" "16" "20" "24" "28" "32" "36" "40" "44" "48" "52" "56" "60" "64" "72" "80" "96" */ }
pub enum Margin { Space(Space), Auto }                       // "4" | "auto"
pub enum Inset  { Space(Space), Auto, Full }                 // "0" | "auto" | "full"
pub enum Size {
    Auto, Full, Screen, Fit, Min, Max,                       // "auto" "full" "screen" "fit" "min" "max"
    Fraction(Fraction),                                      // "1/2" "1/3" "2/3" "1/4" "3/4"
    Space(Space),                                            // "16"
    Container(ContainerSize),                                // "container.xs" … "container.7xl" | "prose"
    Px(u16),                                                 // "372px" — échappatoire, Warning
}
pub enum Direction { Row, Column, RowReverse, ColumnReverse }
pub enum Align { Start, Center, End, Stretch, Baseline }
pub enum Justify { Start, Center, End, Between, Around, Evenly }
pub enum GridColumns { /* "1".."12" */ }
pub enum GridSpan { /* "1".."12" | "full" */ }
pub enum Order { First, Last, Default }
pub enum AspectRatio { Auto, Square, Video, Portrait, Landscape, Wide }   // 1, 16/9, 3/4, 4/3, 21/9
pub enum Position { Static, Relative, Absolute, Fixed, Sticky }
pub enum ZIndex { Auto, Z0, Z10, Z20, Z30, Z40, Z50 }
pub enum FontSize { Xs, Sm, Base, Lg, Xl, Xl2, Xl3, Xl4, Xl5, Xl6, Xl7, Xl8, Xl9 }   // "xs" … "9xl"
pub enum FontWeight { Thin, Extralight, Light, Normal, Medium, Semibold, Bold, Extrabold, Black }
pub enum LineHeight { None, Tight, Snug, Normal, Relaxed, Loose }
pub enum LetterSpacing { Tighter, Tight, Normal, Wide, Wider, Widest }
pub enum TextAlign { Start, Center, End, Justify }
pub enum TextTransform { None, Uppercase, Lowercase, Capitalize }
pub enum TextDecoration { None, Underline, LineThrough }
pub enum TextWrap { Wrap, NoWrap, Balance, Pretty }
pub struct ColorRef { pub source: ColorSource, pub alpha: Option<Alpha> }  // "primary" "primary/80" "slate-900" "white"
pub enum ColorSource { Token(TokenName), Palette { hue: Hue, shade: Shade }, White, Black, Transparent, Current }
#[serde(tag = "kind")]
pub enum Background { Color { color: ColorRef }, Gradient { direction: GradientDir, from: ColorRef, via: Option<ColorRef>, to: ColorRef } }
pub enum GradientDir { ToT, ToTr, ToR, ToBr, ToB, ToBl, ToL, ToTl }
pub enum BorderWidth { W0, W1, W2, W4, W8 }
pub enum BorderStyle { Solid, Dashed, Dotted, None }
pub enum Radius { None, Sm, Md, Lg, Xl, Xl2, Xl3, Full }
pub enum Shadow { None, Xs, Sm, Md, Lg, Xl, Xl2 }
pub enum Opacity { /* "0" "5" … "100", par pas de 5 */ }
pub enum Alpha   { /* "5" … "95", par pas de 5 */ }
pub enum Overflow { Visible, Hidden, Clip, Auto, Scroll }
pub enum ObjectFit { Cover, Contain, Fill, None, ScaleDown }
pub enum Transition { None, Colors, Opacity, Shadow, Transform, All }
pub enum Duration { D75, D100, D150, D200, D300, D500, D700, D1000 }
pub enum Scale { S95, S100, S105, S110 }
pub enum RingWidth { W0, W1, W2, W4 }
```

---

## 4. Validation

```rust
pub struct Issue {
    pub code: IssueCode,                       // ex. A11Y_IMG_ALT, RESP_FIXED_WIDTH_OVERFLOW
    pub severity: Severity,                    // Error (bloque l'acceptation) | Warning | Info
    pub node: Option<NodeId>,
    pub breakpoint: Option<Breakpoint>,
    pub message: String,                       // anglais (renvoyé au LLM) ; l'UI traduit via `code`
    pub fix: Option<Vec<Command>>,             // correction proposée
}
```

| Famille | Règles | Sévérité |
|---|---|---|
| Schéma | arbre cohérent (parent/children, pas de cycle, racines) ; feuilles : Text, Image, Icon, Input, RawCode, Button/Link avec `label` ; pas d'interactif dans un interactif ; `ListItem` sous `List` ; `Slot` seulement dans Component/Layout, un seul `Slot "page"` par Layout ; style autorisé selon la primitive (`columns` → Grid, `direction`/`wrap` → Stack, `object_fit` → Image) ; références valides (page, layout, composant, asset, token, ancre, cible d'`Action`) ; routes et ancres uniques ; icône connue | Error |
| A11y | `alt` obligatoire sauf `a11y.hidden` ; un seul H1 par page, pas de saut de niveau ; nom accessible pour Button/Link/Input/Icon porteuse de sens ; contraste WCAG AA calculé à chaque breakpoint ; un seul `Main` ; `lang` défini | Error |
| Responsive | largeur fixe > 390 px à `base` (fix proposé : `w-full max-w-…`) ; Stack `row` à `base` avec > 3 enfants sans `wrap` ; > 2 colonnes à `base` ; titre ≥ 5xl à `base` ; cible tactile < 44 × 44 px à `base` ; champ de saisie < 16 px à `base` (zoom iOS) | Warning |
| Rendu (navigateur) | débordement horizontal mesuré à 390 / 768 / 1280 px (et à la publication : tous les breakpoints) | Error |
| Qualité | couleur hors tokens, valeur `Px`, conteneur vide | Info |

---

## 5. Commandes, ops et historique

### 5.1 Commandes (API publique)

Utilisées telles quelles par l'UI, le parser (synchro code → IR) et le LLM.
`NodeRef` = `"n_…"` (id existant) ou `"$nom"` (référence locale créée plus tôt dans la transaction ou le run IA).

```rust
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    // Structure
    InsertNodes      { parent: NodeRef, index: Option<u32>, nodes: Vec<NodeSpec> },
    ReplaceNode      { node: NodeRef, nodes: Vec<NodeSpec> },
    DeleteNodes      { nodes: Vec<NodeRef> },
    MoveNode         { node: NodeRef, parent: NodeRef, index: u32 },
    DuplicateNode    { node: NodeRef },
    WrapNodes        { nodes: Vec<NodeRef>, container: ContainerSpec },   // frères contigus
    UnwrapNode       { node: NodeRef },
    ConvertContainer { node: NodeRef, to: ContainerKind },                // Box ⇄ Stack ⇄ Grid
    // Contenu, style, méta
    SetProps         { node: NodeRef, props: PropsPatch },
    SetStyle         { node: NodeRef, state: Option<InteractionState>, style: StylePatch },
    SetVisibility    { node: NodeRef, visibility: Option<ResponsivePatch<bool>> },
    SetMeta          { node: NodeRef, meta: MetaPatch },                  // nom, ancre, verrou, slot, a11y
    SetPlatform      { node: NodeRef, scope: PlatformScope },             // UI seulement
    SetWebOverrides  { node: NodeRef, overrides: WebOverrides },          // UI et parser seulement
    // Composants
    CreateComponent  { node: NodeRef, name: String, props: Vec<ComponentPropSpec> },
    UpdateComponent  { component: ComponentId, name: Option<String>,
                       props: Option<Vec<ComponentPropSpec>>, variants: Option<Vec<VariantAxis>> },
    DetachInstance   { node: NodeRef },
    DeleteComponent  { component: ComponentId },                          // refusée s'il reste des instances
    // Layouts et pages
    CreateLayout     { name: String },                                    // racine + Slot "page"
    UpdateLayout     { layout: LayoutId, name: String },
    DeleteLayout     { layout: LayoutId },                                // refusée s'il est utilisé
    CreatePage       { name: String, route: Vec<RouteSegment>, layout: Option<LayoutId>, seo: Option<Seo> },
    UpdatePage       { page: PageId, name: Option<String>, route: Option<Vec<RouteSegment>>,
                       layout: Option<Option<LayoutId>>, seo: Option<Seo> },
    DeletePage       { page: PageId },
    // Tokens, site, assets
    SetToken         { edit: TokenEdit },                                 // couleur, police, rayon, ombre, unité
    UpdateSettings   { lang: Option<String>, site_name: Option<String>, favicon: Option<Option<AssetId>> },
    AddAsset         { asset: Asset },
    RemoveAsset      { asset: AssetId },                                  // refusée si référencé
}

/// Liste PLATE (non récursive : le mode `strict` des outils Claude refuse les schémas récursifs).
/// La hiérarchie s'exprime par `parent` ; permet aussi le streaming nœud par nœud de l'aperçu fantôme.
pub struct NodeSpec {
    pub r#ref: Option<String>,                 // "$pricing-grid"
    pub parent: Option<String>,                // ref d'un NodeSpec précédent ; absent ⇒ parent de la commande
    pub kind: KindSpec,                        // NodeKind à champs optionnels → défauts sûrs
    pub style: Option<StylePatch>,
    pub visibility: Option<ResponsivePatch<bool>>,
    pub hover: Option<StatePatch>,
    pub focus_visible: Option<StatePatch>,
    pub meta: Option<MetaPatch>,
}

#[serde(tag = "kind")]
pub enum TokenEdit {
    Color   { name: TokenName, value: Option<ColorTokenValue> },   // None = suppression (refusée si utilisé)
    Font    { name: TokenName, value: Option<FontFamily> },
    Radius  { step: Radius, px: u16 },
    Shadow  { step: Shadow, layers: Vec<ShadowLayer> },
    SpacingUnit { px: u8 },
}
```

Sémantique des patchs de style : champ absent = inchangé ; champ `null` = propriété supprimée (tous
breakpoints) ; dans l'objet, breakpoint absent = inchangé, `null` = surcharge effacée (« réinitialiser
à l'héritage »), valeur = posée. Raccourcis acceptés en entrée et développés par le moteur :
`padding`, `padding_x`, `padding_y`, `margin_x`, `margin_y`, `border_width`.

### 5.2 Ops primitives

```rust
/// `apply(&mut Document) -> Op` renvoie l'op inverse. Phase 2 : 1 Op ↔ 1 mise à jour Yjs.
pub enum Op {
    InsertSubtree { parent: NodeId, index: u32, nodes: Vec<Node> },
    RemoveSubtree { root: NodeId },
    MoveNode      { node: NodeId, parent: NodeId, index: u32 },
    SetKind       { node: NodeId, kind: NodeKind },
    SetStyleProp  { node: NodeId, state: Option<InteractionState>, prop: StyleProp, value: Option<ResponsiveValue> },
    SetVisibility { node: NodeId, visibility: Option<Responsive<bool>> },
    SetMeta { node: NodeId, meta: NodeMeta },   SetA11y { node: NodeId, a11y: A11y },
    SetPlatform { node: NodeId, scope: PlatformScope },
    SetOverrides { node: NodeId, overrides: PlatformOverrides },
    PutLayout { index: u32, layout: Layout },      RemoveLayout { id: LayoutId },
    PutPage { index: u32, page: Page },            RemovePage { id: PageId },
    PutComponent { index: u32, component: Component }, RemoveComponent { id: ComponentId },
    SetTokens { tokens: DesignTokens },
    PutAsset { asset: Asset },                     RemoveAsset { id: AssetId },
    SetSettings { settings: SiteSettings },
}
```

### 5.3 Transactions, périmètre, historique

```rust
pub enum Origin { User, Ai { run_id: String }, Code, System }
pub struct Scope {
    pub content_only: bool,                    // mode Contenu : textes, images, liens seulement
    pub breakpoint: Option<Breakpoint>,        // prompt sur breakpoint : valeurs de ce breakpoint seulement
    pub subtree: Option<Vec<NodeId>>,          // prompt sur sélection
}
pub struct Transaction { pub label: String, pub origin: Origin, pub scope: Scope, pub commands: Vec<Command> }

impl Session {
    pub fn apply(&mut self, tx: Transaction) -> Result<Applied, CommandError>;  // atomique, rollback si échec ;
                                                                                // CommandError::OutOfScope hors périmètre
    pub fn undo(&mut self) -> Option<ChangeSet>;                                // illimité dans la session
    pub fn redo(&mut self) -> Option<ChangeSet>;
    pub fn begin_gesture(&mut self, label: &str);                               // slider, drag → 1 entrée
    pub fn end_gesture(&mut self);
    pub fn begin_draft(&mut self, run_id: &str);                                // brouillon IA, hors historique
    pub fn commit_draft(&mut self, accept: DraftAccept) -> Result<ChangeSet, CommandError>;
    pub fn discard_draft(&mut self) -> ChangeSet;
    pub fn validate(&self) -> Vec<Issue>;
}
pub enum DraftAccept { All, Units(Vec<ChangeUnitId>) }
// Applied { ops, inverse, created: "$ref" → NodeId, changes: ChangeSet, issues }
// Acceptation nœud par nœud : rejeu filtré des commandes du run, revalidé, toujours 1 entrée d'undo.
```

---

## 6. Outils LLM

Générés depuis `Command` par schemars, `strict: true`, `eager_input_streaming: true` sur les outils de
mutation. Exposés : `get_outline`, `get_nodes` (lecture), `insert_nodes`, `replace_node`,
`delete_nodes`, `move_node`, `duplicate_node`, `wrap_nodes`, `unwrap_node`, `convert_container`,
`set_props`, `set_style`, `set_visibility`, `set_meta`, `create_component`, `update_component`,
`detach_instance`, `create_layout`, `create_page`, `update_page`, `set_token`,
`finish_run { summary, commit_message }`.

Jamais exposés à l'IA : `SetPlatform`, `SetWebOverrides`, création de `RawCode`, suppression de
pages/layouts/composants, assets.

Exemple `insert_nodes` :

```json
{
  "parent": "n_root01", "index": 2,
  "nodes": [
    { "ref": "$features", "kind": { "type": "Box", "role": { "kind": "Section" } },
      "meta": { "name": "Features", "anchor": "features" },
      "style": { "padding_y": { "base": "16", "md": "24" }, "padding_x": { "base": "4", "md": "8" } } },
    { "parent": "$features",
      "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h2" }, "content": [{ "text": "Pourquoi Deep Atelier" }] },
      "style": { "font_size": { "base": "3xl", "md": "4xl" }, "font_weight": { "base": "bold" },
                 "text_align": { "base": "center" }, "text_wrap": { "base": "balance" } } },
    { "ref": "$grid", "parent": "$features", "kind": { "type": "Grid", "role": { "kind": "List" } },
      "style": { "columns": { "base": "1", "md": "2", "lg": "3" }, "gap": { "base": "6", "lg": "8" },
                 "max_width": { "base": "container.6xl" }, "margin_x": { "base": "auto" }, "margin_top": { "base": "10" } } }
  ]
}
```

Résultat : `{ "ok": true, "created": { "$features": "n_…", "$grid": "n_…" }, "issues": [...] }`.
Échec : `is_error: true`, `{ "ok": false, "error": { "code", "message" } }`, rien n'est appliqué.

---

## 7. Couche IA (`crates/api/src/ai`, `packages/ai`)

- **Boucle côté serveur** sur la crate `ir` native, appliquée à un instantané du document envoyé
  par le client ; le client rejoue les commandes résolues (avec ids) dans son brouillon wasm.
- **Contexte ciblé** : IR compacte de la sélection et de ses parents, tokens, catalogue de composants
  (props, variantes), breakpoint actif, `Scope`. Jamais le projet entier ; `get_outline`/`get_nodes`
  à la demande.
- **Modèles** : `claude-opus-5-5` par défaut (thinking adaptatif, `output_config.effort` explicite,
  réglé par les évals) ; `claude-haiku-4-5` pour les retouches simples (sélection courte, style ou
  texte), routage dans la passerelle, forçable. `tool_choice: auto` (forcer un outil renvoie 400 sur
  Opus 5.5). Fallback serveur en cas de refus (`fallbacks: "default"`). Appels HTTP directs
  (reqwest + SSE) derrière un trait `LlmProvider` — pas de SDK Anthropic officiel en Rust.
- **Cache** : outils (ordre déterministe) + prompt système figé en préfixe ; contexte variable dans
  les messages ; historique en ajout seul (« Ajuster » ajoute un message, ne réécrit rien).
- **Streaming** : chaque élément complet de `nodes` est poussé au client comme nœud fantôme ;
  chaque outil terminé est validé puis appliqué au brouillon.
- **Garde-fous** : erreur de schéma, d'a11y ou de rendu (390 / 768 / 1280 px, mesuré dans des iframes
  cachées) renvoyée **une fois** au modèle, puis signalée à l'utilisateur. Un rejet ne touche pas l'IR.
- **Revue** : tout accepter, accepter nœud par nœud, ajuster, rejeter. Un prompt accepté = 1 undo.
- **Journal** : `ai_runs` lié à `document_versions`, message de commit issu de `finish_run` ;
  quotas par plan ; métriques : taux d'acceptation (cible ≥ 60 %), temps jusqu'au premier nœud (< 3 s).
- **Évals** : 30 cas de référence (`packages/ai/evals/cases`), contrôles déterministes (validation
  Rust, rendu aux 3 largeurs via Playwright) + captures.

---

## 8. Canvas et synchronisation

- **Canvas** : iframe `sandbox="allow-scripts"` (origine opaque). Elle reçoit un `RenderTree` produit
  par compiler-web avec **les mêmes fonctions de classes que l'export** ; Tailwind compilé dans
  l'iframe par `@tailwindcss/browser`. Le parent dessine l'overlay (sélection, survol, guides,
  mesures, indicateurs de dépôt) et dialogue par postMessage (`patch`, `hit_test`, `rects`,
  `layout_report`). Largeurs : 390 / 640 / 768 / 1024 / 1280 / 1536 + libre ; multi-artboards.
- **RawCode** : rendu dans l'iframe (transpilé sur place, imports tiers résolus via esm.sh, dans le
  canvas seulement), avec error boundary ; jamais réécrit par le compilateur.
- **Canvas → code** : commande → compilation incrémentale du seul fichier touché → éditions minimales
  appliquées au modèle Monaco (curseur et undo Monaco préservés).
- **Code → canvas** (spec) : chaque élément JSX porte `data-atl-id` en mode édition (atténué
  visuellement dans Monaco, retiré à l'export). Frappe → débounce 150 ms → parse oxc → diff d'AST
  → commandes (origine `Code`) → canvas. Un élément sans id reçoit un nouvel id.
- **Lien code ⇄ canvas** : clic au canvas = curseur sur la ligne JSX (source map du compilateur) ;
  clic sur une ligne = sélection au canvas.

---

## 9. Persistance (Supabase)

`projects`, `project_documents` (IR en jsonb, `version` pour la concurrence optimiste),
`document_versions` (instantanés nommés, messages de commit), `assets`, `ai_runs`, `plans`,
`usage_monthly`. RLS par propriétaire. Toute sauvegarde est revalidée par l'API Rust.

---

## 10. Écarts assumés par rapport à la spec

1. Props **typées par primitive** au lieu de `props: Record<string, PropValue>` (validation plus forte,
   schémas d'outils stricts ; union discriminée côté TS).
2. Padding et marge **par côté** au lieu de `Responsive<BoxSpacing>`.
3. Tailwind v4 : thème exporté en **`@theme` dans `globals.css`** au lieu d'un `tailwind.config`.
4. `packages/tokens` ne contient que les préréglages ; la compilation tokens → CSS reste en Rust
   (wasm + serveur).
5. Canvas = **interprète de l'IR** avec les classes de l'export, pas un build du code exporté.

---

## 11. Découpage des étapes (une PR chacune)

- **(a)** `crates/ir` : schéma, commandes, ops, `Scope`, brouillon, historique, validation + tests.
- **(b)** `compiler-web` : pages, layouts, composants, variantes, actions, thème + snapshots insta
  + build réel du projet exporté en CI.
- **(b2)** `crates/api` + Supabase : auth, projets, sauvegarde automatique, versions.
- **(c)** `engine-wasm` + canvas iframe : sélection, survol, multi-sélection, zoom/pan, artboards, Preview.
- **(d)** Calques, inspecteur par breakpoint, pages/routes/layouts, tokens, mode Contenu.
- **(e)** Glisser-déposer : primitives, sections prêtes (navbar avec burger…), guides et mesures.
- **(f)** Couche IA : barre de prompt, outils, validation, aperçu fantôme, revue.
- **(g)** Panneau Monaco : modes Split/Code, lien canvas ⇄ JSX, ⌘K.
- **(h)** `parser-web` : TSX → IR, synchro inverse.
- **(i)** Export ZIP, push/pull GitHub, historique des versions.

---

## 12. Questions ouvertes

Posées au propriétaire une par une, juste avant l'étape qu'elles conditionnent. Chaque réponse est
consignée ici (question → décision, avec la date).

**Avant (a)**

1. Data binding (« Should » dans la spec) : dans la Phase 1 ? Si oui, le schéma l'intègre dès (a) ;
   sinon il arrive plus tard par une migration d'IR. Conditionne aussi le contenu d'une route dynamique.
   - **Décision (2026-10-06) : hors Phase 1.** L'IR v1 ne contient ni source de données ni liste
     répétée ; ils arriveront par une migration (`migrate.rs`, `IR_VERSION` 2). En Phase 1, le
     contenu d'une route dynamique (`[slug]`) reste statique.
2. Validation du découpage (§ 11), dont la nouvelle étape (b2).
   - **Décision (2026-10-06) : découpage validé tel quel**, (b2) entre (b) et (c).

**Avant (b)**

3. Version de Next.js du code exporté (l'éditeur reste en Next 15, imposé par la spec).
   - **Décision (2026-10-07) : Next.js 16** (16.4.0, dernière stable à la date de la décision).
     Le compilateur épingle les versions exactes dans le `package.json` exporté.
4. Mode sombre du site exporté.
   - **Décision (2026-10-07) : suivre le système.** Les valeurs sombres des tokens sont émises dans
     une media query `prefers-color-scheme: dark` : aucun JavaScript, aucun flash au chargement.
5. Style posé sur une instance de composant (il devient la prop `className` du composant exporté).
   - **Décision (2026-10-07) : tout style est permis, résolu par `tailwind-merge`.** La racine du
     composant fusionne ses classes avec `className` (`twMerge`) : en cas de conflit, la classe de
     l'instance l'emporte. `tailwind-merge` devient une dépendance du code exporté.

**Avant (b2)**

6. Projet Supabase hébergé : organisation, région, offre.

**Avant (c)**

7. Langue de l'interface de l'éditeur : français, anglais, ou i18n dès le départ ?
8. Build de l'app exportée dans l'éditeur (onglet Terminal) en Phase 1 ? Recommandation : non,
   build seulement à l'export, dans un job serveur.

**Avant (f)**

9. Prompt à partir d'une URL (multimodal) en Phase 1 ? Demande un service de capture headless.

---

## 13. Environnement de développement (décidé le 2026-10-06)

- **Dépôt** public : `github.com/CheikhMohamedD/deep_atelier`, sans licence pour l'instant (tous droits
  réservés). Commits signés de l'adresse noreply GitHub du propriétaire (voir `CLAUDE.md`).
- **Développement dans des sessions Claude Code cloud**, rien n'est installé sur le poste local.
  L'image cloud fournit rustc/cargo, Node 22 avec pnpm, Docker et PostgreSQL 16.
- **Outillage complémentaire** (cible `wasm32-unknown-unknown`, `wasm-bindgen-cli`, navigateurs
  Playwright) : ajouté au script de setup de l'environnement cloud à l'étape (c), pour être mis en
  cache. Les téléchargements de releases GitHub d'autres dépôts sont bloqués : `wasm-bindgen-cli`
  s'installe par `cargo install`, à la version exacte de la crate `wasm-bindgen`.
- **Supabase hébergé**, pas de Docker local. Le projet est créé à l'étape (b2). L'accès réseau
  « Trusted » des sessions cloud n'inclut pas Supabase : à l'étape (b2), passer l'environnement en
  « Custom » (liste par défaut + `*.supabase.co`, `api.supabase.com`).
- **Secrets** : jamais dans le dépôt (il est public). Ils vont dans les variables d'environnement
  de l'environnement cloud ou, sur Pro/Max, dans ses credentials API. `ANTHROPIC_API_KEY` (évals,
  étape f) reste une variable d'environnement : le proxy n'attache pas de credential à `api.anthropic.com`.

---

## 14. Précisions et écarts d'implémentation de l'étape (a) (2026-10-06)

Le schéma des §§ 3 à 5 est implémenté tel quel dans `crates/ir`, aux points suivants près.

**Schéma**

1. `Style.align_self` utilise un type dédié `AlignSelf` (`auto | start | center | end | stretch |
   baseline`) : `auto` est la valeur neutre et n'existe pas dans `Align`.
2. `Size` gagne `none`, valide seulement pour `max_width` / `max_height` (leur valeur neutre) ; ailleurs,
   erreur de validation `STYLE_NOT_ALLOWED`.
3. Valeur neutre posée quand une surcharge crée une propriété absente (§ 3.5) : valeur initiale
   CSS/Tailwind pour les propriétés non héritées ; valeur héritée des ancêtres (à `base`) pour la
   typographie ; valeur hors état pour un état d'interaction. Le compilateur (b) n'émet pas une base
   égale à ce neutre.
4. Palette : les 26 teintes de Tailwind 4.3 (dont `mauve`, `olive`, `mist`, `taupe`), valeurs OKLCH
   embarquées (`crates/ir/data/tailwind-palette.txt`) pour le calcul de contraste. Les noms de token
   de la forme `<teinte>-<nuance>` et `white | black | transparent | current` sont réservés.
5. Icônes : catalogue lucide 1.52.0 embarqué (`crates/ir/data/lucide-icons.txt`). Le compilateur (b)
   devra épingler `lucide-react` à la même version.
6. Côté TypeScript, les valeurs composites sérialisées en chaîne (`Size`, `Margin`, `Inset`,
   `ColorRef`, `Hex`, ids, `NodeRef`) sont typées `string` ; les énumérations fermées (`Space`,
   `FontSize`…) sont des unions de littéraux. Les champs omis à la sérialisation sont optionnels.

**Commandes**

7. `KindSpec::Text` accepte `for_input` (référence `$nom` ou id) et `KindSpec::Button.action` une
   `ActionSpec` à cible `NodeRef` : un bouton burger et son menu, ou une étiquette et son champ,
   se créent dans la même commande.
8. `MetaPatch` est plat : `name`, `anchor`, `locked`, `slot`, `a11y_label`, `a11y_hidden`.
9. `create_component` déplace les propriétés de placement de la racine (marges, `grow`, `shrink`,
   `align_self`, `col_span`, `order`) sur l'instance créée, pour que la mise en page ne bouge pas.
10. Les défauts explicites d'un nœud créé sont posés par la commande : `Stack` → `direction: column`,
    `Grid` → `columns: 1`, `Button`/`Link`/`Input` → anneau de focus (token `ring`, sinon `primary`),
    `Icon` → décorative (`a11y.hidden`) sauf étiquette fournie.
11. Origine `Ai` : en plus des commandes non exposées (§ 6), la création et l'édition de `RawCode`
    sont refusées (`FORBIDDEN`).
12. Périmètre : `content_only` n'autorise que `set_props` sur les champs de contenu (texte, source et
    alt d'image, lien, libellé de bouton) ; `breakpoint` n'autorise que `set_style` et
    `set_visibility` sur ce seul breakpoint ; `subtree` refuse toute commande hors nœuds (pages,
    tokens, composants).

**Ops et historique**

13. `Op::InsertSubtree.parent` est un `Option<NodeId>` (`None` : racine de page, layout ou
    composant) et `Op::PutAsset` porte un `index`, pour que l'undo restaure l'ordre exact.
14. `Session::new(doc, seed)` : la graine du générateur d'ids est fournie par l'hôte
    (`crypto.getRandomValues` en wasm), la crate reste déterministe et sans dépendance système.
15. `undo` / `redo` renvoient `Result<Option<ChangeSet>, CommandError>` (refusés pendant un brouillon
    IA). `begin_draft` renvoie une erreur si un brouillon est déjà ouvert.
16. Unité de revue d'un brouillon (`ChangeUnitId`) = une commande du run. L'acceptation partielle
    rejoue les ops des unités choisies (indices ramenés dans les bornes) ; une unité qui dépend d'une
    unité rejetée fait échouer l'acceptation (`UNIT_DEPENDENCY`), brouillon conservé. Seules les
    erreurs absentes avant le brouillon bloquent sa validation.

**Validation**

17. Un saut de niveau de titre n'est signalé qu'entre deux titres consécutifs ; l'absence de H1 ou
    de `Main` est un avertissement (une page en construction n'est pas bloquée).
18. Le contraste est calculé dans les ancêtres du même arbre (une instance ne connaît pas le fond de
    la page hôte), sans les images de fond, l'opacité des ancêtres ni les états d'interaction ; en
    mode sombre seulement si un token définit une valeur sombre.
19. Les cibles tactiles ne sont contrôlées que sur les dimensions explicites ; la taille réelle et le
    débordement horizontal relèvent de la mesure dans le navigateur (étapes c et f).

**Outillage**

20. `cargo xtask codegen` (§ 2) est livré avec son premier consommateur, `packages/ir-types` à
    l'étape (c). En (a), la génération TypeScript (ts-rs) et JSON Schema (schemars) est couverte par
    des tests, dont l'absence de récursion dans le schéma de `Command` (mode `strict`).

**Reste à faire de la revue de la PR #1 (constaté le 2026-10-07)**

21. La PR #1 a été fusionnée avant son dernier lot de corrections, « validation (schéma, ordre de
    rendu) » : 14 défauts confirmés par la revue, dont la perte du contenu des slots imbriqués dans
    l'ordre de rendu (`render_order`). Le détail des constats n'a pas été conservé. Aucun lot de
    correction n'a touché `validate/schema.rs`, `validate/mod.rs` ni `validate/contrast.rs` :
    ils sont à ré-auditer, puis à corriger avec des tests de non-régression, avant l'étape (b).
22. Ce § 14 n'a pas encore été complété avec les changements de comportement issus des quatre lots
    de corrections fusionnés (`ops-history`, `lower`, `style`, `validate-a11y-resp`).
