//! Nœuds de l'arbre : primitives sémantiques, sans concept DOM/CSS.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::{AssetId, ComponentId, NodeId, PageId};
use crate::style::color::ColorRef;
use crate::style::responsive::Responsive;
use crate::style::style::{StateStyles, Style};
use crate::style::values::AspectRatio;

/// Nœud de l'arène du document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Node {
    pub id: NodeId,
    /// `None` ⇔ racine de page, de layout ou de composant.
    pub parent: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<NodeId>>", optional)]
    pub children: Vec<NodeId>,
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Style::is_empty")]
    #[ts(as = "Option<Style>", optional)]
    pub style: Style,
    /// hover / focus-visible / active (WebOnly).
    #[serde(default, skip_serializing_if = "StateStyles::is_empty")]
    #[ts(as = "Option<StateStyles>", optional)]
    pub states: StateStyles,
    /// Visibilité par breakpoint (`hidden md:flex`) ; absent = toujours visible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub visibility: Option<Responsive<bool>>,
    #[serde(default, skip_serializing_if = "PlatformScope::is_all")]
    #[ts(as = "Option<PlatformScope>", optional)]
    pub platform: PlatformScope,
    #[serde(default)]
    pub meta: NodeMeta,
    #[serde(default, skip_serializing_if = "A11y::is_default")]
    #[ts(as = "Option<A11y>", optional)]
    pub a11y: A11y,
    #[serde(default, skip_serializing_if = "PlatformOverrides::is_empty")]
    #[ts(as = "Option<PlatformOverrides>", optional)]
    pub platform_overrides: PlatformOverrides,
}

impl Node {
    /// Nœud sans style ni méta.
    pub fn new(id: NodeId, parent: Option<NodeId>, kind: NodeKind) -> Self {
        Self {
            id,
            parent,
            children: Vec::new(),
            kind,
            style: Style::default(),
            states: StateStyles::default(),
            visibility: None,
            platform: PlatformScope::All,
            meta: NodeMeta::default(),
            a11y: A11y::default(),
            platform_overrides: PlatformOverrides::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub enum PlatformScope {
    #[default]
    All,
    WebOnly,
    NativeOnly,
}

impl PlatformScope {
    pub fn is_all(&self) -> bool {
        *self == PlatformScope::All
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub enum NodeSource {
    #[default]
    Visual,
    Code,
    Ai,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct NodeMeta {
    /// Nom du calque.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// Ancre de navigation (`pricing` → `#pricing`), unique par page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub source: NodeSource,
    /// Enfant d'instance : slot ciblé (`None` = `children`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub slot: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct A11y {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    #[serde(default)]
    pub hidden: bool,
}

impl A11y {
    pub fn is_default(&self) -> bool {
        *self == A11y::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type")]
pub enum NodeKind {
    /// Conteneur en flux.
    Box(ContainerProps),
    /// Conteneur flex 1D.
    Stack(ContainerProps),
    /// Grille ; en natif, compilée en lignes de `Stack`.
    Grid(ContainerProps),
    Text(TextProps),
    Image(ImageProps),
    Icon(IconProps),
    Button(ButtonProps),
    Input(InputProps),
    Link(LinkProps),
    ComponentInstance(InstanceProps),
    /// Seulement dans un composant ou un layout.
    Slot(SlotProps),
    RawCode(RawCodeProps),
}

/// Type de conteneur (`Box` ⇄ `Stack` ⇄ `Grid`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub enum ContainerKind {
    Box,
    Stack,
    Grid,
}

impl NodeKind {
    /// Nom de la primitive (`"Stack"`).
    pub fn type_name(&self) -> &'static str {
        match self {
            NodeKind::Box(_) => "Box",
            NodeKind::Stack(_) => "Stack",
            NodeKind::Grid(_) => "Grid",
            NodeKind::Text(_) => "Text",
            NodeKind::Image(_) => "Image",
            NodeKind::Icon(_) => "Icon",
            NodeKind::Button(_) => "Button",
            NodeKind::Input(_) => "Input",
            NodeKind::Link(_) => "Link",
            NodeKind::ComponentInstance(_) => "ComponentInstance",
            NodeKind::Slot(_) => "Slot",
            NodeKind::RawCode(_) => "RawCode",
        }
    }

    pub fn container(&self) -> Option<(ContainerKind, &ContainerProps)> {
        match self {
            NodeKind::Box(props) => Some((ContainerKind::Box, props)),
            NodeKind::Stack(props) => Some((ContainerKind::Stack, props)),
            NodeKind::Grid(props) => Some((ContainerKind::Grid, props)),
            _ => None,
        }
    }

    pub fn from_container(kind: ContainerKind, props: ContainerProps) -> NodeKind {
        match kind {
            ContainerKind::Box => NodeKind::Box(props),
            ContainerKind::Stack => NodeKind::Stack(props),
            ContainerKind::Grid => NodeKind::Grid(props),
        }
    }

    /// Vrai si le nœud peut recevoir des enfants.
    pub fn accepts_children(&self) -> bool {
        match self {
            NodeKind::Box(_) | NodeKind::Stack(_) | NodeKind::Grid(_) | NodeKind::ComponentInstance(_) => true,
            NodeKind::Button(props) => props.label.is_none(),
            NodeKind::Link(props) => props.label.is_none(),
            _ => false,
        }
    }

    /// Élément interactif (ne peut pas en contenir un autre).
    pub fn is_interactive(&self) -> bool {
        matches!(self, NodeKind::Button(_) | NodeKind::Link(_) | NodeKind::Input(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct ContainerProps {
    pub role: ContainerRole,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind")]
pub enum ContainerRole {
    #[default]
    Generic,
    Section,
    Header,
    Footer,
    Nav,
    Main,
    Article,
    Aside,
    List,
    ListItem,
    Form {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        action: Option<String>,
        method: FormMethod,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FormMethod {
    Get,
    Post,
}

/// Texte : suite de segments ; `"\n"` dans un segment = saut de ligne.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct TextProps {
    pub role: TextRole,
    pub content: Vec<TextRun>,
}

impl TextProps {
    /// Texte brut concaténé.
    pub fn plain_text(&self) -> String {
        self.content.iter().map(|run| run.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind")]
pub enum TextRole {
    Heading {
        level: HeadingLevel,
    },
    Paragraph,
    Inline,
    Caption,
    Quote,
    Code,
    Label {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        for_input: Option<NodeId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HeadingLevel {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl HeadingLevel {
    pub fn number(self) -> u8 {
        match self {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct TextRun {
    pub text: String,
    #[serde(default, skip_serializing_if = "is_false")]
    #[ts(as = "Option<bool>", optional)]
    pub strong: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    #[ts(as = "Option<bool>", optional)]
    pub em: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    #[ts(as = "Option<bool>", optional)]
    pub code: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub color: Option<ColorRef>,
}

impl TextRun {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct ImageProps {
    pub source: ImageSource,
    pub alt: String,
    /// Obligatoire pour `Asset` et `Url` (pas de décalage de mise en page, `next/image`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub intrinsic: Option<Dimensions>,
    /// Image LCP.
    #[serde(default)]
    pub priority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind")]
pub enum ImageSource {
    Asset {
        id: AssetId,
    },
    Url {
        url: String,
    },
    /// Rendu en SVG local à l'export.
    Placeholder {
        label: String,
        ratio: AspectRatio,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct IconProps {
    /// Nom lucide en kebab-case, validé contre le catalogue.
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct ButtonProps {
    /// `None` ⇒ enfants (Icon + Text) ; `a11y.label` requis si icône seule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    pub button_type: ButtonType,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub action: Option<Action>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ButtonType {
    #[default]
    Button,
    Submit,
    Reset,
}

/// Action déclarative ; côté web, compilée en composant client minimal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind")]
pub enum Action {
    /// Menu burger : bascule la visibilité de la cible.
    ToggleVisibility { target: NodeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct InputProps {
    pub input_type: InputType,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub autocomplete: Option<Autocomplete>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    #[default]
    Text,
    Email,
    Password,
    Number,
    Tel,
    Url,
    Search,
    Multiline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Autocomplete {
    Off,
    Name,
    Email,
    Tel,
    Organization,
    StreetAddress,
    PostalCode,
    Country,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct LinkProps {
    pub href: Href,
    /// `Some` ⇒ feuille ; `None` ⇒ conteneur.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    #[serde(default)]
    pub new_tab: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind")]
pub enum Href {
    /// Résiste au renommage de route.
    Page {
        page: PageId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        anchor: Option<String>,
    },
    Anchor {
        anchor: String,
    },
    External {
        url: String,
    },
    Email {
        address: String,
    },
    Phone {
        number: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct InstanceProps {
    pub component: ComponentId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<PropOverride>>", optional)]
    pub overrides: Vec<PropOverride>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<VariantChoice>>", optional)]
    pub variants: Vec<VariantChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct PropOverride {
    pub prop: String,
    pub value: PropValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct VariantChoice {
    pub axis: String,
    pub option: String,
}

/// Valeur d'une prop de composant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind", content = "value")]
pub enum PropValue {
    Text(String),
    Bool(bool),
    Href(Href),
    Image(ImageSource),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct SlotProps {
    /// `children` | `page` (layout) | nom libre.
    pub name: String,
}

/// Nom du slot par défaut d'un composant.
pub const DEFAULT_SLOT: &str = "children";
/// Nom du slot de page d'un layout.
pub const PAGE_SLOT: &str = "page";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct RawCodeProps {
    /// JSX conservé à l'identique, jamais réécrit.
    pub code: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<ImportDecl>>", optional)]
    pub imports: Vec<ImportDecl>,
    /// `"use client"` → extrait en composant client.
    #[serde(default)]
    pub client: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct ImportDecl {
    pub module: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub named: Vec<String>,
}

/// Échappatoires propres à une plateforme, isolées du cœur de l'IR.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct PlatformOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub web: Option<WebOverrides>,
}

impl PlatformOverrides {
    pub fn is_empty(&self) -> bool {
        self.web.as_ref().is_none_or(WebOverrides::is_empty)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct WebOverrides {
    /// Classes Tailwind non modélisées, conservées telles quelles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub extra_classes: Vec<String>,
    /// Attributs `data-*` / `aria-*` non modélisés (jamais `on*`, `style`, `className`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<Attribute>>", optional)]
    pub extra_attributes: Vec<Attribute>,
}

impl WebOverrides {
    pub fn is_empty(&self) -> bool {
        self.extra_classes.is_empty() && self.extra_attributes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}
