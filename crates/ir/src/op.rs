//! Ops primitives : chaque `apply` modifie le document et renvoie l'op inverse.
//!
//! Les ops ne connaissent que la cohérence structurelle (ids, indices, cycles) ; les règles
//! métier sont vérifiées par l'abaissement des commandes et par la validation.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::document::{Asset, Component, Document, Layout, Page, SiteSettings};
use crate::id::{AssetId, ComponentId, LayoutId, NodeId, PageId, TokenName};
use crate::node::{
    A11y, Action, Href, ImageSource, Node, NodeKind, NodeMeta, PlatformOverrides, PlatformScope, PropValue, TextRole,
};
use crate::query::value_colors;
use crate::style::color::ColorRef;
use crate::style::responsive::Responsive;
use crate::style::style::{InteractionState, PropChange, ResponsiveValue, ResponsiveValuePatch, StyleError, StyleProp};
use crate::tokens::DesignTokens;

/// Op primitive inversible. Phase 2 : 1 op ↔ 1 mise à jour Yjs.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Insère un sous-arbre (`nodes[0]` = racine, ordre préfixe). `parent: None` crée une racine
    /// détachée (page, layout ou composant), enregistrée par une op `Put*`.
    InsertSubtree {
        parent: Option<NodeId>,
        index: u32,
        nodes: Vec<Node>,
    },
    RemoveSubtree {
        root: NodeId,
    },
    MoveNode {
        node: NodeId,
        parent: NodeId,
        index: u32,
    },
    SetKind {
        node: NodeId,
        kind: NodeKind,
    },
    SetStyleProp {
        node: NodeId,
        state: Option<InteractionState>,
        prop: StyleProp,
        value: Option<ResponsiveValue>,
    },
    SetVisibility {
        node: NodeId,
        visibility: Option<Responsive<bool>>,
    },
    SetMeta {
        node: NodeId,
        meta: NodeMeta,
    },
    SetA11y {
        node: NodeId,
        a11y: A11y,
    },
    SetPlatform {
        node: NodeId,
        scope: PlatformScope,
    },
    SetOverrides {
        node: NodeId,
        overrides: PlatformOverrides,
    },
    /// Insère à `index` ou remplace l'entrée de même id (à sa place).
    PutLayout {
        index: u32,
        layout: Layout,
    },
    RemoveLayout {
        id: LayoutId,
    },
    PutPage {
        index: u32,
        page: Page,
    },
    RemovePage {
        id: PageId,
    },
    PutComponent {
        index: u32,
        component: Component,
    },
    RemoveComponent {
        id: ComponentId,
    },
    SetTokens {
        tokens: DesignTokens,
    },
    PutAsset {
        index: u32,
        asset: Asset,
    },
    RemoveAsset {
        id: AssetId,
    },
    SetSettings {
        settings: SiteSettings,
    },
}

/// Échec d'application d'une op (le document n'est pas modifié).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpError {
    #[error("node `{0}` not found")]
    NodeNotFound(NodeId),
    #[error("node `{0}` already exists")]
    NodeExists(NodeId),
    #[error("index {index} out of bounds (length {len})")]
    IndexOutOfBounds { index: u32, len: usize },
    #[error("node `{0}` is a root and has no parent")]
    RootNode(NodeId),
    #[error("moving `{node}` into `{parent}` would create a cycle")]
    Cycle { node: NodeId, parent: NodeId },
    #[error("malformed subtree: {0}")]
    MalformedSubtree(String),
    #[error("{0} not found")]
    EntityNotFound(String),
    #[error(transparent)]
    Style(#[from] StyleError),
}

/// Entité identifiée de l'IR.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Entity {
    Node(NodeId),
    Page(PageId),
    Layout(LayoutId),
    Component(ComponentId),
    Asset(AssetId),
    /// Token de couleur, nommé par les styles.
    ColorToken(TokenName),
    /// Token de police, nommé par `font_family`.
    FontToken(TokenName),
}

/// Champ d'un nœud remplacé par une op.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum NodeField {
    /// Tous les champs : nœud inséré avec ses valeurs.
    All,
    /// Parent (déplacement) ; l'ordre des enfants n'est pas une donnée suivie.
    Parent,
    Kind,
    Style(Option<InteractionState>, StyleProp),
    Visibility,
    Meta,
    A11y,
    Platform,
    Overrides,
}

/// Donnée de l'IR touchée par une op (dépendances entre unités d'un brouillon IA).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum OpKey {
    /// Existence d'une entité (création, suppression).
    Exists(Entity),
    /// Valeur entière d'une page, d'un layout, d'un composant ou d'un asset.
    Value(Entity),
    /// Champ d'un nœud.
    Field(NodeId, NodeField),
    Tokens,
    Settings,
}

/// Accès d'une op au document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OpAccess {
    /// Données dont l'op enregistre la valeur entière (calculée sur l'état qui la précède), ou
    /// dont elle change l'existence.
    pub writes: Vec<OpKey>,
    /// Données que l'op suppose présentes sans les écrire : nœuds visés, parents, entités
    /// référencées.
    pub reads: Vec<OpKey>,
}

impl OpAccess {
    fn node_field(node: &NodeId, field: NodeField) -> Self {
        Self {
            writes: vec![OpKey::Field(node.clone(), field)],
            reads: vec![OpKey::Exists(Entity::Node(node.clone()))],
        }
    }

    /// `Put*` : remplace la valeur, et crée l'entité si l'inverse la retire.
    fn put(entity: Entity, creates: bool) -> Self {
        let mut writes = vec![OpKey::Value(entity.clone())];
        if creates {
            writes.push(OpKey::Exists(entity));
        }
        Self {
            writes,
            reads: Vec::new(),
        }
    }

    fn removal(entity: Entity) -> Self {
        Self {
            writes: vec![OpKey::Exists(entity)],
            reads: Vec::new(),
        }
    }

    fn read(&mut self, entity: Entity) {
        self.reads.push(OpKey::Exists(entity));
    }

    fn read_href(&mut self, href: &Href) {
        if let Href::Page { page, .. } = href {
            self.read(Entity::Page(page.clone()));
        }
    }

    fn read_image(&mut self, source: &ImageSource) {
        if let ImageSource::Asset { id } = source {
            self.read(Entity::Asset(id.clone()));
        }
    }

    fn read_value(&mut self, value: &PropValue) {
        match value {
            PropValue::Href(href) => self.read_href(href),
            PropValue::Image(source) => self.read_image(source),
            PropValue::Text(_) | PropValue::Bool(_) => {}
        }
    }

    fn read_color(&mut self, color: &ColorRef) {
        if let Some(name) = color.token_name() {
            self.read(Entity::ColorToken(name.clone()));
        }
    }

    /// Tokens nommés par une valeur de style : couleurs (texte, fond, bordure, anneau) et police
    /// (seule propriété à valeur `Token`).
    fn read_style_value(&mut self, value: &ResponsiveValue) {
        if let ResponsiveValue::Token(fonts) = value {
            for name in fonts.values() {
                self.read(Entity::FontToken(name.clone()));
            }
        }
        for color in value_colors(value) {
            self.read_color(color);
        }
    }

    /// Tokens nommés par un patch de style (surcharges de variantes).
    fn read_style_patch(&mut self, entries: Vec<(StyleProp, PropChange)>) {
        for (_, change) in entries {
            match change {
                PropChange::Merge(ResponsiveValuePatch::Token(fonts)) => {
                    for name in fonts.values() {
                        self.read(Entity::FontToken(name.clone()));
                    }
                }
                PropChange::Merge(ResponsiveValuePatch::Color(colors)) => {
                    for color in colors.values() {
                        self.read_color(color);
                    }
                }
                PropChange::Merge(ResponsiveValuePatch::Background(backgrounds)) => {
                    for color in backgrounds.values().into_iter().flat_map(|b| b.colors()) {
                        self.read_color(color);
                    }
                }
                PropChange::Merge(ResponsiveValuePatch::Ring(rings)) => {
                    for ring in rings.values() {
                        self.read_color(&ring.color);
                    }
                }
                _ => {}
            }
        }
    }

    /// Entités référencées par un nœud inséré : style, états d'interaction et type.
    fn read_node(&mut self, node: &Node) {
        for (_, value) in node.style.entries() {
            self.read_style_value(&value);
        }
        for state in InteractionState::ALL {
            for (_, value) in node.states.get(state).entries() {
                self.read_style_value(&value);
            }
        }
        self.read_kind(&node.kind);
    }

    /// Entités référencées par un type de nœud.
    fn read_kind(&mut self, kind: &NodeKind) {
        match kind {
            NodeKind::Text(text) => {
                if let TextRole::Label { for_input: Some(input) } = &text.role {
                    self.read(Entity::Node(input.clone()));
                }
                for color in text.content.iter().filter_map(|run| run.color.as_ref()) {
                    self.read_color(color);
                }
            }
            NodeKind::Button(button) => {
                if let Some(Action::ToggleVisibility { target }) = &button.action {
                    self.read(Entity::Node(target.clone()));
                }
            }
            NodeKind::Image(image) => self.read_image(&image.source),
            NodeKind::Link(link) => self.read_href(&link.href),
            NodeKind::ComponentInstance(instance) => {
                let component = Entity::Component(instance.component.clone());
                // Surcharges et variantes nomment des props et des axes du composant : elles
                // dépendent de sa valeur, pas seulement de son existence.
                if !instance.overrides.is_empty() || !instance.variants.is_empty() {
                    self.reads.push(OpKey::Value(component.clone()));
                }
                self.read(component);
                for value in instance.overrides.iter().map(|o| &o.value) {
                    self.read_value(value);
                }
            }
            NodeKind::Box(_)
            | NodeKind::Stack(_)
            | NodeKind::Grid(_)
            | NodeKind::Icon(_)
            | NodeKind::Input(_)
            | NodeKind::Slot(_)
            | NodeKind::RawCode(_) => {}
        }
    }
}

fn check_index(index: u32, len: usize) -> Result<usize, OpError> {
    let i = index as usize;
    if i <= len {
        Ok(i)
    } else {
        Err(OpError::IndexOutOfBounds { index, len })
    }
}

fn node_mut<'a>(doc: &'a mut Document, id: &NodeId) -> Result<&'a mut Node, OpError> {
    doc.nodes.get_mut(id).ok_or_else(|| OpError::NodeNotFound(id.clone()))
}

/// Insère ou remplace une entité identifiée dans une liste ordonnée ; retourne l'inverse.
fn put<T: Clone, K: PartialEq>(
    list: &mut Vec<T>,
    index: u32,
    item: T,
    key: impl Fn(&T) -> &K,
    put_op: impl Fn(u32, T) -> Op,
    remove_op: impl Fn(&T) -> Op,
) -> Result<Op, OpError> {
    if let Some(i) = list.iter().position(|x| key(x) == key(&item)) {
        let old = std::mem::replace(&mut list[i], item);
        return Ok(put_op(i as u32, old));
    }
    let i = check_index(index, list.len())?;
    let inverse = remove_op(&item);
    list.insert(i, item);
    Ok(inverse)
}

/// Retire une entité identifiée ; retourne l'inverse.
fn remove<T, K: PartialEq + std::fmt::Display>(
    list: &mut Vec<T>,
    id: &K,
    key: impl Fn(&T) -> &K,
    put_op: impl Fn(u32, T) -> Op,
) -> Result<Op, OpError> {
    let i = list
        .iter()
        .position(|x| key(x) == id)
        .ok_or_else(|| OpError::EntityNotFound(id.to_string()))?;
    let old = list.remove(i);
    Ok(put_op(i as u32, old))
}

impl Op {
    /// Applique l'op ; retourne l'op inverse. En cas d'erreur, le document est inchangé.
    pub fn apply(&self, doc: &mut Document) -> Result<Op, OpError> {
        match self {
            Op::InsertSubtree { parent, index, nodes } => insert_subtree(doc, parent.as_ref(), *index, nodes),
            Op::RemoveSubtree { root } => remove_subtree(doc, root),
            Op::MoveNode { node, parent, index } => move_node(doc, node, parent, *index),
            Op::SetKind { node, kind } => {
                let target = node_mut(doc, node)?;
                let old = std::mem::replace(&mut target.kind, kind.clone());
                Ok(Op::SetKind {
                    node: node.clone(),
                    kind: old,
                })
            }
            Op::SetStyleProp {
                node,
                state,
                prop,
                value,
            } => {
                let target = node_mut(doc, node)?;
                let old = match state {
                    None => target.style.set(*prop, value.clone())?,
                    Some(state) => target.states.get_mut(*state).set(*prop, value.clone())?,
                };
                Ok(Op::SetStyleProp {
                    node: node.clone(),
                    state: *state,
                    prop: *prop,
                    value: old,
                })
            }
            Op::SetVisibility { node, visibility } => {
                let target = node_mut(doc, node)?;
                let old = std::mem::replace(&mut target.visibility, visibility.clone());
                Ok(Op::SetVisibility {
                    node: node.clone(),
                    visibility: old,
                })
            }
            Op::SetMeta { node, meta } => {
                let target = node_mut(doc, node)?;
                let old = std::mem::replace(&mut target.meta, meta.clone());
                Ok(Op::SetMeta {
                    node: node.clone(),
                    meta: old,
                })
            }
            Op::SetA11y { node, a11y } => {
                let target = node_mut(doc, node)?;
                let old = std::mem::replace(&mut target.a11y, a11y.clone());
                Ok(Op::SetA11y {
                    node: node.clone(),
                    a11y: old,
                })
            }
            Op::SetPlatform { node, scope } => {
                let target = node_mut(doc, node)?;
                let old = std::mem::replace(&mut target.platform, *scope);
                Ok(Op::SetPlatform {
                    node: node.clone(),
                    scope: old,
                })
            }
            Op::SetOverrides { node, overrides } => {
                let target = node_mut(doc, node)?;
                let old = std::mem::replace(&mut target.platform_overrides, overrides.clone());
                Ok(Op::SetOverrides {
                    node: node.clone(),
                    overrides: old,
                })
            }
            Op::PutLayout { index, layout } => put(
                &mut doc.layouts,
                *index,
                layout.clone(),
                |l| &l.id,
                |index, layout| Op::PutLayout { index, layout },
                |l| Op::RemoveLayout { id: l.id.clone() },
            ),
            Op::RemoveLayout { id } => remove(
                &mut doc.layouts,
                id,
                |l| &l.id,
                |index, layout| Op::PutLayout { index, layout },
            ),
            Op::PutPage { index, page } => put(
                &mut doc.pages,
                *index,
                page.clone(),
                |p| &p.id,
                |index, page| Op::PutPage { index, page },
                |p| Op::RemovePage { id: p.id.clone() },
            ),
            Op::RemovePage { id } => remove(&mut doc.pages, id, |p| &p.id, |index, page| Op::PutPage { index, page }),
            Op::PutComponent { index, component } => put(
                &mut doc.components,
                *index,
                component.clone(),
                |c| &c.id,
                |index, component| Op::PutComponent { index, component },
                |c| Op::RemoveComponent { id: c.id.clone() },
            ),
            Op::RemoveComponent { id } => remove(
                &mut doc.components,
                id,
                |c| &c.id,
                |index, component| Op::PutComponent { index, component },
            ),
            Op::SetTokens { tokens } => {
                let old = std::mem::replace(&mut doc.tokens, tokens.clone());
                Ok(Op::SetTokens { tokens: old })
            }
            Op::PutAsset { index, asset } => put(
                &mut doc.assets,
                *index,
                asset.clone(),
                |a| &a.id,
                |index, asset| Op::PutAsset { index, asset },
                |a| Op::RemoveAsset { id: a.id.clone() },
            ),
            Op::RemoveAsset { id } => remove(
                &mut doc.assets,
                id,
                |a| &a.id,
                |index, asset| Op::PutAsset { index, asset },
            ),
            Op::SetSettings { settings } => {
                let old = std::mem::replace(&mut doc.settings, settings.clone());
                Ok(Op::SetSettings { settings: old })
            }
        }
    }

    /// Vrai si les deux ops ne diffèrent que par leur indice de position (rang parmi les enfants
    /// d'un nœud ou dans la liste des pages, layouts, composants, assets). L'ordre n'est pas une
    /// dépendance entre unités d'un brouillon.
    pub(crate) fn same_except_position(&self, other: &Op) -> bool {
        self.without_position() == other.without_position()
    }

    fn without_position(&self) -> Op {
        let mut op = self.clone();
        match &mut op {
            Op::InsertSubtree { index, .. }
            | Op::MoveNode { index, .. }
            | Op::PutLayout { index, .. }
            | Op::PutPage { index, .. }
            | Op::PutComponent { index, .. }
            | Op::PutAsset { index, .. } => *index = 0,
            _ => {}
        }
        op
    }

    /// Enregistre dans `changes` ce que touche l'op (à appeler sur l'op et sur son inverse).
    pub fn touch(&self, changes: &mut ChangeSet) {
        match self {
            Op::InsertSubtree { parent, nodes, .. } => {
                changes.nodes.extend(parent.iter().cloned());
                changes.nodes.extend(nodes.iter().map(|n| n.id.clone()));
            }
            Op::RemoveSubtree { root } => {
                changes.nodes.insert(root.clone());
            }
            Op::MoveNode { node, parent, .. } => {
                changes.nodes.insert(node.clone());
                changes.nodes.insert(parent.clone());
            }
            Op::SetKind { node, .. }
            | Op::SetStyleProp { node, .. }
            | Op::SetVisibility { node, .. }
            | Op::SetMeta { node, .. }
            | Op::SetA11y { node, .. }
            | Op::SetPlatform { node, .. }
            | Op::SetOverrides { node, .. } => {
                changes.nodes.insert(node.clone());
            }
            Op::PutLayout { layout, .. } => {
                changes.layouts.insert(layout.id.clone());
            }
            Op::RemoveLayout { id } => {
                changes.layouts.insert(id.clone());
            }
            Op::PutPage { page, .. } => {
                changes.pages.insert(page.id.clone());
            }
            Op::RemovePage { id } => {
                changes.pages.insert(id.clone());
            }
            Op::PutComponent { component, .. } => {
                changes.components.insert(component.id.clone());
            }
            Op::RemoveComponent { id } => {
                changes.components.insert(id.clone());
            }
            Op::SetTokens { .. } => changes.tokens = true,
            Op::PutAsset { asset, .. } => {
                changes.assets.insert(asset.id.clone());
            }
            Op::RemoveAsset { id } => {
                changes.assets.insert(id.clone());
            }
            Op::SetSettings { .. } => changes.settings = true,
        }
    }

    /// Données écrites et lues par l'op. `inverse` est l'op renvoyée par son `apply` : il
    /// distingue la création d'une entité du remplacement de sa valeur.
    pub(crate) fn access(&self, inverse: &Op) -> OpAccess {
        match self {
            Op::InsertSubtree { parent, nodes, .. } => {
                let mut access = OpAccess::default();
                if let Some(parent) = parent {
                    access.read(Entity::Node(parent.clone()));
                }
                for node in nodes {
                    access.writes.push(OpKey::Exists(Entity::Node(node.id.clone())));
                    access.writes.push(OpKey::Field(node.id.clone(), NodeField::All));
                    access.read_node(node);
                }
                access
            }
            // Le retrait ne porte aucune valeur : seule compte l'existence de la racine.
            Op::RemoveSubtree { root } => OpAccess::removal(Entity::Node(root.clone())),
            Op::MoveNode { node, parent, .. } => {
                let mut access = OpAccess::node_field(node, NodeField::Parent);
                access.read(Entity::Node(parent.clone()));
                access
            }
            Op::SetKind { node, kind } => {
                let mut access = OpAccess::node_field(node, NodeField::Kind);
                access.read_kind(kind);
                access
            }
            Op::SetStyleProp {
                node,
                state,
                prop,
                value,
            } => {
                let mut access = OpAccess::node_field(node, NodeField::Style(*state, *prop));
                if let Some(value) = value {
                    access.read_style_value(value);
                }
                access
            }
            Op::SetVisibility { node, .. } => OpAccess::node_field(node, NodeField::Visibility),
            Op::SetMeta { node, .. } => OpAccess::node_field(node, NodeField::Meta),
            Op::SetA11y { node, .. } => OpAccess::node_field(node, NodeField::A11y),
            Op::SetPlatform { node, .. } => OpAccess::node_field(node, NodeField::Platform),
            Op::SetOverrides { node, .. } => OpAccess::node_field(node, NodeField::Overrides),
            Op::PutLayout { layout, .. } => {
                let creates = matches!(inverse, Op::RemoveLayout { .. });
                let mut access = OpAccess::put(Entity::Layout(layout.id.clone()), creates);
                access.read(Entity::Node(layout.root.clone()));
                access
            }
            Op::RemoveLayout { id } => OpAccess::removal(Entity::Layout(id.clone())),
            Op::PutPage { page, .. } => {
                let creates = matches!(inverse, Op::RemovePage { .. });
                let mut access = OpAccess::put(Entity::Page(page.id.clone()), creates);
                access.read(Entity::Node(page.root.clone()));
                if let Some(layout) = &page.layout {
                    access.read(Entity::Layout(layout.clone()));
                }
                if let Some(image) = &page.seo.og_image {
                    access.read(Entity::Asset(image.clone()));
                }
                access
            }
            Op::RemovePage { id } => OpAccess::removal(Entity::Page(id.clone())),
            Op::PutComponent { component, .. } => {
                let creates = matches!(inverse, Op::RemoveComponent { .. });
                let mut access = OpAccess::put(Entity::Component(component.id.clone()), creates);
                access.read(Entity::Node(component.root.clone()));
                for prop in &component.props {
                    access.read_value(&prop.default);
                }
                for over in component
                    .variants
                    .iter()
                    .flat_map(|axis| &axis.options)
                    .flat_map(|option| &option.overrides)
                {
                    access.read_style_patch(over.style.entries());
                    for state in InteractionState::ALL {
                        if let Some(patch) = over.states.get(state) {
                            access.read_style_patch(patch.entries());
                        }
                    }
                }
                access
            }
            Op::RemoveComponent { id } => OpAccess::removal(Entity::Component(id.clone())),
            Op::SetTokens { tokens } => {
                let mut access = OpAccess {
                    writes: vec![OpKey::Tokens],
                    reads: Vec::new(),
                };
                // Les tokens créés ou supprimés : les styles qui les nomment en dépendent. Un
                // changement de valeur laisse la référence valide.
                if let Op::SetTokens { tokens: before } = inverse {
                    let colors = |t: &DesignTokens| t.colors.iter().map(|c| c.name.clone()).collect::<BTreeSet<_>>();
                    let fonts = |t: &DesignTokens| t.fonts.iter().map(|f| f.name.clone()).collect::<BTreeSet<_>>();
                    for name in colors(tokens).symmetric_difference(&colors(before)) {
                        access.writes.push(OpKey::Exists(Entity::ColorToken(name.clone())));
                    }
                    for name in fonts(tokens).symmetric_difference(&fonts(before)) {
                        access.writes.push(OpKey::Exists(Entity::FontToken(name.clone())));
                    }
                }
                access
            }
            Op::PutAsset { asset, .. } => {
                let creates = matches!(inverse, Op::RemoveAsset { .. });
                OpAccess::put(Entity::Asset(asset.id.clone()), creates)
            }
            Op::RemoveAsset { id } => OpAccess::removal(Entity::Asset(id.clone())),
            Op::SetSettings { settings } => {
                let mut access = OpAccess {
                    writes: vec![OpKey::Settings],
                    reads: Vec::new(),
                };
                if let Some(favicon) = &settings.favicon {
                    access.read(Entity::Asset(favicon.clone()));
                }
                access
            }
        }
    }
}

fn insert_subtree(doc: &mut Document, parent: Option<&NodeId>, index: u32, nodes: &[Node]) -> Result<Op, OpError> {
    let root = nodes
        .first()
        .ok_or_else(|| OpError::MalformedSubtree("empty subtree".into()))?;
    if root.parent.as_ref() != parent {
        return Err(OpError::MalformedSubtree(format!(
            "root `{}` does not point to its parent",
            root.id
        )));
    }
    let by_id: BTreeMap<&NodeId, &Node> = nodes.iter().map(|n| (&n.id, n)).collect();
    if by_id.len() != nodes.len() {
        return Err(OpError::MalformedSubtree("duplicate node ids".into()));
    }
    if let Some(node) = nodes.iter().find(|n| doc.nodes.contains_key(&n.id)) {
        return Err(OpError::NodeExists(node.id.clone()));
    }
    check_subtree_links(nodes, &by_id)?;
    if let Some(parent) = parent {
        let target = doc
            .nodes
            .get(parent)
            .ok_or_else(|| OpError::NodeNotFound(parent.clone()))?;
        let i = check_index(index, target.children.len())?;
        node_mut(doc, parent)?.children.insert(i, root.id.clone());
    }
    for node in nodes {
        doc.nodes.insert(node.id.clone(), node.clone());
    }
    Ok(Op::RemoveSubtree { root: root.id.clone() })
}

/// Vérifie que les liens `parent` et `children` d'un sous-arbre (`nodes[0]` = racine) concordent
/// dans les deux sens : chaque nœud hors racine figure une seule fois dans les enfants de son
/// parent, chaque enfant listé appartient au sous-arbre, et tous les nœuds sont atteignables
/// depuis la racine (ni orphelin ni cycle détaché). L'inverse `RemoveSubtree`, qui suit
/// `children`, retire alors exactement ce qui a été inséré.
fn check_subtree_links(nodes: &[Node], by_id: &BTreeMap<&NodeId, &Node>) -> Result<(), OpError> {
    let mut listed: BTreeSet<&NodeId> = BTreeSet::new();
    let mut stack: Vec<&Node> = nodes.first().into_iter().collect();
    // Chaque nœud n'est empilé qu'après sa première inscription dans `listed` : le parcours se
    // termine même sur un payload cyclique.
    while let Some(node) = stack.pop() {
        for child in &node.children {
            let Some(&child_node) = by_id.get(child) else {
                return Err(OpError::MalformedSubtree(format!(
                    "child `{child}` of `{}` is outside the subtree",
                    node.id
                )));
            };
            if child_node.parent.as_ref() != Some(&node.id) {
                return Err(OpError::MalformedSubtree(format!(
                    "`{child}` is listed as a child of `{}` but points to another parent",
                    node.id
                )));
            }
            if !listed.insert(child) {
                return Err(OpError::MalformedSubtree(format!("`{child}` is listed twice")));
            }
            stack.push(child_node);
        }
    }
    match nodes.iter().skip(1).find(|n| !listed.contains(&n.id)) {
        Some(node) => Err(OpError::MalformedSubtree(format!(
            "node `{}` is not reachable from the root",
            node.id
        ))),
        None => Ok(()),
    }
}

fn remove_subtree(doc: &mut Document, root: &NodeId) -> Result<Op, OpError> {
    let node = doc.nodes.get(root).ok_or_else(|| OpError::NodeNotFound(root.clone()))?;
    let parent = node.parent.clone();
    let mut index = 0;
    if let Some(parent_id) = &parent {
        let siblings = &mut node_mut(doc, parent_id)?.children;
        let i = siblings
            .iter()
            .position(|c| c == root)
            .ok_or_else(|| OpError::MalformedSubtree(format!("`{root}` missing from its parent")))?;
        siblings.remove(i);
        index = i as u32;
    }
    let nodes = doc.subtree(root).iter().filter_map(|id| doc.nodes.remove(id)).collect();
    Ok(Op::InsertSubtree { parent, index, nodes })
}

fn move_node(doc: &mut Document, node: &NodeId, parent: &NodeId, index: u32) -> Result<Op, OpError> {
    let current = doc.nodes.get(node).ok_or_else(|| OpError::NodeNotFound(node.clone()))?;
    let old_parent = current.parent.clone().ok_or_else(|| OpError::RootNode(node.clone()))?;
    if !doc.nodes.contains_key(parent) {
        return Err(OpError::NodeNotFound(parent.clone()));
    }
    if doc.is_within(parent, node) {
        return Err(OpError::Cycle {
            node: node.clone(),
            parent: parent.clone(),
        });
    }
    let old_index = doc
        .nodes
        .get(&old_parent)
        .and_then(|p| p.children.iter().position(|c| c == node))
        .ok_or_else(|| OpError::MalformedSubtree(format!("`{node}` missing from its parent")))?;
    let target_len = doc.nodes[parent].children.len() - usize::from(*parent == old_parent);
    let i = check_index(index, target_len)?;
    node_mut(doc, &old_parent)?.children.remove(old_index);
    node_mut(doc, parent)?.children.insert(i, node.clone());
    node_mut(doc, node)?.parent = Some(parent.clone());
    Ok(Op::MoveNode {
        node: node.clone(),
        parent: old_parent,
        index: old_index as u32,
    })
}

/// Ce qu'une transaction a touché (compilation incrémentale, rafraîchissement du canvas).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ChangeSet {
    pub nodes: BTreeSet<NodeId>,
    pub pages: BTreeSet<PageId>,
    pub layouts: BTreeSet<LayoutId>,
    pub components: BTreeSet<ComponentId>,
    pub assets: BTreeSet<AssetId>,
    pub tokens: bool,
    pub settings: bool,
}

impl ChangeSet {
    /// Changements d'une suite d'ops et de leurs inverses.
    pub fn from_ops<'a>(ops: impl IntoIterator<Item = &'a Op>) -> ChangeSet {
        let mut changes = ChangeSet::default();
        for op in ops {
            op.touch(&mut changes);
        }
        changes
    }

    pub fn merge(&mut self, other: ChangeSet) {
        self.nodes.extend(other.nodes);
        self.pages.extend(other.pages);
        self.layouts.extend(other.layouts);
        self.components.extend(other.components);
        self.assets.extend(other.assets);
        self.tokens |= other.tokens;
        self.settings |= other.settings;
    }

    pub fn is_empty(&self) -> bool {
        *self == ChangeSet::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::IdGen;
    use crate::node::{ContainerProps, ContainerRole};

    fn boxed(ids: &mut IdGen, parent: &NodeId) -> Node {
        Node::new(
            ids.node(|_| false),
            Some(parent.clone()),
            NodeKind::Box(ContainerProps {
                role: ContainerRole::Generic,
            }),
        )
    }

    #[test]
    fn insert_move_remove_are_inverted_exactly() {
        let mut ids = IdGen::from_seed(1);
        let mut doc = Document::new("Test", &mut ids);
        let root = doc.pages[0].root.clone();
        let original = doc.clone();

        let a = boxed(&mut ids, &root);
        let b = boxed(&mut ids, &root);
        let undo_a = Op::InsertSubtree {
            parent: Some(root.clone()),
            index: 0,
            nodes: vec![a.clone()],
        }
        .apply(&mut doc)
        .unwrap();
        let undo_b = Op::InsertSubtree {
            parent: Some(root.clone()),
            index: 1,
            nodes: vec![b.clone()],
        }
        .apply(&mut doc)
        .unwrap();
        let after_insert = doc.clone();

        let undo_move = Op::MoveNode {
            node: b.id.clone(),
            parent: a.id.clone(),
            index: 0,
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.nodes[&a.id].children, vec![b.id.clone()]);
        let redo_move = undo_move.apply(&mut doc).unwrap();
        assert_eq!(doc, after_insert);
        redo_move.apply(&mut doc).unwrap();

        let undo_remove = Op::RemoveSubtree { root: a.id.clone() }.apply(&mut doc).unwrap();
        assert!(!doc.nodes.contains_key(&b.id));
        undo_remove.apply(&mut doc).unwrap();
        assert_eq!(doc.nodes[&a.id].children, vec![b.id.clone()]);

        Op::MoveNode {
            node: b.id.clone(),
            parent: root.clone(),
            index: 1,
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc, after_insert);
        undo_b.apply(&mut doc).unwrap();
        undo_a.apply(&mut doc).unwrap();
        assert_eq!(doc, original);
    }

    #[test]
    fn invalid_ops_leave_the_document_untouched() {
        let mut ids = IdGen::from_seed(2);
        let mut doc = Document::new("Test", &mut ids);
        let root = doc.pages[0].root.clone();
        let a = boxed(&mut ids, &root);
        Op::InsertSubtree {
            parent: Some(root.clone()),
            index: 0,
            nodes: vec![a.clone()],
        }
        .apply(&mut doc)
        .unwrap();
        let before = doc.clone();
        assert!(matches!(
            Op::MoveNode {
                node: root.clone(),
                parent: a.id.clone(),
                index: 0
            }
            .apply(&mut doc),
            Err(OpError::RootNode(_))
        ));
        let child = boxed(&mut ids, &a.id);
        Op::InsertSubtree {
            parent: Some(a.id.clone()),
            index: 0,
            nodes: vec![child.clone()],
        }
        .apply(&mut doc)
        .unwrap();
        let with_child = doc.clone();
        assert!(matches!(
            Op::MoveNode {
                node: a.id.clone(),
                parent: child.id.clone(),
                index: 0
            }
            .apply(&mut doc),
            Err(OpError::Cycle { .. })
        ));
        assert!(matches!(
            Op::InsertSubtree {
                parent: Some(root.clone()),
                index: 9,
                nodes: vec![boxed(&mut ids, &root)]
            }
            .apply(&mut doc),
            Err(OpError::IndexOutOfBounds { .. })
        ));
        assert_eq!(doc, with_child);
        assert_ne!(doc, before);
    }
}
