//! Accessibilité : alt, titres, noms accessibles, landmark `Main`, langue.
//!
//! `a11y.hidden` (aria-hidden) retire le nœud et tout ce qu'il rend de l'arbre d'accessibilité :
//! son sous-arbre, l'arbre du composant d'une instance, le contenu d'un slot.

use std::collections::{BTreeMap, BTreeSet};

use crate::document::{BindableField, Component, Document, Owner, Page};
use crate::id::NodeId;
use crate::node::{ContainerRole, DEFAULT_SLOT, InstanceProps, Node, NodeKind, PAGE_SLOT, PropValue, TextRole};
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
    let hidden = hidden_nodes(ctx);
    for node in doc.nodes.values() {
        if ctx.owner(&node.id).is_none() || hidden.contains(&node.id) {
            continue;
        }
        node_rules(ctx, node, issues);
    }
    for page in &doc.pages {
        let mut h1 = 0;
        let mut previous = 0u8;
        let mut mains = 0;
        // Un titre ou un `Main` masqué n'existe pas pour les technologies d'assistance.
        for id in &exposed_render(ctx, page, &hidden) {
            let Some(node) = doc.node(id) else { continue };
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
/// masquage de ce slot. L'arbre d'un composant, validé une seule fois pour toutes ses instances,
/// n'hérite pas du masquage d'une instance ; le rendu de chaque page en tient compte
/// (`exposed_render`).
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

/// Nœuds exposés d'une page, dans l'ordre de rendu. `Context::render_order` donne le rendu en
/// préordre ; le parent rendu de chaque nœud y est retrouvé parmi les nœuds encore ouverts
/// (enfant d'un élément, racine du composant d'une instance, contenu d'un slot, racine de la page
/// dans le slot `page` du layout), pour qu'il hérite de son masquage : l'arbre du composant d'une
/// instance masquée, ou placée sous un conteneur masqué, n'existe pas pour les technologies
/// d'assistance.
fn exposed_render(ctx: &Context<'_>, page: &Page, hidden: &BTreeSet<NodeId>) -> Vec<NodeId> {
    let doc = ctx.doc;
    let mut open: Vec<Open<'_>> = Vec::new();
    let mut out = Vec::new();
    for id in ctx.render_order(page) {
        let Some(node) = doc.node(&id) else { continue };
        let placed = loop {
            let Some(parent) = open.last_mut() else {
                break None;
            };
            if let Some((position, frames)) = parent.place(doc, page, node) {
                parent.next = position + 1;
                break Some((parent.hidden, frames));
            }
            open.pop();
        };
        // Sans parent rendu (racine du rendu) : masquage de l'arbre où le nœud est écrit.
        let (masked, frames) = match placed {
            Some((parent, frames)) => (parent || node.a11y.hidden, frames),
            None => (hidden.contains(&id), Vec::new()),
        };
        if !masked {
            out.push(id);
        }
        open.push(Open {
            node,
            hidden: masked,
            frames,
            next: 0,
        });
    }
    out
}

/// Nœud rendu dont la descendance n'est peut-être pas encore entièrement rendue.
struct Open<'d> {
    node: &'d Node,
    /// Masqué lui-même ou par un ancêtre rendu.
    hidden: bool,
    /// Instances développées autour du nœud, de la plus externe à celle dont le composant le
    /// contient.
    frames: Vec<&'d Node>,
    /// Rang, parmi ce que le nœud rend, à partir duquel un enfant peut encore venir : ce qu'il a
    /// déjà rendu ne revient pas sous lui (un même nœud peut revenir sous une autre instance).
    next: usize,
}

impl<'d> Open<'d> {
    /// Rang de `child` parmi ce que ce nœud rend, avec les instances développées autour de lui ;
    /// `None` si ce nœud ne peut pas, ou plus, le rendre.
    fn place(&self, doc: &'d Document, page: &Page, child: &Node) -> Option<(usize, Vec<&'d Node>)> {
        let (position, frames) = match &self.node.kind {
            // Racine du composant de l'instance.
            NodeKind::ComponentInstance(props) => {
                if doc.component(&props.component)?.root != child.id {
                    return None;
                }
                let mut frames = self.frames.clone();
                frames.push(self.node);
                (0, frames)
            }
            // Racine de la page, à la place du slot `page` du layout.
            NodeKind::Slot(slot) if slot.name == PAGE_SLOT && child.id == page.root => (0, Vec::new()),
            // Contenu du slot : enfants de l'instance qui le ciblent, rendus dans le contexte qui
            // entoure l'instance.
            NodeKind::Slot(slot) => {
                let (instance, outer) = self.frames.split_last()?;
                if child.meta.slot.as_deref().unwrap_or(DEFAULT_SLOT) != slot.name {
                    return None;
                }
                let position = instance.children.iter().position(|c| *c == child.id)?;
                (position, outer.to_vec())
            }
            _ => {
                let position = self.node.children.iter().position(|c| *c == child.id)?;
                (position, self.frames.clone())
            }
        };
        (position >= self.next).then_some((position, frames))
    }
}

/// Vrai si le contenu rendu du nœud fournit un nom accessible : texte visible, image avec `alt`,
/// icône ou image étiquetée. Les instances sont développées (arbre du composant avec les valeurs
/// des props, contenu des slots) ; un sous-arbre masqué ne compte pas.
fn has_named_content(doc: &Document, id: &NodeId) -> bool {
    let Some(node) = doc.node(id) else { return false };
    let mut search = NameSearch {
        doc,
        frames: Vec::new(),
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

/// Règles d'un nœud exposé aux technologies d'assistance (les nœuds masqués sont écartés avant).
fn node_rules(ctx: &Context<'_>, node: &Node, issues: &mut Vec<Issue>) {
    let id = &node.id;
    match &node.kind {
        NodeKind::Image(image) if image.alt.trim().is_empty() => {
            issues.push(Issue::error(
                C::A11yImgAlt,
                Some(id),
                "image needs alt text (or be marked hidden if decorative)",
            ));
        }
        NodeKind::Button(button) => {
            if !(non_empty(&button.label) || non_empty(&node.a11y.label) || has_named_content(ctx.doc, id)) {
                issues.push(Issue::error(
                    C::A11yAccessibleName,
                    Some(id),
                    "button has no accessible name",
                ));
            }
        }
        NodeKind::Link(link) => {
            if !(non_empty(&link.label) || non_empty(&node.a11y.label) || has_named_content(ctx.doc, id)) {
                issues.push(Issue::error(
                    C::A11yAccessibleName,
                    Some(id),
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
