//! Commandes : API publique de mutation, utilisée telle quelle par l'UI, le parser et le LLM.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::command::spec::{ComponentPropSpec, ContainerSpec, MetaPatch, NodeSpec, PropsPatch, TokenEdit};
use crate::document::{Asset, RouteSegment, Seo, VariantAxis};
use crate::id::{AssetId, ComponentId, LayoutId, NodeId, NodeRef, PageId};
use crate::node::{ContainerKind, PlatformScope, WebOverrides};
use crate::op::OpError;
use crate::style::responsive::{ResponsivePatch, double_option};
use crate::style::style::{InteractionState, StylePatch};
use crate::validate::Issue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    // Structure
    /// Insère des nœuds (liste plate) dans `parent`, à `index` (défaut : à la fin).
    InsertNodes {
        parent: NodeRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        index: Option<u32>,
        nodes: Vec<NodeSpec>,
    },
    /// Remplace un nœud (et son sous-arbre) par de nouveaux nœuds, à la même place.
    ReplaceNode {
        node: NodeRef,
        nodes: Vec<NodeSpec>,
    },
    DeleteNodes {
        nodes: Vec<NodeRef>,
    },
    /// `index` : position finale parmi les enfants de `parent`.
    MoveNode {
        node: NodeRef,
        parent: NodeRef,
        index: u32,
    },
    /// Copie profonde insérée juste après l'original.
    DuplicateNode {
        node: NodeRef,
    },
    /// Enveloppe des frères contigus dans un nouveau conteneur.
    WrapNodes {
        nodes: Vec<NodeRef>,
        container: ContainerSpec,
    },
    /// Remplace un conteneur par ses enfants.
    UnwrapNode {
        node: NodeRef,
    },
    /// Box ⇄ Stack ⇄ Grid.
    ConvertContainer {
        node: NodeRef,
        to: ContainerKind,
    },
    // Contenu, style, méta
    SetProps {
        node: NodeRef,
        props: PropsPatch,
    },
    SetStyle {
        node: NodeRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        state: Option<InteractionState>,
        style: StylePatch,
    },
    /// `null` = toujours visible.
    SetVisibility {
        node: NodeRef,
        visibility: Option<ResponsivePatch<bool>>,
    },
    /// Nom, ancre, verrou, slot, a11y.
    SetMeta {
        node: NodeRef,
        meta: MetaPatch,
    },
    /// UI seulement.
    SetPlatform {
        node: NodeRef,
        scope: PlatformScope,
    },
    /// UI et parser seulement.
    SetWebOverrides {
        node: NodeRef,
        overrides: WebOverrides,
    },
    // Composants
    /// Transforme un nœud en composant et le remplace par une instance.
    CreateComponent {
        node: NodeRef,
        name: String,
        props: Vec<ComponentPropSpec>,
    },
    UpdateComponent {
        component: ComponentId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        props: Option<Vec<ComponentPropSpec>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        variants: Option<Vec<VariantAxis>>,
    },
    /// Remplace une instance par une copie éditable du composant.
    DetachInstance {
        node: NodeRef,
    },
    /// Refusée s'il reste des instances.
    DeleteComponent {
        component: ComponentId,
    },
    // Layouts et pages
    /// Racine + `Slot "page"`.
    CreateLayout {
        name: String,
    },
    UpdateLayout {
        layout: LayoutId,
        name: String,
    },
    /// Refusée si le layout est utilisé.
    DeleteLayout {
        layout: LayoutId,
    },
    CreatePage {
        name: String,
        route: Vec<crate::document::RouteSegment>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        layout: Option<LayoutId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        seo: Option<Seo>,
    },
    UpdatePage {
        page: PageId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        route: Option<Vec<RouteSegment>>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "double_option::deserialize"
        )]
        #[ts(optional)]
        layout: Option<Option<LayoutId>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        seo: Option<Seo>,
    },
    DeletePage {
        page: PageId,
    },
    // Tokens, site, assets
    SetToken {
        edit: TokenEdit,
    },
    UpdateSettings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        lang: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        site_name: Option<String>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "double_option::deserialize"
        )]
        #[ts(optional)]
        favicon: Option<Option<AssetId>>,
    },
    AddAsset {
        asset: Asset,
    },
    /// Refusée si l'asset est référencé.
    RemoveAsset {
        asset: AssetId,
    },
}

impl Command {
    /// Nom de l'outil LLM correspondant (`insert_nodes`).
    pub fn name(&self) -> &'static str {
        match self {
            Command::InsertNodes { .. } => "insert_nodes",
            Command::ReplaceNode { .. } => "replace_node",
            Command::DeleteNodes { .. } => "delete_nodes",
            Command::MoveNode { .. } => "move_node",
            Command::DuplicateNode { .. } => "duplicate_node",
            Command::WrapNodes { .. } => "wrap_nodes",
            Command::UnwrapNode { .. } => "unwrap_node",
            Command::ConvertContainer { .. } => "convert_container",
            Command::SetProps { .. } => "set_props",
            Command::SetStyle { .. } => "set_style",
            Command::SetVisibility { .. } => "set_visibility",
            Command::SetMeta { .. } => "set_meta",
            Command::SetPlatform { .. } => "set_platform",
            Command::SetWebOverrides { .. } => "set_web_overrides",
            Command::CreateComponent { .. } => "create_component",
            Command::UpdateComponent { .. } => "update_component",
            Command::DetachInstance { .. } => "detach_instance",
            Command::DeleteComponent { .. } => "delete_component",
            Command::CreateLayout { .. } => "create_layout",
            Command::UpdateLayout { .. } => "update_layout",
            Command::DeleteLayout { .. } => "delete_layout",
            Command::CreatePage { .. } => "create_page",
            Command::UpdatePage { .. } => "update_page",
            Command::DeletePage { .. } => "delete_page",
            Command::SetToken { .. } => "set_token",
            Command::UpdateSettings { .. } => "update_settings",
            Command::AddAsset { .. } => "add_asset",
            Command::RemoveAsset { .. } => "remove_asset",
        }
    }

    /// Commandes exposées à l'IA (ADR 0001 § 6). La création de `RawCode` est en plus refusée
    /// à l'abaissement.
    pub fn allowed_for_ai(&self) -> bool {
        !matches!(
            self,
            Command::SetPlatform { .. }
                | Command::SetWebOverrides { .. }
                | Command::DeleteComponent { .. }
                | Command::UpdateLayout { .. }
                | Command::DeleteLayout { .. }
                | Command::DeletePage { .. }
                | Command::UpdateSettings { .. }
                | Command::AddAsset { .. }
                | Command::RemoveAsset { .. }
        )
    }
}

/// Erreur d'une commande : rien n'est appliqué. `code` et `message` sont renvoyés au LLM.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CommandError {
    #[error("node `{0}` not found")]
    NodeNotFound(String),
    #[error("unknown local reference `{0}`")]
    UnknownRef(String),
    #[error("local reference `{0}` is already defined")]
    DuplicateRef(String),
    #[error("invalid local reference name `{0}`")]
    InvalidRef(String),
    #[error("{0} not found")]
    EntityNotFound(String),
    #[error("node `{parent}` ({kind}) cannot contain children")]
    InvalidParent { parent: String, kind: &'static str },
    #[error("node `{0}` is a page, layout or component root")]
    RootNode(NodeId),
    #[error("command expects a {expected} node, found {found}")]
    KindMismatch {
        expected: &'static str,
        found: &'static str,
    },
    #[error("{0}")]
    InvalidCommand(String),
    #[error("{0} is still in use")]
    InUse(String),
    #[error("command `{command}` is outside the allowed scope: {reason}")]
    OutOfScope { command: &'static str, reason: String },
    #[error("command `{0}` is not available to this origin")]
    Forbidden(String),
    #[error("an AI draft is in progress")]
    DraftInProgress,
    #[error("no AI draft in progress")]
    NoDraft,
    #[error("accepted change depends on rejected change: {0}")]
    UnitDependency(String),
    #[error("the result has {} blocking issue(s)", .0.len())]
    ValidationFailed(Vec<Issue>),
    #[error("command {index} (`{command}`): {source}")]
    InCommand {
        index: usize,
        command: &'static str,
        source: Box<CommandError>,
    },
    #[error(transparent)]
    Op(#[from] OpError),
}

impl CommandError {
    /// Code stable (SCREAMING_SNAKE_CASE) pour l'UI et le LLM.
    pub fn code(&self) -> &'static str {
        match self {
            CommandError::NodeNotFound(_) => "NODE_NOT_FOUND",
            CommandError::UnknownRef(_) => "UNKNOWN_REF",
            CommandError::DuplicateRef(_) => "DUPLICATE_REF",
            CommandError::InvalidRef(_) => "INVALID_REF",
            CommandError::EntityNotFound(_) => "ENTITY_NOT_FOUND",
            CommandError::InvalidParent { .. } => "INVALID_PARENT",
            CommandError::RootNode(_) => "ROOT_NODE",
            CommandError::KindMismatch { .. } => "KIND_MISMATCH",
            CommandError::InvalidCommand(_) => "INVALID_COMMAND",
            CommandError::InUse(_) => "IN_USE",
            CommandError::OutOfScope { .. } => "OUT_OF_SCOPE",
            CommandError::Forbidden(_) => "FORBIDDEN",
            CommandError::DraftInProgress => "DRAFT_IN_PROGRESS",
            CommandError::NoDraft => "NO_DRAFT",
            CommandError::UnitDependency(_) => "UNIT_DEPENDENCY",
            CommandError::ValidationFailed(_) => "VALIDATION_FAILED",
            CommandError::InCommand { source, .. } => source.code(),
            CommandError::Op(_) => "INVALID_OPERATION",
        }
    }

    /// Forme renvoyée au LLM : `{ "ok": false, "error": { "code", "message" } }`.
    pub fn to_tool_result(&self) -> serde_json::Value {
        serde_json::json!({ "ok": false, "error": { "code": self.code(), "message": self.to_string() } })
    }
}
