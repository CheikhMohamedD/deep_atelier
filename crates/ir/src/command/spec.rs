//! Spécifications d'entrée des commandes : champs optionnels → défauts sûrs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::document::BindableField;
use crate::id::{ComponentId, NodeRef, TokenName};
use crate::node::{
    Autocomplete, ButtonType, ContainerKind, ContainerRole, Dimensions, Href, ImageSource, ImportDecl, InputType,
    PropOverride, PropValue, TextRole, TextRun, VariantChoice,
};
use crate::style::color::Hex;
use crate::style::responsive::{ResponsivePatch, double_option};
use crate::style::style::{StatePatch, StylePatch};
use crate::style::values::{Radius, Shadow};
use crate::tokens::{FontFamily, ShadowLayer};

/// Nœud à créer. Liste PLATE (le mode `strict` des outils refuse les schémas récursifs) : la
/// hiérarchie s'exprime par `parent`, ce qui permet aussi le streaming nœud par nœud.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct NodeSpec {
    /// Référence locale (`$pricing-grid`) réutilisable plus loin dans la transaction ou le run.
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    #[ts(rename = "ref", optional)]
    pub r#ref: Option<String>,
    /// `ref` d'un `NodeSpec` précédent de la même commande ; absent ⇒ parent de la commande.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parent: Option<String>,
    pub kind: KindSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub style: Option<StylePatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub visibility: Option<ResponsivePatch<bool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hover: Option<StatePatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub focus_visible: Option<StatePatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<MetaPatch>,
}

impl NodeSpec {
    pub fn new(kind: KindSpec) -> Self {
        Self {
            r#ref: None,
            parent: None,
            kind,
            style: None,
            visibility: None,
            hover: None,
            focus_visible: None,
            meta: None,
        }
    }
}

/// `NodeKind` à champs optionnels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type")]
pub enum KindSpec {
    Box {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        role: Option<ContainerRole>,
    },
    /// Défaut explicite : `direction: { base: "column" }`.
    Stack {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        role: Option<ContainerRole>,
    },
    /// Défaut explicite : `columns: { base: "1" }`.
    Grid {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        role: Option<ContainerRole>,
    },
    Text {
        /// Défaut : `Paragraph`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        role: Option<TextRole>,
        #[serde(default)]
        content: Vec<TextRun>,
        /// Étiquette d'un champ (`$email` ou `n_…`) : impose le rôle `Label`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        for_input: Option<NodeRef>,
    },
    Image {
        /// Défaut : espace réservé paysage.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        source: Option<ImageSource>,
        #[serde(default)]
        alt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        intrinsic: Option<Dimensions>,
        #[serde(default)]
        priority: bool,
    },
    /// Décorative par défaut (`a11y.hidden`) sauf si `meta.a11y_label` est fourni.
    Icon { name: String },
    /// Défaut explicite : anneau de focus dans `states.focus_visible`.
    Button {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        button_type: Option<ButtonType>,
        #[serde(default)]
        disabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        action: Option<ActionSpec>,
    },
    Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        input_type: Option<InputType>,
        /// Défaut : dérivé du type (`email`, `text`…).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        placeholder: Option<String>,
        #[serde(default)]
        required: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        autocomplete: Option<Autocomplete>,
    },
    Link {
        href: Href,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        label: Option<String>,
        #[serde(default)]
        new_tab: bool,
    },
    ComponentInstance {
        component: ComponentId,
        #[serde(default)]
        overrides: Vec<PropOverride>,
        #[serde(default)]
        variants: Vec<VariantChoice>,
    },
    Slot {
        /// Défaut : `children`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
    },
    /// Jamais créé par l'IA.
    RawCode {
        code: String,
        #[serde(default)]
        imports: Vec<ImportDecl>,
        #[serde(default)]
        client: bool,
    },
}

/// Action dont la cible est une référence de commande (`$menu` ou `n_…`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind")]
pub enum ActionSpec {
    ToggleVisibility { target: NodeRef },
}

/// Patch des métadonnées et de l'accessibilité d'un nœud.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct MetaPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option::deserialize"
    )]
    #[ts(optional)]
    pub name: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option::deserialize"
    )]
    #[ts(optional)]
    pub anchor: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub locked: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option::deserialize"
    )]
    #[ts(optional)]
    pub slot: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option::deserialize"
    )]
    #[ts(optional)]
    pub a11y_label: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub a11y_hidden: Option<bool>,
}

/// Conteneur créé par `wrap_nodes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ContainerSpec {
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    #[ts(rename = "ref", optional)]
    pub r#ref: Option<String>,
    pub kind: ContainerKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub role: Option<ContainerRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub style: Option<StylePatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<MetaPatch>,
}

/// Prop de composant : champ d'un nœud du composant exposé à l'instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ComponentPropSpec {
    pub name: String,
    pub node: NodeRef,
    pub field: BindableField,
    /// Défaut : valeur actuelle du champ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub default: Option<PropValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ColorTokenValue {
    pub light: Hex,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dark: Option<Hex>,
}

/// Modification de tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind")]
pub enum TokenEdit {
    /// `value: null` = suppression (refusée si le token est utilisé).
    Color {
        name: TokenName,
        value: Option<ColorTokenValue>,
    },
    Font {
        name: TokenName,
        value: Option<FontFamily>,
    },
    Radius {
        step: Radius,
        px: u16,
    },
    Shadow {
        step: Shadow,
        layers: Vec<ShadowLayer>,
    },
    SpacingUnit {
        px: u8,
    },
}

/// Patch des propriétés propres à la primitive ; la variante doit correspondre au nœud.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type")]
pub enum PropsPatch {
    Box {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        role: Option<ContainerRole>,
    },
    Stack {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        role: Option<ContainerRole>,
    },
    Grid {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        role: Option<ContainerRole>,
    },
    Text {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        role: Option<TextRole>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        content: Option<Vec<TextRun>>,
    },
    Image {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        source: Option<ImageSource>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        alt: Option<String>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "double_option::deserialize"
        )]
        #[ts(optional)]
        intrinsic: Option<Option<Dimensions>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        priority: Option<bool>,
    },
    Icon {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
    },
    Button {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "double_option::deserialize"
        )]
        #[ts(optional)]
        label: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        button_type: Option<ButtonType>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        disabled: Option<bool>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "double_option::deserialize"
        )]
        #[ts(optional)]
        action: Option<Option<ActionSpec>>,
    },
    Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        input_type: Option<InputType>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "double_option::deserialize"
        )]
        #[ts(optional)]
        placeholder: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        required: Option<bool>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "double_option::deserialize"
        )]
        #[ts(optional)]
        autocomplete: Option<Option<Autocomplete>>,
    },
    Link {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        href: Option<Href>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "double_option::deserialize"
        )]
        #[ts(optional)]
        label: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        new_tab: Option<bool>,
    },
    ComponentInstance {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        overrides: Option<Vec<PropOverride>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        variants: Option<Vec<VariantChoice>>,
    },
    Slot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
    },
    RawCode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        imports: Option<Vec<ImportDecl>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        client: Option<bool>,
    },
}
