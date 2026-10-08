//! Accessibilité : alt, titres, noms accessibles, landmark `Main`, langue.
//!
//! `a11y.hidden` (aria-hidden) retire le nœud et tout ce qu'il rend de l'arbre d'accessibilité :
//! son sous-arbre, l'arbre du composant d'une instance, le contenu d'un slot.

use std::collections::{BTreeMap, BTreeSet};

use crate::document::{BindableField, Component, Document, Owner};
use crate::id::NodeId;
use crate::node::{ContainerRole, DEFAULT_SLOT, InstanceProps, Node, NodeKind, PAGE_SLOT, PropValue, TextRole};
use crate::render::{RenderTree, default_bound};
use crate::validate::{Context, Issue, IssueCode as C};

pub(super) fn check(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    if !valid_lang(&doc.settings.lang) {
        issues.push(Issue::error(
            C::A11yLang,
            None,
            format!("site language `{}` is not a valid BCP 47 tag", doc.settings.lang),
        ));
    }
    // Nœud jamais rendu (composant sans instance, layout sans page) : jugé dans l'arbre où il
    // est écrit, avec les valeurs par défaut des props qui le lient.
    let hidden = hidden_nodes(ctx);
    for node in doc.nodes.values() {
        if ctx.owner(&node.id).is_none() || ctx.is_rendered(&node.id) || hidden.contains(&node.id) {
            continue;
        }
        let bound = |field: BindableField| default_bound(doc, &node.id, field).map(|value| (value, None));
        node_rules(ctx, node, &[], &bound, issues);
    }
    // Nœuds rendus : jugés à chaque rendu exposé, avec les valeurs de l'instance qui les rend.
    for (page, render) in &ctx.renders {
        let masked = masked(doc, render);
        let mut h1 = 0;
        let mut previous = 0u8;
        let mut mains = 0;
        for (index, entry) in render.nodes.iter().enumerate() {
            // Un titre ou un `Main` masqué n'existe pas pour les technologies d'assistance.
            if masked[index] {
                continue;
            }
            let Some(node) = doc.node(&entry.id) else { continue };
            let id = &entry.id;
            let bound = |field: BindableField| render.bound(doc, index, field).map(|b| (b.value, b.overridden_by));
            node_rules(ctx, node, &entry.frames, &bound, issues);
            if let NodeKind::Text(text) = &node.kind
                && let TextRole::Heading { level } = text.role
            {
                let level = level.number();
                if level == 1 {
                    h1 += 1;
                    if h1 == 2 {
                        issues.push(Issue::error(
                            C::A11yMultipleH1,
                            Some(id),
                            format!("page `{}` has more than one H1", page.name),
                        ));
                    }
                }
                if previous > 0 && level > previous + 1 {
                    issues.push(Issue::error(
                        C::A11yHeadingSkip,
                        Some(id),
                        format!(
                            "heading level jumps from H{previous} to H{level} on page `{}`",
                            page.name
                        ),
                    ));
                }
                previous = level;
            }
            if node
                .kind
                .container()
                .is_some_and(|(_, p)| p.role == ContainerRole::Main)
            {
                mains += 1;
                if mains == 2 {
                    issues.push(Issue::error(
                        C::A11yMultipleMain,
                        Some(id),
                        format!("page `{}` has more than one Main landmark", page.name),
                    ));
                }
            }
        }
        if h1 == 0 {
            issues.push(Issue::warning(
                C::A11yMissingH1,
                Some(&page.root),
                format!("page `{}` has no H1", page.name),
            ));
        }
        if mains == 0 {
            issues.push(Issue::warning(
                C::A11yMissingMain,
                Some(&page.root),
                format!("page `{}` has no Main landmark", page.name),
            ));
        }
    }
}

/// Masquage au rendu : `a11y.hidden` posé sur le nœud ou sur un ancêtre rendu (instance qui rend
/// un composant, slot qui rend un contenu, slot `page` qui rend la page compris).
fn masked(doc: &Document, render: &RenderTree) -> Vec<bool> {
    let mut out = vec![false; render.nodes.len()];
    // Préordre : un parent précède ses enfants.
    for (index, entry) in render.nodes.iter().enumerate() {
        let own = doc.node(&entry.id).is_some_and(|n| n.a11y.hidden);
        out[index] = own || entry.parent.is_some_and(|p| out[p]);
    }
    out
}

fn valid_lang(lang: &str) -> bool {
    let mut parts = lang.split('-');
    let primary = parts.next().unwrap_or_default();
    (2..=3).contains(&primary.len())
        && primary.chars().all(|c| c.is_ascii_lowercase())
        && parts.all(|p| (2..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// Nœuds retirés de l'arbre d'accessibilité là où ils sont écrits : `a11y.hidden` posé sur eux ou
/// sur un ancêtre rendu. Le contenu d'un slot est rendu à la place du slot ciblé dans l'arbre du
/// composant, la racine d'une page à la place du slot `page` de son layout : ils héritent du
/// masquage de ce slot. Ne sert qu'aux nœuds jamais rendus : un nœud rendu hérite du masquage de
/// ses ancêtres rendus (`masked`).
fn hidden_nodes(ctx: &Context<'_>) -> BTreeSet<NodeId> {
    let mut hiding = Hiding {
        ctx,
        known: BTreeMap::new(),
        pending: BTreeSet::new(),
        slots: BTreeMap::new(),
    };
    ctx.doc
        .nodes
        .values()
        .filter(|n| ctx.owner(&n.id).is_some() && hiding.hidden(n))
        .map(|n| n.id.clone())
        .collect()
}

/// Calcul du masquage des nœuds dans l'arbre où ils sont écrits.
struct Hiding<'c> {
    ctx: &'c Context<'c>,
    /// Résultats acquis.
    known: BTreeMap<NodeId, bool>,
    /// Nœuds en cours d'évaluation (garde contre les cycles de parents et les composants
    /// récursifs, signalés par le schéma).
    pending: BTreeSet<NodeId>,
    /// Masquage des slots d'un nom donné dans l'arbre d'une racine.
    slots: BTreeMap<(NodeId, String), bool>,
}

impl Hiding<'_> {
    fn hidden(&mut self, node: &Node) -> bool {
        if node.a11y.hidden {
            return true;
        }
        if let Some(&known) = self.known.get(&node.id) {
            return known;
        }
        if !self.pending.insert(node.id.clone()) {
            return false;
        }
        let doc = self.ctx.doc;
        let hidden = match node.parent.as_ref().and_then(|p| doc.node(p)) {
            // Masquage du parent ; le contenu d'un slot hérite en plus de celui du slot ciblé,
            // à la place duquel il est rendu dans l'arbre du composant.
            Some(parent) => {
                self.hidden(parent)
                    || match &parent.kind {
                        NodeKind::ComponentInstance(props) => {
                            let target = node.meta.slot.as_deref().unwrap_or(DEFAULT_SLOT);
                            doc.component(&props.component)
                                .is_some_and(|c| self.slots_hidden(&c.root, target))
                        }
                        _ => false,
                    }
            }
            // Racine de page : rendue à la place du slot `page` de son layout.
            None => match self.ctx.owner(&node.id) {
                Some(Owner::Page(page)) => doc
                    .page(page)
                    .and_then(|p| p.layout.as_ref())
                    .and_then(|l| doc.layout(l))
                    .is_some_and(|l| self.slots_hidden(&l.root, PAGE_SLOT)),
                _ => false,
            },
        };
        self.pending.remove(&node.id);
        self.known.insert(node.id.clone(), hidden);
        hidden
    }

    /// Vrai si l'arbre de `root` a des slots nommés `name` et qu'ils sont tous masqués (un
    /// contenu qui ne cible aucun slot est signalé par le schéma).
    fn slots_hidden(&mut self, root: &NodeId, name: &str) -> bool {
        let key = (root.clone(), name.to_owned());
        if let Some(&known) = self.slots.get(&key) {
            return known;
        }
        let doc = self.ctx.doc;
        let slots: Vec<&Node> = doc
            .subtree(root)
            .iter()
            .filter_map(|id| doc.node(id))
            .filter(|n| matches!(&n.kind, NodeKind::Slot(slot) if slot.name == name))
            .collect();
        let hidden = !slots.is_empty() && slots.into_iter().all(|slot| self.hidden(slot));
        self.slots.insert(key, hidden);
        hidden
    }
}

/// Vrai si le contenu rendu du nœud fournit un nom accessible : texte visible, image avec `alt`,
/// icône ou image étiquetée. Les instances sont développées (arbre du composant avec les valeurs
/// des props, contenu des slots) ; un sous-arbre masqué ne compte pas.
fn has_named_content(doc: &Document, id: &NodeId, context: &[NodeId]) -> bool {
    let Some(node) = doc.node(id) else { return false };
    let frames = context
        .iter()
        .filter_map(|i| {
            let instance = doc.node(i)?;
            let NodeKind::ComponentInstance(props) = &instance.kind else {
                return None;
            };
            Some(Frame {
                instance,
                props,
                component: doc.component(&props.component)?,
            })
        })
        .collect();
    let mut search = NameSearch {
        doc,
        frames,
        seen: BTreeSet::new(),
    };
    node.children.iter().any(|child| search.named(child))
}

/// Instance en cours de développement : ses props résolvent les liaisons du composant, ses
/// enfants remplissent les slots.
struct Frame<'d> {
    instance: &'d Node,
    props: &'d InstanceProps,
    component: &'d Component,
}

/// Recherche d'un nom accessible dans le contenu rendu.
struct NameSearch<'d> {
    doc: &'d Document,
    /// Instances développées, de la plus externe à la plus interne.
    frames: Vec<Frame<'d>>,
    /// Nœuds déjà explorés, avec les instances qui les développent (garde contre les cycles).
    seen: BTreeSet<(NodeId, Vec<NodeId>)>,
}

impl<'d> NameSearch<'d> {
    fn named(&mut self, id: &NodeId) -> bool {
        let Some(node) = self.doc.node(id) else { return false };
        let context = self.frames.iter().map(|f| f.instance.id.clone()).collect();
        if node.a11y.hidden || !self.seen.insert((id.clone(), context)) {
            return false;
        }
        match &node.kind {
            NodeKind::Text(text) => match self.bound(node, BindableField::Text) {
                Some(PropValue::Text(value)) => !value.trim().is_empty(),
                _ => !text.plain_text().trim().is_empty(),
            },
            NodeKind::Image(image) => {
                non_empty(&node.a11y.label)
                    || match self.bound(node, BindableField::ImageAlt) {
                        Some(PropValue::Text(alt)) => !alt.trim().is_empty(),
                        _ => !image.alt.trim().is_empty(),
                    }
            }
            NodeKind::Icon(_) => non_empty(&node.a11y.label),
            NodeKind::ComponentInstance(props) => non_empty(&node.a11y.label) || self.instance(node, props),
            NodeKind::Slot(slot) => self.slot(&slot.name),
            _ => node.children.iter().any(|child| self.named(child)),
        }
    }

    /// Valeur d'un champ lié à une prop du composant développé : surcharge de l'instance, sinon
    /// valeur par défaut de la prop.
    fn bound(&self, node: &Node, field: BindableField) -> Option<&'d PropValue> {
        let frame = self.frames.last()?;
        let (component, props) = (frame.component, frame.props);
        let prop = component
            .props
            .iter()
            .find(|p| p.binding.node == node.id && p.binding.field == field)?;
        Some(
            props
                .overrides
                .iter()
                .find(|o| o.prop == prop.name)
                .map_or(&prop.default, |o| &o.value),
        )
    }

    /// Arbre du composant d'une instance. Un composant déjà en cours de développement n'est pas
    /// repris (récursion signalée par le schéma).
    fn instance(&mut self, node: &'d Node, props: &'d InstanceProps) -> bool {
        let Some(component) = self.doc.component(&props.component) else {
            return false;
        };
        if self.frames.iter().any(|f| f.component.id == component.id) {
            return false;
        }
        self.frames.push(Frame {
            instance: node,
            props,
            component,
        });
        let named = self.named(&component.root);
        self.frames.pop();
        named
    }

    /// Contenu d'un slot : enfants de l'instance qui le ciblent, explorés dans le contexte où
    /// l'instance est placée (un slot hors instance ne rend rien ici).
    fn slot(&mut self, name: &str) -> bool {
        let Some(frame) = self.frames.pop() else {
            return false;
        };
        let doc = self.doc;
        let named = frame.instance.children.iter().any(|child| {
            let target = doc
                .node(child)
                .and_then(|c| c.meta.slot.as_deref())
                .unwrap_or(DEFAULT_SLOT);
            target == name && self.named(child)
        });
        self.frames.push(frame);
        named
    }
}

fn non_empty(text: &Option<String>) -> bool {
    text.as_deref().is_some_and(|t| !t.trim().is_empty())
}

/// Valeur effective d'un champ lié à une prop, avec l'instance qui la surcharge (`None` : valeur
/// par défaut de la prop).
type BoundField<'d> = dyn Fn(BindableField) -> Option<(&'d PropValue, Option<&'d NodeId>)> + 'd;

/// Texte effectif d'un champ : valeur liée à une prop (avec l'instance qui la surcharge), sinon
/// valeur écrite sur le nœud.
fn text_field<'d>(
    bound: &BoundField<'d>,
    field: BindableField,
    written: Option<&'d str>,
) -> (Option<&'d str>, Option<&'d NodeId>) {
    match bound(field) {
        Some((PropValue::Text(value), by)) => (Some(value.as_str()), by),
        _ => (written, None),
    }
}

fn non_empty_str(text: Option<&str>) -> bool {
    text.is_some_and(|t| !t.trim().is_empty())
}

/// Règles d'un nœud exposé aux technologies d'assistance (les nœuds masqués sont écartés avant).
/// `context` : instances développées autour du nœud ; `bound` : champs liés à des props. Le
/// problème est signalé sur l'instance dont la surcharge le cause, sinon sur le nœud.
fn node_rules<'d>(
    ctx: &Context<'d>,
    node: &'d Node,
    context: &[NodeId],
    bound: &BoundField<'d>,
    issues: &mut Vec<Issue>,
) {
    let id = &node.id;
    match &node.kind {
        NodeKind::Image(image) => {
            let (alt, by) = text_field(bound, BindableField::ImageAlt, Some(image.alt.as_str()));
            if !non_empty_str(alt) {
                issues.push(Issue::error(
                    C::A11yImgAlt,
                    Some(by.unwrap_or(id)),
                    "image needs alt text (or be marked hidden if decorative)",
                ));
            }
        }
        NodeKind::Button(button) => {
            let (label, by) = text_field(bound, BindableField::Label, button.label.as_deref());
            if !(non_empty_str(label) || non_empty(&node.a11y.label) || has_named_content(ctx.doc, id, context)) {
                issues.push(Issue::error(
                    C::A11yAccessibleName,
                    Some(by.unwrap_or(id)),
                    "button has no accessible name",
                ));
            }
        }
        NodeKind::Link(link) => {
            let (label, by) = text_field(bound, BindableField::Label, link.label.as_deref());
            if !(non_empty_str(label) || non_empty(&node.a11y.label) || has_named_content(ctx.doc, id, context)) {
                issues.push(Issue::error(
                    C::A11yAccessibleName,
                    Some(by.unwrap_or(id)),
                    "link has no accessible name",
                ));
            }
        }
        NodeKind::Input(_) => {
            let labelled = ctx.doc.nodes.values().any(|n| {
                matches!(&n.kind, NodeKind::Text(t) if matches!(&t.role, TextRole::Label { for_input: Some(target) } if target == id))
            });
            if !(labelled || non_empty(&node.a11y.label)) {
                issues.push(Issue::error(
                    C::A11yAccessibleName,
                    Some(id),
                    "input has no label (Label text or a11y label)",
                ));
            }
        }
        NodeKind::Icon(_) if !non_empty(&node.a11y.label) => {
            issues.push(Issue::error(
                C::A11yAccessibleName,
                Some(id),
                "meaningful icon needs an a11y label (or be marked hidden)",
            ));
        }
        _ => {}
    }
}
