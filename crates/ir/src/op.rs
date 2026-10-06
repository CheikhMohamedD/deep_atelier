//! Ops primitives : chaque `apply` modifie le document et renvoie l'op inverse.
//!
//! Les ops ne connaissent que la cohérence structurelle (ids, indices, cycles) ; les règles
//! métier sont vérifiées par l'abaissement des commandes et par la validation.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::document::{Asset, Component, Document, Layout, Page, SiteSettings};
use crate::id::{AssetId, ComponentId, LayoutId, NodeId, PageId};
use crate::node::{A11y, Node, NodeKind, NodeMeta, PlatformOverrides, PlatformScope};
use crate::style::responsive::Responsive;
use crate::style::style::{InteractionState, ResponsiveValue, StyleError, StyleProp};
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

    /// Ramène les indices d'insertion dans les bornes (rejeu partiel d'un brouillon).
    pub fn clamp_indices(&mut self, doc: &Document) {
        let clamp = |index: &mut u32, len: usize| *index = (*index).min(len as u32);
        match self {
            Op::InsertSubtree {
                parent: Some(parent),
                index,
                ..
            } => {
                if let Some(node) = doc.nodes.get(parent) {
                    clamp(index, node.children.len());
                }
            }
            Op::MoveNode { node, parent, index } => {
                if let Some(target) = doc.nodes.get(parent) {
                    let len = target.children.iter().filter(|c| *c != node).count();
                    clamp(index, len);
                }
            }
            Op::PutLayout { index, .. } => clamp(index, doc.layouts.len()),
            Op::PutPage { index, .. } => clamp(index, doc.pages.len()),
            Op::PutComponent { index, .. } => clamp(index, doc.components.len()),
            Op::PutAsset { index, .. } => clamp(index, doc.assets.len()),
            _ => {}
        }
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
    let ids: BTreeSet<&NodeId> = nodes.iter().map(|n| &n.id).collect();
    if ids.len() != nodes.len() {
        return Err(OpError::MalformedSubtree("duplicate node ids".into()));
    }
    for node in nodes {
        if doc.nodes.contains_key(&node.id) {
            return Err(OpError::NodeExists(node.id.clone()));
        }
        if node.id != root.id && !node.parent.as_ref().is_some_and(|p| ids.contains(p)) {
            return Err(OpError::MalformedSubtree(format!(
                "node `{}` is outside the subtree",
                node.id
            )));
        }
    }
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
