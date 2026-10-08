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
   - **Décision (2026-10-08) : nouveau projet, offre Free, région `eu-west-3` (Paris)**, dans
     une organisation dédiée « Deep Atelier », créée pour lui (choix du propriétaire, plutôt que
     son organisation existante). Limites acceptées pour développer le MVP : 500 Mo de base,
     1 Go de fichiers, pas de sauvegardes, pause après une semaine sans activité. Passage en Pro
     sans migration quand de vrais utilisateurs arrivent. Ni l'organisation ni l'id du projet
     n'apparaissent dans le dépôt.
10. Méthodes de connexion à l'éditeur (question ajoutée à l'étape b2 : la spec ne les précise pas).
    - **Décision (2026-10-08) : GitHub OAuth et lien magique par e-mail**, sans mot de passe. Le
      propriétaire crée une OAuth App GitHub (id et secret dans la configuration Supabase, jamais
      dans le dépôt). Le service e-mail par défaut de Supabase n'écrit qu'aux adresses autorisées
      (2 messages par heure) : un SMTP (Resend, Postmark…) sera configuré avant l'ouverture au
      public.

**Avant (c)**

7. Langue de l'interface de l'éditeur : français, anglais, ou i18n dès le départ ?
   - **Décision (2026-10-08) : français et anglais dès le départ.** Chaque texte de l'interface
     passe par un catalogue (un par langue) ; les deux sont tenus à jour à chaque PR et un test de
     CI vérifie qu'ils ont les mêmes clés. Les codes d'erreur de l'IR et de l'API se traduisent par
     ces catalogues.
8. Build de l'app exportée dans l'éditeur (onglet Terminal) en Phase 1 ? Recommandation : non,
   build seulement à l'export, dans un job serveur.
   - **Décision (2026-10-08) : non, build seulement à l'export.** Pas d'onglet Terminal en
     Phase 1 : le canvas interprète l'IR (§ 10, point 5) ; le vrai `next build` tourne à l'export
     (étape i), dans un job serveur sur le modèle du job CI `export`, et son rapport s'affiche.

**Avant (f)**

9. Prompt à partir d'une URL (multimodal) en Phase 1 ? Demande un service de capture headless.

---

## 13. Environnement de développement (décidé le 2026-10-06)

- **Dépôt** public : `github.com/CheikhMohamedD/deep_atelier`, sans licence pour l'instant (tous droits
  réservés). Commits signés de l'adresse noreply GitHub du propriétaire (voir `CLAUDE.md`).
- **Développement dans des sessions Claude Code cloud**, rien n'est installé sur le poste local.
  L'image cloud fournit rustc/cargo, Node 22 avec pnpm, Docker et PostgreSQL 16.
  - **Complément (2026-10-07)** : le développement se fait aussi en local (rustup avec la toolchain
    du dépôt, Node, pnpm). Une seule session, locale ou cloud, travaille sur une branche à la fois.
- **Outillage complémentaire** (cible `wasm32-unknown-unknown`, `wasm-bindgen-cli`, navigateurs
  Playwright) : ajouté au script de setup de l'environnement cloud à l'étape (c), pour être mis en
  cache. Les téléchargements de releases GitHub d'autres dépôts sont bloqués : `wasm-bindgen-cli`
  s'installe par `cargo install`, à la version exacte de la crate `wasm-bindgen`.
- **Supabase hébergé**, pas de Docker local. Le projet est créé à l'étape (b2).
  - **Complément (2026-10-08)** : Docker et la CLI Supabase sont installés sur le poste local ; le
    stack Supabase local sert au développement et aux tests de l'API (§ 16), le projet hébergé
    reste la cible. L'accès réseau
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
18. Le contraste est calculé dans les ancêtres rendus du texte (point 22), sans les images de fond,
    l'opacité des ancêtres ni les états d'interaction ; en mode sombre seulement si un token définit
    une valeur sombre.
19. Les cibles tactiles ne sont contrôlées que sur les dimensions explicites ; la taille réelle et le
    débordement horizontal relèvent de la mesure dans le navigateur (étapes c et f).

**Outillage**

20. `cargo xtask codegen` (§ 2) est livré avec son premier consommateur, `packages/ir-types` à
    l'étape (c). En (a), la génération TypeScript (ts-rs) et JSON Schema (schemars) est couverte par
    des tests, dont l'absence de récursion dans le schéma de `Command` (mode `strict`).

**Revue de la PR #1 : changements de comportement (complété le 2026-10-07)**

La PR #1 a été fusionnée avant son dernier lot de corrections, « validation (schéma, ordre de
rendu) », dont le détail n'avait pas été conservé : la validation a été ré-auditée, et chaque défaut
confirmé est couvert par `crates/ir/tests/regressions_validate_schema_render.rs`.

21. **Rendu des pages** (`crates/ir/src/render.rs`, partagé par la validation et les commandes) :
    `RenderTree::of_page` rend le layout, la page à la place de son slot `page` (une seule fois, et
    seulement le slot écrit dans le layout), chaque instance développée, et le contenu d'un slot à
    la place du slot, dans le contexte où l'instance est écrite. Corrigé : contenu des slots
    imbriqués perdu, instance d'un composant dans le contenu du slot d'une instance du même
    composant ignorée, longues pages tronquées, slot `page` d'un composant pris pour celui du
    layout. Un nœud lié à une prop `Visible` qui vaut `false` dans une instance n'y est pas rendu.
22. **Règles jugées au rendu.** Un nœud rendu est jugé à chacun de ses rendus ; un nœud jamais
    rendu (composant sans instance, layout sans page) dans l'arbre où il est écrit. Le problème est
    signalé sur le nœud placé dans l'élément parent (l'instance qui rend une racine de composant).
    - Placement (`col_span`, `grow`, `shrink`, `align_self`, `order`) : permis selon les hôtes au
      rendu, exactement comme les commandes (le contenu d'un slot n'est plus refusé à tort).
    - Listes : un élément de liste est jugé à sa place au rendu ; une liste ne contient que des
      éléments de liste ou du code libre (nouveau code `LIST_CHILD_NOT_ITEM`, exigé par l'audit
      Lighthouse « list »).
    - Éléments interactifs imbriqués à travers les composants et les slots.
    - Ancres : celles d'un composant comptent pour la page qui le rend (liens valides, doublons
      signalés) ; un lien vers une ancre est vérifié sur chaque page qui le rend, avec le lien
      effectif (surcharge ou défaut d'une prop `Href`).
    - Contraste : fond, couleur et taille du texte viennent des ancêtres rendus (le composant qui
      accueille un contenu de slot, la page qui accueille une instance ; une instance porte le
      style de la racine de son composant).
    - Accessibilité : alt, libellés et contenu nommant liés à des props sont jugés avec la valeur
      de chaque instance ; une surcharge vide est signalée sur l'instance.
    - Taille de texte d'un champ : héritée des ancêtres rendus.
23. **Règles des commandes reprises par la validation** (un document chargé, ou un brouillon IA,
    ne peut plus les contourner) :
    - surcharges de variantes : propriétés permises selon le type et les hôtes au rendu,
      propriétés d'état seulement dans un état, `none` réservé aux tailles max, tokens existants ;
    - noms des props et des axes de variantes en camelCase ; options non vides et uniques.
24. **Nouvelles règles de schéma.**
    - Props, axes de variantes et slots deviennent des props TypeScript : noms camelCase, uniques
      entre eux, et pas `children` (sauf le slot par défaut), `className`, `key` ni `ref` ;
      un champ n'est lié qu'à une prop. Le nom de slot `page` est réservé aux layouts, qui
      n'ont pas d'autre slot. La commande d'insertion refuse un nom de slot non camelCase.
    - Une instance ne choisit qu'une option par axe de variantes.
    - Routes : un paramètre n'apparaît qu'une fois par route, et deux routes nomment pareil le
      paramètre qu'elles placent au même endroit de l'arborescence (contrainte de l'App Router).
    - Images : URL http(s) absolue, asset de type image, dimensions intrinsèques positives ; le
      favicon est une image, l'image OG un PNG, JPEG, GIF ou WebP. Les valeurs de props (défauts et
      surcharges) suivent les règles du champ qu'elles remplissent.
    - Les racines de page et de layout sont des conteneurs (nouveau code `ROOT_NOT_CONTAINER`).
    - Un nœud d'un cycle de parents n'est plus signalé en plus comme orphelin.
25. **Lots fusionnés dans la PR #1** (tests `regressions_*.rs`) :
    - `ops-history` : l'acceptation partielle d'un brouillon IA refuse une unité qui embarque une
      valeur (entière, copiée, héritée) ou une entité d'une unité rejetée, ou dont le retrait
      emporterait des nœuds qu'une unité rejetée avait supprimés ou déplacés ; une erreur déjà
      présente avant le brouillon ne le bloque pas ; `InsertSubtree` refuse un sous-arbre
      incohérent sans rien modifier.
    - `lower` : une commande ne peut pas faire sortir de la sélection ; l'IA ne crée pas de
      `RawCode` par duplication ni par détachement ; un nœud déplacé, enveloppé ou désenveloppé
      perd le placement et le slot que son nouvel hôte n'admet pas ; le détachement garde slot,
      a11y, verrou, plateforme et échappatoires web ; l'extraction d'un composant refuse ce dont
      l'arbre d'origine dépend (slots, références qui franchissent sa frontière, cibles de props
      ou de variantes) ; une page liée depuis ailleurs ne peut pas être supprimée ; une prop
      `Visible` détachée à `true` affiche le nœud.
    - `style` : effacer une surcharge absente ne crée pas la propriété ; `null` est refusé pour
      `base` ; schémas des valeurs alignés sur leurs parseurs (noms de tokens réservés annoncés).
    - `validate-a11y-resp` : les corrections responsive ne touchent que le mobile ; une cible
      tactile se juge sur sa taille et ses minimums ; `a11y.hidden` masque tout le rendu du nœud ;
      l'alt d'une image nomme son bouton ou son lien ; une instance ne nomme un bouton que par son
      contenu rendu.

---

## 15. Étape (b) : compilateur web (2026-10-08)

`crates/compiler-web` : `compile(&Document, Mode) -> Project` produit un projet Next.js 16 complet
(App Router, TypeScript strict, Tailwind v4) qui ne dépend pas de Deep Atelier. Modules : `plan`
(noms, îlots clients, assets, hôtes d'images), `elements` (nœuds → JSX), `classes` (style →
classes), `theme` (tokens → CSS), `project` (assemblage et gabarit), `doc`, `jsx` et `module`
(impression façon Prettier), `names`, `demo` (documents de démonstration). Ils remplacent la liste
de fichiers prévue au § 2 ; `tools/export-check/` s'ajoute à l'arborescence (point 12).

**Projet exporté**

1. Arborescence :
   - `app/layout.tsx` : `<html lang>`, polices `next/font/google`, titre et favicon du site ;
     `app/globals.css` (point 6) ; `app/icon.svg` quand aucun favicon n'est choisi (initiale du
     site sur `primary`, pour éviter un `favicon.ico` en 404) ;
   - un groupe de routes par layout de l'IR : `app/(<layout>)/layout.tsx`, où le slot `page`
     devient `{children}` ; chaque page sous son groupe (`app/(<layout>)/<route>/page.tsx`,
     segments dynamiques `[slug]`), avec ses métadonnées (titre, description, image OG) ;
   - `components/<Nom>.tsx` par composant ou îlot client (point 4), `components/raw/<Nom>.tsx` par
     bloc de code libre client ;
   - `public/assets/<id>-<fichier>` (contenu copié depuis le stockage par l'export, étape i) et
     `public/placeholders/*.svg` (images de remplissage) ;
   - configuration du gabarit `create-next-app` 16.4 : `package.json` aux versions exactes (`next`,
     `react`, `react-dom`, et `lucide-react` 1.52.0 comme le catalogue de l'IR ; `lucide-react` et
     `tailwind-merge` seulement s'ils servent), `tsconfig.json`, `eslint.config.mjs`,
     `next.config.ts` (hôtes des images distantes dans `images.remotePatterns`),
     `pnpm-workspace.yaml`, `.gitignore`, `AGENTS.md`, `README.md`. Pas de `pnpm-lock.yaml` : il
     est créé au premier `pnpm install`.
2. Mise en forme : le code sort tel que Prettier 3.9.9 (réglages par défaut) l'écrirait.
   L'imprimeur de documents de Prettier est porté en Rust (`doc.rs` : groupes, `fill`,
   `conditionalGroup`, propagation des sauts, largeur 80 en colonnes Unicode), avec ses règles JSX,
   d'objets, de tableaux et d'imports (`jsx.rs`, `module.rs`). Seul le code libre est repris tel
   qu'écrit, réindenté.
3. Composants : une fonction nommée par composant, props typées (`type <Nom>Props`) et défauts
   dans la déstructuration ; variantes : union de littéraux et une table de classes par nœud visé
   (`const toneClasses = {…} as const`) ; slots : props `ReactNode` (`children` pour le slot par
   défaut) ; prop `Visible` : `if (!show) return null` sur la racine, `{show && …}` ailleurs. Selon
   les instances, le composant accepte aussi :
   - `className` (instance stylée, ou variantes sur la racine), fusionné par `twMerge`
     (décision 5) ;
   - `id` (instance ancrée), posé sur la racine ;
   - `...rest`, typé `AriaAttributes` et étalé sur la racine, quand une instance porte une
     étiquette, un masquage ou des attributs `data-*` / `aria-*` : ils s'appliquent à la racine
     rendue, comme le supposent la validation et le détachement d'instance.
4. Interactivité : un bouton `toggle` produit `useState`, `aria-expanded`, `aria-controls` (ancre
   de la cible ou `useId`) et `onClick` ; sa cible porte `data-open` et la variante
   `data-[open=true]:<affichage>`, qui l'affiche là où elle est masquée fermée. Dans une page ou un
   layout, seul le plus petit sous-arbre qui contient les boutons et leurs cibles devient client :
   un îlot (`components/Menu.tsx`, nommé d'après son rôle), le reste de la page reste rendu côté
   serveur ; un îlot qui contient le slot `page` reçoit `children`. Un composant qui contient une
   bascule est client en entier.
5. Classes : base, puis `sm:` à `2xl:`, puis états ; dans chaque bloc, ordre fixe par famille
   (affichage, disposition, placement, position, dimensions, marges, typographie, apparence,
   transitions) ; côtés regroupés (`p-4`, `px-4 py-2`). Une base égale à la valeur initiale CSS
   d'une propriété non héritée n'est pas émise (§ 14, point 3) ; une propriété héritée l'est
   toujours, car un composant ou un contenu de slot n'hérite pas, au rendu, des ancêtres où il est
   écrit.
6. Thème : couleurs en variables CSS sur `:root`, valeurs sombres sous
   `@media (prefers-color-scheme: dark)` (décision 4), exposées par `@theme inline` (`bg-primary`,
   `font-display`) ; crans de rayon, d'ombre et d'espacement dans `@theme` seulement s'ils
   diffèrent de ceux de Tailwind.
7. Éléments : balise du rôle de chaque nœud ; `next/link` pour les liens internes, `<a>` pour les
   autres (`rel="noopener noreferrer"` en nouvel onglet) ; `next/image` avec les dimensions
   intrinsèques, `preload` pour l'image prioritaire (`priority` est déprécié en Next 16),
   `unoptimized` pour un SVG ; icônes `lucide-react` (`aria-hidden`, ou `role="img"` et
   `aria-label`) ; champ lié à son étiquette par `htmlFor` (id fixe dans une page, `useId` dans un
   composant ou un îlot).
8. Code libre : un `RawCode` serveur est inséré tel quel avec ses imports ; un `RawCode` client
   devient `components/raw/<Nom>.tsx` (`"use client"`). Il doit suivre les règles de Next 16 avec
   `cacheComponents` (par exemple, pas de `new Date()` au rendu hors d'un `Suspense`) : le
   compilateur ne l'analyse pas avant l'étape (h).

**Mode édition**

9. `Mode::Edit` pose `data-atl-id` sur chaque élément, et chaque fichier donne la plage exacte de
   l'élément de chaque nœud (`File::source_map`, en octets), pour le lien code ⇄ canvas des étapes
   (g) et (h). `Mode::Export` ne laisse aucune trace de l'éditeur. Le canvas reste un interprète de
   l'IR (§ 10, point 5) : il réutilisera `node_classes` et `RenderTree` à l'étape (c).

**Validation**

10. Contraste : chaque rendu applique les surcharges de la variante choisie par l'instance
    (`render::variant_style`), comme `twMerge` à l'export. Le défaut a été révélé par Lighthouse
    sur la landing de démonstration (description grise sur l'offre mise en avant) ; test
    `crates/ir/tests/variant_contrast.rs`.

**Vérification**

11. Snapshots `insta` de chaque fichier de la landing de démonstration (`demo::landing`) et d'un
    document « tout-en-un » (`demo::kitchen_sink` : slots nommés, variantes sur la racine et sur un
    descendant, gardes `Visible`, composants client, props d'image et de lien, attributs transmis,
    route dynamique, police Google, assets, code libre, nœud natif seul, échappatoires web).
    S'y ajoutent la compilation déterministe et le mode édition (chaque nœud rendu a sa plage, et
    chaque plage son `data-atl-id`).
12. Job CI `export` : les tests écrivent les deux projets sur disque, puis `prettier --check`,
    `pnpm build`, `pnpm lint`, et `tools/export-check/check.mjs` sur `next start` :
    - aucun défilement horizontal à 390, 768 et 1280 px, en clair et en sombre ;
    - aucune exception ni erreur d'hydratation (ni aucune erreur de console pour la landing) ;
    - menu burger ouvert puis refermé à 390 px, remplacé par la navigation à 1280 px ;
    - Lighthouse mobile ≥ 90 dans les quatre catégories, en clair et en sombre (landing de
      démonstration : 99, 100, 100 et 100 en local).

**Limites connues**

13. `metadataBase` n'est pas défini (l'IR ne connaît pas l'URL du site) : Next l'avertit au build,
    et l'URL de l'image OG est résolue sur `localhost`. À traiter avec le domaine de publication
    (étape i).
14. Les images de remplissage ne suivent pas le thème sombre.
15. Une étiquette posée sur un nœud rendu en élément générique (`div`, `span`) ou en paragraphe est
    émise telle quelle, alors qu'ARIA 1.2 interdit de nommer ces rôles (audit Lighthouse
    `aria-prohibited-attr`). La validation de l'IR ne le signale pas encore.

---

## 16. Étape (b2) : API et Supabase (2026-10-08)

**Projet hébergé.** Organisation « Deep Atelier », projet `deep-atelier`, offre Free, région
`eu-west-3` (décision 6), créés avec la CLI Supabase. Le mot de passe de la base est généré sur le
poste et rangé dans `.env` (non versionné) ; le schéma est appliqué par `supabase db push`. Ni
l'organisation ni le projet ne sont identifiés dans le dépôt.

**Base** (`supabase/migrations/…_projects.sql`)

1. `projects`, `project_documents` (IR en `jsonb`, `version` pour la concurrence optimiste,
   `ir_version`) et `document_versions` (numérotées par projet ; `origin` `user | ai | restore` ;
   message obligatoire, sauf pour une version `restore`, qui porte `restored_from`). `assets`,
   `ai_runs`, `plans` et `usage_monthly` (§ 9) arriveront avec les étapes qui s'en servent.
2. Droits : l'API, avec le rôle propriétaire des tables, est le seul écrivain. Le rôle
   `authenticated` lit seulement ses propres lignes (RLS, `(select auth.uid())`), `anon` n'a aucun
   droit. Vérifié de bout en bout par PostgREST avec de vrais jetons.

**API** (`crates/api`, Axum ; routes dans `src/routes/mod.rs`)

3. Authentification : l'API vérifie le jeton d'accès Supabase avec le JWKS du projet. Les
   projets récents signent en ES256 (vérifié sur le projet hébergé) ; RS256 est accepté, HS256
   refusé. Contrôles : émetteur `<SUPABASE_URL>/auth/v1`, audience et rôle `authenticated`,
   expiration (30 s de tolérance). Les clés restent 10 minutes en cache et sont rechargées pour un
   `kid` inconnu, au plus une fois toutes les 30 secondes. Des clés injoignables donnent 503, pas
   401, pour ne pas déconnecter l'éditeur.
4. Sauvegarde automatique : le client envoie tout le document avec `base_version`. Si un autre
   onglet a enregistré entre-temps, la réponse est 409 `VERSION_CONFLICT` avec `current_version`.
   Un verrou sur la ligne du projet sérialise ses écritures (deux sauvegardes simultanées : une
   seule passe).
5. Contrôle du document : migration (`ir::migrate`), puis intégrité (`IssueCode::is_integrity` :
   version, cible web, page, identifiants, arbre, références, composant récursif). Seuls ces
   problèmes bloquent (422 `INVALID_DOCUMENT`, avec les problèmes). L'accessibilité ou la qualité
   n'empêchent jamais d'enregistrer un travail en cours. Le document est stocké sous sa forme
   normalisée, et le contrôle tourne hors du runtime (`spawn_blocking`).
6. Versions : instantané du document courant, avec un message et un nom facultatif. La
   restauration demande `base_version` et garde d'abord l'état courant comme version `restore` :
   rien n'est perdu. L'interface compose le libellé d'une version `restore`, donc aucun texte
   d'interface n'est stocké en base.
7. Réponses d'erreur `{ "error": { "code", "message" } }`, en anglais, avec des codes stables.
   Le projet d'un autre utilisateur répond 404, ce qui ne révèle pas son existence. Corps limité
   à 10 Mio ; CORS limité aux origines de l'éditeur ; une panique donne 500.

**Développement et tests**

8. Stack Supabase local (CLI, Docker) avec une clé ES256 générée une fois par poste, non
   versionnée : les jetons locaux ont la même forme que ceux du projet hébergé. En local, le lien
   magique arrive dans Mailpit ; GitHub demande une OAuth App de développement
   (`supabase/config.toml`).
9. Tests d'intégration, ignorés par défaut, contre le stack local et avec de vrais jetons de
   Supabase Auth : isolation entre utilisateurs, concurrence optimiste, documents refusés ou
   acceptés, versions et restauration, droits directs via PostgREST. Le job CI `api` les lance.

**Limites connues**

10. Pas d'écran de connexion avant l'éditeur (étape c, après la question 7). Sur le projet
    hébergé, la connexion GitHub attend l'OAuth App du propriétaire, et les URL de redirection
    attendent l'adresse de l'éditeur.
11. Aucun test ne tourne encore avec un jeton du projet hébergé, faute d'écran pour l'obtenir ;
    son JWKS est en ES256, comme le stack local.
12. L'API n'est pas déployée. Sur la base hébergée, elle passe par le pooler en mode session avec
    `sslmode=require` : la connexion est chiffrée mais le certificat n'est pas vérifié. La
    vérification (autorité de certification de Supabase) est à ajouter au déploiement.
13. Chaque sauvegarde envoie et enregistre le document entier, sans différentiel : suffisant pour
    le MVP, à revoir pour les gros documents.
