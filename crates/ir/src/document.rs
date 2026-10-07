//! Document : pages, layouts, composants, tokens et arène de nœuds.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::{AssetId, ComponentId, IdGen, LayoutId, NodeId, PageId};
use crate::node::{ContainerProps, ContainerRole, Href, ImageSource, Node, NodeKind, PropValue};
use crate::style::style::{StateStylesPatch, StylePatch};
use crate::tokens::DesignTokens;

/// Version courante du schéma de l'IR.
pub const IR_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct Document {
    pub version: u32,
    pub name: String,
    /// `[Web]` en v1 ; activer `Native` = ajouter un compilateur.
    pub targets: Vec<Target>,
    pub settings: SiteSettings,
    pub tokens: DesignTokens,
    pub layouts: Vec<Layout>,
    /// Ordre = ordre de navigation.
    pub pages: Vec<Page>,
    pub components: Vec<Component>,
    /// Arène unique : nœuds des pages, layouts et composants.
    pub nodes: BTreeMap<NodeId, Node>,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub enum Target {
    Web,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct SiteSettings {
    pub lang: String,
    pub site_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub favicon: Option<AssetId>,
}

/// Layout partagé → `app/(<nom>)/layout.tsx`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Layout {
    pub id: LayoutId,
    pub name: String,
    /// Contient exactement un `Slot { name: "page" }`.
    pub root: NodeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Page {
    pub id: PageId,
    pub name: String,
    /// `[]` = `/` ; graphe de routes → App Router / Expo Router.
    pub route: Vec<RouteSegment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub layout: Option<LayoutId>,
    /// Conteneur racine ; sans style ni rôle → fragment.
    pub root: NodeId,
    pub seo: Seo,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind", content = "name")]
pub enum RouteSegment {
    /// `blog`
    Static(String),
    /// `[slug]`
    Param(String),
}

/// Chemin d'une route (`/blog/[slug]`).
pub fn route_path(route: &[RouteSegment]) -> String {
    if route.is_empty() {
        return "/".to_owned();
    }
    route
        .iter()
        .map(|segment| match segment {
            RouteSegment::Static(name) => format!("/{name}"),
            RouteSegment::Param(name) => format!("/[{name}]"),
        })
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Seo {
    pub title: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub og_image: Option<AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Component {
    pub id: ComponentId,
    /// PascalCase unique → `components/<Name>.tsx`.
    pub name: String,
    pub root: NodeId,
    #[serde(default)]
    pub props: Vec<ComponentProp>,
    #[serde(default)]
    pub variants: Vec<VariantAxis>,
}

impl Component {
    pub fn prop(&self, name: &str) -> Option<&ComponentProp> {
        self.props.iter().find(|p| p.name == name)
    }

    pub fn axis(&self, name: &str) -> Option<&VariantAxis> {
        self.variants.iter().find(|a| a.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct ComponentProp {
    pub name: String,
    pub default: PropValue,
    pub binding: PropBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct PropBinding {
    pub node: NodeId,
    pub field: BindableField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub enum BindableField {
    Text,
    ImageSource,
    ImageAlt,
    Href,
    Label,
    Visible,
}

impl BindableField {
    /// Vrai si une valeur de prop a le bon type pour ce champ.
    pub fn accepts(self, value: &PropValue) -> bool {
        matches!(
            (self, value),
            (
                BindableField::Text | BindableField::ImageAlt | BindableField::Label,
                PropValue::Text(_)
            ) | (BindableField::ImageSource, PropValue::Image(_))
                | (BindableField::Href, PropValue::Href(_))
                | (BindableField::Visible, PropValue::Bool(_))
        )
    }

    /// Valeur actuelle du champ sur un nœud, si le nœud porte ce champ.
    pub fn read(self, node: &Node) -> Option<PropValue> {
        match (self, &node.kind) {
            (BindableField::Text, NodeKind::Text(text)) => Some(PropValue::Text(text.plain_text())),
            (BindableField::ImageSource, NodeKind::Image(image)) => Some(PropValue::Image(image.source.clone())),
            (BindableField::ImageAlt, NodeKind::Image(image)) => Some(PropValue::Text(image.alt.clone())),
            (BindableField::Href, NodeKind::Link(link)) => Some(PropValue::Href(link.href.clone())),
            (BindableField::Label, NodeKind::Button(button)) => button.label.clone().map(PropValue::Text),
            (BindableField::Label, NodeKind::Link(link)) => link.label.clone().map(PropValue::Text),
            (BindableField::Visible, _) => Some(PropValue::Bool(
                node.visibility
                    .as_ref()
                    .is_none_or(|v| v.values().any(|visible| *visible)),
            )),
            _ => None,
        }
    }
}

/// Axe de variantes (`intent: primary | ghost`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct VariantAxis {
    pub name: String,
    pub options: Vec<VariantOption>,
    pub default: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct VariantOption {
    pub name: String,
    #[serde(default)]
    pub overrides: Vec<VariantOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct VariantOverride {
    pub node: NodeId,
    #[serde(default)]
    pub style: StylePatch,
    #[serde(default)]
    pub states: StateStylesPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Asset {
    pub id: AssetId,
    pub file_name: String,
    pub mime: String,
    pub storage_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub height: Option<u32>,
    #[ts(type = "number")]
    pub bytes: u64,
}

/// Entité propriétaire d'un arbre de nœuds.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Owner {
    Page(PageId),
    Layout(LayoutId),
    Component(ComponentId),
}

impl Document {
    /// Document neuf : une page d'accueil vide, tokens par défaut.
    pub fn new(name: &str, ids: &mut IdGen) -> Self {
        let root = ids.node(|_| false);
        let page = ids.page(|_| false);
        let mut root_node = Node::new(
            root.clone(),
            None,
            NodeKind::Box(ContainerProps {
                role: ContainerRole::Generic,
            }),
        );
        root_node.meta.name = Some("Page".to_owned());
        let mut nodes = BTreeMap::new();
        nodes.insert(root.clone(), root_node);
        Self {
            version: IR_VERSION,
            name: name.to_owned(),
            targets: vec![Target::Web],
            settings: SiteSettings {
                lang: "fr".to_owned(),
                site_name: name.to_owned(),
                favicon: None,
            },
            tokens: DesignTokens::default(),
            layouts: Vec::new(),
            pages: vec![Page {
                id: page,
                name: "Accueil".to_owned(),
                route: Vec::new(),
                layout: None,
                root,
                seo: Seo {
                    title: name.to_owned(),
                    description: String::new(),
                    og_image: None,
                },
            }],
            components: Vec::new(),
            nodes,
            assets: Vec::new(),
        }
    }

    pub fn node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn page(&self, id: &PageId) -> Option<&Page> {
        self.pages.iter().find(|p| &p.id == id)
    }

    pub fn layout(&self, id: &LayoutId) -> Option<&Layout> {
        self.layouts.iter().find(|l| &l.id == id)
    }

    pub fn component(&self, id: &ComponentId) -> Option<&Component> {
        self.components.iter().find(|c| &c.id == id)
    }

    pub fn asset(&self, id: &AssetId) -> Option<&Asset> {
        self.assets.iter().find(|a| &a.id == id)
    }

    /// Racines enregistrées (pages, layouts, composants).
    pub fn roots(&self) -> Vec<(Owner, NodeId)> {
        let pages = self.pages.iter().map(|p| (Owner::Page(p.id.clone()), p.root.clone()));
        let layouts = self
            .layouts
            .iter()
            .map(|l| (Owner::Layout(l.id.clone()), l.root.clone()));
        let components = self
            .components
            .iter()
            .map(|c| (Owner::Component(c.id.clone()), c.root.clone()));
        pages.chain(layouts).chain(components).collect()
    }

    /// Ancêtres d'un nœud, du parent à la racine.
    pub fn ancestors(&self, id: &NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut current = self.nodes.get(id).and_then(|n| n.parent.clone());
        while let Some(parent) = current {
            if out.contains(&parent) {
                break;
            }
            current = self.nodes.get(&parent).and_then(|n| n.parent.clone());
            out.push(parent);
        }
        out
    }

    /// Racine de l'arbre qui contient le nœud.
    pub fn tree_root(&self, id: &NodeId) -> NodeId {
        self.ancestors(id).pop().unwrap_or_else(|| id.clone())
    }

    /// Propriétaire de l'arbre qui contient le nœud.
    pub fn owner_of(&self, id: &NodeId) -> Option<Owner> {
        let root = self.tree_root(id);
        self.roots()
            .into_iter()
            .find(|(_, r)| *r == root)
            .map(|(owner, _)| owner)
    }

    /// Vrai si `id` est `ancestor` ou l'un de ses descendants.
    pub fn is_within(&self, id: &NodeId, ancestor: &NodeId) -> bool {
        id == ancestor || self.ancestors(id).contains(ancestor)
    }

    /// Sous-arbre en ordre préfixe (racine d'abord).
    pub fn subtree(&self, root: &NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut stack = vec![root.clone()];
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(node) = self.nodes.get(&id) {
                stack.extend(node.children.iter().rev().cloned());
                out.push(id);
            }
        }
        out
    }

    /// Position d'un nœud parmi les enfants de son parent.
    pub fn index_in_parent(&self, id: &NodeId) -> Option<(NodeId, usize)> {
        let parent = self.nodes.get(id)?.parent.clone()?;
        let index = self.nodes.get(&parent)?.children.iter().position(|c| c == id)?;
        Some((parent, index))
    }

    /// Instances d'un composant.
    pub fn instances_of(&self, component: &ComponentId) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|n| matches!(&n.kind, NodeKind::ComponentInstance(i) if &i.component == component))
            .map(|n| n.id.clone())
            .collect()
    }

    /// Vrai si l'asset est référencé (image, favicon, image OG, valeur de prop).
    pub fn asset_in_use(&self, asset: &AssetId) -> bool {
        let in_source = |source: &ImageSource| matches!(source, ImageSource::Asset { id } if id == asset);
        let in_value = |value: &PropValue| matches!(value, PropValue::Image(source) if in_source(source));
        self.settings.favicon.as_ref() == Some(asset)
            || self.pages.iter().any(|p| p.seo.og_image.as_ref() == Some(asset))
            || self
                .components
                .iter()
                .any(|c| c.props.iter().any(|p| in_value(&p.default)))
            || self.nodes.values().any(|node| match &node.kind {
                NodeKind::Image(image) => in_source(&image.source),
                NodeKind::ComponentInstance(instance) => instance.overrides.iter().any(|o| in_value(&o.value)),
                _ => false,
            })
    }

    /// Pages qui pointent vers une page donnée.
    pub fn links_to_page(&self, page: &PageId) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|n| matches!(&n.kind, NodeKind::Link(link) if matches!(&link.href, Href::Page { page: p, .. } if p == page)))
            .map(|n| n.id.clone())
            .collect()
    }
}
