//! Rendu : les nœuds tels qu'une page les rend, et les hôtes de rendu d'un nœud.
//!
//! Une instance ne produit pas d'élément : l'arbre de son composant est rendu à sa place. Un slot
//! non plus : les enfants de l'instance qui le ciblent sont rendus à sa place, dans le contexte où
//! l'instance est écrite. La racine d'une page est rendue à la place du slot `page` de son layout.
//! Un nœud lié à une prop `Visible` qui vaut `false` dans une instance n'y est pas rendu.

use std::collections::BTreeSet;

use crate::command::neutral::neutral_value;
use crate::document::{BindableField, Document, Owner, Page};
use crate::id::{ComponentId, NodeId};
use crate::node::{ContainerKind, DEFAULT_SLOT, Node, NodeKind, PAGE_SLOT, PropValue};
use crate::style::style::{PropChange, Style, StyleProp};

/// Au-delà de ce multiple du nombre de nœuds du document, le rendu d'une page est interrompu
/// (composants qui s'imbriquent de façon exponentielle) et marqué [`RenderTree::truncated`].
const BUDGET_FACTOR: usize = 64;

/// Nœud rendu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderNode {
    pub id: NodeId,
    /// Rang du parent rendu dans [`RenderTree::nodes`] (`None` : racine du rendu).
    pub parent: Option<usize>,
    /// Instances développées autour du nœud, de la plus externe à celle dont le composant
    /// contient le nœud ; vide pour un nœud écrit dans la page ou dans son layout.
    pub frames: Vec<NodeId>,
}

/// Rendu d'une page, en préordre (instances et slots compris, comme nœuds transparents).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderTree {
    pub nodes: Vec<RenderNode>,
    children: Vec<Vec<usize>>,
    /// Vrai si le rendu a été interrompu par le budget.
    pub truncated: bool,
}

/// Valeur d'un champ lié à une prop dans une instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bound<'d> {
    pub value: &'d PropValue,
    /// Instance dont la surcharge fournit la valeur (`None` : valeur par défaut de la prop).
    pub overridden_by: Option<&'d NodeId>,
}

impl RenderTree {
    /// Rendu complet d'une page : layout (s'il existe), page à la place de son slot `page`.
    pub fn of_page(doc: &Document, page: &Page) -> RenderTree {
        let layout = page.layout.as_ref().and_then(|l| doc.layout(l));
        let layout_nodes: BTreeSet<NodeId> = layout
            .map(|l| doc.subtree(&l.root).into_iter().collect())
            .unwrap_or_default();
        let start = layout.map_or(&page.root, |l| &l.root).clone();
        let mut page_placed = layout.is_none();
        let budget = BUDGET_FACTOR * (doc.nodes.len() + 1);

        let mut tree = RenderTree::default();
        let mut stack: Vec<(NodeId, Option<usize>, Vec<NodeId>)> = vec![(start, None, Vec::new())];
        while let Some((id, parent, frames)) = stack.pop() {
            if tree.nodes.len() >= budget {
                tree.truncated = true;
                break;
            }
            let Some(node) = doc.node(&id) else { continue };
            let hidden_by_prop = matches!(
                bound(doc, &frames, &id, BindableField::Visible),
                Some(Bound {
                    value: PropValue::Bool(false),
                    ..
                })
            );
            if hidden_by_prop || tree.on_path(parent, &id, &frames) {
                continue;
            }
            let index = tree.nodes.len();
            tree.nodes.push(RenderNode {
                id: id.clone(),
                parent,
                frames: frames.clone(),
            });
            tree.children.push(Vec::new());
            if let Some(parent) = parent {
                tree.children[parent].push(index);
            }

            let mut next: Vec<(NodeId, Vec<NodeId>)> = Vec::new();
            match &node.kind {
                NodeKind::Slot(slot) => match frames.split_last() {
                    // Contenu du slot : enfants de l'instance qui le ciblent, dans le contexte
                    // où l'instance est écrite.
                    Some((instance, outer)) => {
                        if let Some(instance) = doc.node(instance) {
                            for child in &instance.children {
                                if slot_target(doc, child) == slot.name {
                                    next.push((child.clone(), outer.to_vec()));
                                }
                            }
                        }
                    }
                    // Seul le slot `page` du layout de la page reçoit la page, une fois.
                    None => {
                        if slot.name == PAGE_SLOT && !page_placed && layout_nodes.contains(&id) {
                            page_placed = true;
                            next.push((page.root.clone(), Vec::new()));
                        }
                    }
                },
                NodeKind::ComponentInstance(props) => {
                    // Un composant déjà en cours de développement n'est pas repris (récursion
                    // signalée par le schéma).
                    let recursive = frames
                        .iter()
                        .any(|f| instance_component(doc, f) == Some(&props.component));
                    if !recursive && let Some(component) = doc.component(&props.component) {
                        let mut inner = frames.clone();
                        inner.push(id.clone());
                        next.push((component.root.clone(), inner));
                    }
                }
                _ => next.extend(node.children.iter().map(|c| (c.clone(), frames.clone()))),
            }
            for (child, child_frames) in next.into_iter().rev() {
                stack.push((child, Some(index), child_frames));
            }
        }
        tree
    }

    /// Vrai si `(id, frames)` est déjà sur le chemin qui mène à `parent` (cycle de parents).
    fn on_path(&self, parent: Option<usize>, id: &NodeId, frames: &[NodeId]) -> bool {
        self.ancestors_from(parent)
            .any(|i| self.nodes[i].id == *id && self.nodes[i].frames == frames)
    }

    fn ancestors_from(&self, start: Option<usize>) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(start, |i| self.nodes[*i].parent)
    }

    /// Ancêtres rendus d'un nœud, du parent à la racine (instances et slots compris).
    pub fn ancestors(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        self.ancestors_from(self.nodes[index].parent)
    }

    /// Le nœud puis ses ancêtres rendus.
    pub fn chain(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        self.ancestors_from(Some(index))
    }

    /// Enfants rendus directs (instances et slots compris).
    pub fn children(&self, index: usize) -> &[usize] {
        &self.children[index]
    }

    /// Parent qui produit un élément : premier ancêtre rendu qui n'est ni une instance ni un slot.
    pub fn element_parent(&self, doc: &Document, index: usize) -> Option<usize> {
        self.ancestors(index).find(|i| !is_transparent(doc, &self.nodes[*i].id))
    }

    /// Enfants qui produisent un élément, à travers les instances et les slots.
    pub fn element_children(&self, doc: &Document, index: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut pending: Vec<usize> = self.children[index].iter().rev().copied().collect();
        while let Some(i) = pending.pop() {
            if is_transparent(doc, &self.nodes[i].id) {
                pending.extend(self.children[i].iter().rev());
            } else {
                out.push(i);
            }
        }
        out
    }

    /// Nœud placé dans l'élément parent : une racine de composant est placée par son instance
    /// (en remontant les instances imbriquées), tout autre nœud par lui-même.
    pub fn placement(&self, doc: &Document, mut index: usize) -> usize {
        while let Some(parent) = self.nodes[index].parent {
            if !matches!(
                doc.node(&self.nodes[parent].id).map(|n| &n.kind),
                Some(NodeKind::ComponentInstance(_))
            ) {
                break;
            }
            index = parent;
        }
        index
    }

    /// Valeur d'un champ lié à une prop pour ce nœud rendu (`None` : champ non lié).
    pub fn bound<'d>(&self, doc: &'d Document, index: usize, field: BindableField) -> Option<Bound<'d>> {
        let node = &self.nodes[index];
        bound(doc, &node.frames, &node.id, field)
    }
}

/// Instances et slots ne produisent pas d'élément.
pub fn is_transparent(doc: &Document, id: &NodeId) -> bool {
    matches!(
        doc.node(id).map(|n| &n.kind),
        Some(NodeKind::ComponentInstance(_) | NodeKind::Slot(_))
    )
}

/// Slot ciblé par un enfant d'instance.
pub fn slot_target<'d>(doc: &'d Document, child: &NodeId) -> &'d str {
    doc.node(child)
        .and_then(|c| c.meta.slot.as_deref())
        .unwrap_or(DEFAULT_SLOT)
}

fn instance_component<'d>(doc: &'d Document, instance: &NodeId) -> Option<&'d ComponentId> {
    match &doc.node(instance)?.kind {
        NodeKind::ComponentInstance(props) => Some(&props.component),
        _ => None,
    }
}

/// Valeur d'un champ lié à une prop du composant de l'instance la plus interne de `frames` :
/// surcharge de l'instance, sinon valeur par défaut de la prop.
pub fn bound<'d>(doc: &'d Document, frames: &[NodeId], node: &NodeId, field: BindableField) -> Option<Bound<'d>> {
    let instance = doc.node(frames.last()?)?;
    let NodeKind::ComponentInstance(props) = &instance.kind else {
        return None;
    };
    let component = doc.component(&props.component)?;
    let prop = component
        .props
        .iter()
        .find(|p| &p.binding.node == node && p.binding.field == field)?;
    Some(match props.overrides.iter().find(|o| o.prop == prop.name) {
        Some(over) => Bound {
            value: &over.value,
            overridden_by: Some(&instance.id),
        },
        None => Bound {
            value: &prop.default,
            overridden_by: None,
        },
    })
}

/// Valeur par défaut d'un champ lié à une prop du composant qui contient le nœud (rendu d'un
/// composant hors de toute instance).
pub fn default_bound<'d>(doc: &'d Document, node: &NodeId, field: BindableField) -> Option<&'d PropValue> {
    let Some(Owner::Component(component)) = doc.owner_of(node) else {
        return None;
    };
    doc.component(&component)?
        .props
        .iter()
        .find(|p| &p.binding.node == node && p.binding.field == field)
        .map(|p| &p.default)
}

/// Style d'un nœud rendu dans le composant de l'instance la plus interne de `frames`, avec les
/// surcharges des variantes que l'instance choisit (option par défaut sinon) ; `None` si aucune
/// surcharge ne vise le nœud.
pub fn variant_style(doc: &Document, frames: &[NodeId], node: &Node) -> Option<Style> {
    let instance = doc.node(frames.last()?)?;
    let NodeKind::ComponentInstance(props) = &instance.kind else {
        return None;
    };
    let component = doc.component(&props.component)?;
    let mut style: Option<Style> = None;
    for axis in &component.variants {
        let chosen = props
            .variants
            .iter()
            .find(|v| v.axis == axis.name)
            .map_or(&axis.default, |v| &v.option);
        let Some(option) = axis.options.iter().find(|o| &o.name == chosen) else {
            continue;
        };
        for over in option.overrides.iter().filter(|o| o.node == node.id) {
            let style = style.get_or_insert_with(|| node.style.clone());
            for (prop, change) in over.style.entries() {
                let PropChange::Merge(patch) = change else { continue };
                let current = style.get(prop).ok().flatten();
                if let Ok(Some(value)) = patch.apply(current, || neutral_value(doc, &node.id, None, prop)) {
                    let _ = style.set(prop, Some(value));
                }
            }
        }
    }
    style
}

// ---------------------------------------------------------------- style permis selon l'hôte

/// Propriétés qui dépendent du type de conteneur du parent.
pub(crate) const PARENT_PROPS: [StyleProp; 5] = [
    StyleProp::ColSpan,
    StyleProp::Grow,
    StyleProp::Shrink,
    StyleProp::AlignSelf,
    StyleProp::Order,
];

/// Vrai si une propriété propre à la primitive est permise sur un nœud de ce type.
pub(crate) fn own_prop_allowed(prop: StyleProp, kind: &NodeKind) -> bool {
    let own = kind.container().map(|(k, _)| k);
    match prop {
        StyleProp::Columns => own == Some(ContainerKind::Grid),
        StyleProp::Direction | StyleProp::Wrap => own == Some(ContainerKind::Stack),
        StyleProp::Gap | StyleProp::Align | StyleProp::Justify => {
            matches!(own, Some(ContainerKind::Stack | ContainerKind::Grid))
        }
        StyleProp::ObjectFit => matches!(kind, NodeKind::Image(_)),
        _ => true,
    }
}

/// Vrai si une propriété de placement est permise sous un hôte de ce type de conteneur
/// (`None` : hôte qui n'est pas un conteneur, ou racine du rendu).
pub(crate) fn placement_allowed(prop: StyleProp, host: Option<ContainerKind>) -> bool {
    match prop {
        StyleProp::ColSpan => host == Some(ContainerKind::Grid),
        StyleProp::Grow | StyleProp::Shrink => host == Some(ContainerKind::Stack),
        StyleProp::AlignSelf | StyleProp::Order => {
            matches!(host, Some(ContainerKind::Stack | ContainerKind::Grid))
        }
        _ => true,
    }
}

/// Vrai si une propriété de style est permise sur ce nœud, sous chacun de ses hôtes au rendu
/// (voir [`rendered_hosts`]) ; un nœud jamais rendu n'impose aucune contrainte de placement.
pub(crate) fn style_prop_allowed(doc: &Document, prop: StyleProp, node: &Node) -> bool {
    own_prop_allowed(prop, &node.kind)
        && (!PARENT_PROPS.contains(&prop)
            || rendered_hosts(doc, &node.id)
                .into_iter()
                .all(|host| placement_allowed(prop, host)))
}

/// Conteneurs qui accueillent un nœud au rendu, un par contexte de rendu possible. Les instances
/// et les slots ne produisent pas d'élément : le contenu d'une instance est rendu à la place du
/// slot qu'il cible dans le composant, une racine de composant à la place de chacune de ses
/// instances, une racine de page à la place du slot `page` de son layout. `None` : hôte qui
/// n'est pas un conteneur, ou racine du rendu. Liste vide : nœud jamais rendu (slot introuvable,
/// composant sans instance, composant récursif) ou contextes trop nombreux pour être parcourus.
pub(crate) fn rendered_hosts(doc: &Document, id: &NodeId) -> Vec<Option<ContainerKind>> {
    // Des composants récursifs qui se transmettent des slots multiplient les contextes : au-delà
    // de ce budget, les hôtes sont tenus pour inconnus.
    let budget = 8 * (doc.nodes.len() + 1);
    let mut hosts = Vec::new();
    // (nœud, instances développées autour de lui, de la plus externe à la plus interne)
    let mut pending: Vec<(NodeId, Vec<NodeId>)> = vec![(id.clone(), Vec::new())];
    let mut visited = BTreeSet::new();
    while let Some((current, chain)) = pending.pop() {
        if !visited.insert((current.clone(), chain.clone())) {
            continue;
        }
        if visited.len() > budget {
            return Vec::new();
        }
        let Some(node) = doc.node(&current) else { continue };
        if let Some(parent) = &node.parent {
            let Some(parent) = doc.node(parent) else { continue };
            match &parent.kind {
                // Une pile plus longue que le nombre de composants trahit une récursion.
                NodeKind::ComponentInstance(props) if chain.len() < doc.components.len() => {
                    let Some(component) = doc.component(&props.component) else {
                        continue;
                    };
                    let target = slot_target(doc, &current);
                    let mut inner = chain.clone();
                    inner.push(parent.id.clone());
                    for slot in doc.subtree(&component.root) {
                        if matches!(doc.node(&slot).map(|n| &n.kind), Some(NodeKind::Slot(s)) if s.name == target) {
                            pending.push((slot, inner.clone()));
                        }
                    }
                }
                NodeKind::ComponentInstance(_) => {}
                kind => hosts.push(kind.container().map(|(k, _)| k)),
            }
            continue;
        }
        // Racine du composant d'une instance développée : l'instance tient sa place.
        if let Some((instance, outer)) = chain.split_last() {
            pending.push((instance.clone(), outer.to_vec()));
            continue;
        }
        match doc.owner_of(&current) {
            Some(Owner::Component(component)) => {
                pending.extend(doc.instances_of(&component).into_iter().map(|i| (i, Vec::new())));
            }
            Some(Owner::Page(page)) => {
                let layout = doc
                    .page(&page)
                    .and_then(|p| p.layout.as_ref())
                    .and_then(|l| doc.layout(l));
                match layout {
                    Some(layout) => {
                        for slot in doc.subtree(&layout.root) {
                            if matches!(doc.node(&slot).map(|n| &n.kind), Some(NodeKind::Slot(s)) if s.name == PAGE_SLOT)
                            {
                                pending.push((slot, Vec::new()));
                            }
                        }
                    }
                    None => hosts.push(None),
                }
            }
            Some(Owner::Layout(_)) => hosts.push(None),
            None => {}
        }
    }
    hosts
}
