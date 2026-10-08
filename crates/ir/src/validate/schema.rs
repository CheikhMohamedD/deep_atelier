//! Règles de schéma : arbre, imbrication, style par primitive, références, routes, noms.
//!
//! Les règles qui dépendent du contexte de rendu (élément parent d'un élément de liste, éléments
//! interactifs imbriqués, ancres, hôte des propriétés de placement) jugent un nœud là où les pages
//! le rendent : une racine de composant à la place de chacune de ses instances, le contenu d'un
//! slot à la place du slot. Un nœud jamais rendu est jugé dans l'arbre où il est écrit.

use std::collections::{BTreeMap, BTreeSet};

use crate::document::{BindableField, Component, Document, Owner, Page, RouteSegment, route_path};
use crate::id::{ComponentId, NodeId, is_camel_case, is_kebab_case};
use crate::node::{ContainerRole, DEFAULT_SLOT, Href, ImageSource, Node, NodeKind, PAGE_SLOT, PropValue, TextRole};
use crate::query::{node_colors, style_patch_colors};
use crate::render::{PARENT_PROPS, RenderTree, is_transparent, own_prop_allowed, placement_allowed, rendered_hosts};
use crate::style::color::ColorSource;
use crate::style::style::{InteractionState, PropChange, ResponsiveValue, ResponsiveValuePatch, StyleProp};
use crate::style::values::Size;
use crate::tokens::FontFamily;
use crate::validate::{Context, Issue, IssueCode as C, is_known_icon};

/// Noms qu'une prop, un axe de variantes ou un slot ne peut pas prendre : props React réservées
/// ou passées par le compilateur à la racine du composant.
const RESERVED_PROP_NAMES: [&str; 4] = ["children", "className", "key", "ref"];

pub(super) fn check(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    document_level(ctx.doc, issues);
    tree(ctx, issues);
    for node in ctx.doc.nodes.values() {
        node_rules(ctx, node, issues);
    }
    for (page, render) in &ctx.renders {
        rendered_rules(ctx, page, render, issues);
    }
    pages(ctx, issues);
    layouts(ctx, issues);
    components(ctx, issues);
    tokens(ctx.doc, issues);
}

fn document_level(doc: &Document, issues: &mut Vec<Issue>) {
    if doc.version != crate::document::IR_VERSION {
        issues.push(Issue::error(
            C::InvalidVersion,
            None,
            format!("unsupported IR version {}", doc.version),
        ));
    }
    if !doc.targets.contains(&crate::document::Target::Web) {
        issues.push(Issue::error(C::NoWebTarget, None, "the document must target the web"));
    }
    if doc.pages.is_empty() {
        issues.push(Issue::error(
            C::NoPage,
            None,
            "the document must contain at least one page",
        ));
    }
    let duplicates = |ids: Vec<String>, what: &str, issues: &mut Vec<Issue>| {
        let mut seen = BTreeSet::new();
        for id in ids {
            if !seen.insert(id.clone()) {
                issues.push(Issue::error(
                    C::DuplicateId,
                    None,
                    format!("duplicate {what} id `{id}`"),
                ));
            }
        }
    };
    duplicates(doc.pages.iter().map(|p| p.id.to_string()).collect(), "page", issues);
    duplicates(doc.layouts.iter().map(|l| l.id.to_string()).collect(), "layout", issues);
    duplicates(
        doc.components.iter().map(|c| c.id.to_string()).collect(),
        "component",
        issues,
    );
    duplicates(doc.assets.iter().map(|a| a.id.to_string()).collect(), "asset", issues);
    if let Some(favicon) = &doc.settings.favicon {
        match doc.asset(favicon) {
            None => issues.push(Issue::error(
                C::InvalidReference,
                None,
                format!("favicon asset `{favicon}` does not exist"),
            )),
            Some(asset) if !asset.mime.starts_with("image/") => issues.push(Issue::error(
                C::InvalidReference,
                None,
                format!("favicon asset `{favicon}` is not an image (`{}`)", asset.mime),
            )),
            Some(_) => {}
        }
    }
}

fn tree(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let mut root_count: BTreeMap<NodeId, usize> = BTreeMap::new();
    for (owner, root) in doc.roots() {
        *root_count.entry(root.clone()).or_default() += 1;
        match doc.node(&root) {
            None => issues.push(Issue::error(
                C::InvalidReference,
                None,
                format!("root node `{root}` does not exist"),
            )),
            Some(node) if node.parent.is_some() => issues.push(Issue::error(
                C::TreeInconsistent,
                Some(&root),
                "a root node must not have a parent",
            )),
            Some(node) if !matches!(owner, Owner::Component(_)) && node.kind.container().is_none() => {
                issues.push(Issue::error(
                    C::RootNotContainer,
                    Some(&root),
                    format!(
                        "the root of a page or layout must be a Box, Stack or Grid, not {}",
                        node.kind.type_name()
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    for (root, count) in root_count {
        if count > 1 {
            issues.push(Issue::error(
                C::TreeInconsistent,
                Some(&root),
                "node is the root of several entities",
            ));
        }
    }
    for (key, node) in &doc.nodes {
        if *key != node.id {
            issues.push(Issue::error(
                C::TreeInconsistent,
                Some(key),
                format!("node stored under `{key}` has id `{}`", node.id),
            ));
        }
        let mut seen = BTreeSet::new();
        for child in &node.children {
            if !seen.insert(child) {
                issues.push(Issue::error(
                    C::TreeInconsistent,
                    Some(key),
                    format!("child `{child}` is listed twice"),
                ));
            }
            match doc.node(child) {
                None => issues.push(Issue::error(
                    C::TreeInconsistent,
                    Some(key),
                    format!("child `{child}` does not exist"),
                )),
                Some(c) if c.parent.as_ref() != Some(key) => issues.push(Issue::error(
                    C::TreeInconsistent,
                    Some(child),
                    format!("node is listed as a child of `{key}` but points to another parent"),
                )),
                Some(_) => {}
            }
        }
        if let Some(parent) = &node.parent {
            match doc.node(parent) {
                None => issues.push(Issue::error(
                    C::TreeInconsistent,
                    Some(key),
                    format!("parent `{parent}` does not exist"),
                )),
                Some(p) if !p.children.contains(key) => issues.push(Issue::error(
                    C::TreeInconsistent,
                    Some(key),
                    format!("parent `{parent}` does not list this node"),
                )),
                Some(_) => {}
            }
        }
        if ctx.owner(key).is_none() {
            // Hors de tout arbre enregistré : sur un cycle de parents, accroché à un cycle, ou
            // orphelin (une seule de ces causes est signalée).
            let mut current = node.parent.clone();
            let mut steps = 0;
            let mut cycle = None;
            while let Some(p) = current {
                steps += 1;
                if p == *key {
                    cycle = Some("node is part of a parent cycle");
                    break;
                }
                if steps > doc.nodes.len() {
                    cycle = Some("node hangs from a parent cycle");
                    break;
                }
                current = doc.node(&p).and_then(|n| n.parent.clone());
            }
            match cycle {
                Some(message) => issues.push(Issue::error(C::Cycle, Some(key), message)),
                None => issues.push(Issue::error(
                    C::OrphanNode,
                    Some(key),
                    "node is not reachable from any page, layout or component",
                )),
            }
        }
    }
}

fn node_rules(ctx: &Context<'_>, node: &Node, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let id = &node.id;
    let owner = ctx.owner(id);
    let parent = node.parent.as_ref().and_then(|p| doc.node(p));
    let rendered = ctx.is_rendered(id);

    // Feuilles et conteneurs.
    if !node.children.is_empty() && !node.kind.accepts_children() {
        issues.push(Issue::error(
            C::LeafHasChildren,
            Some(id),
            format!("{} cannot have children", node.kind.type_name()),
        ));
    }
    match &node.kind {
        NodeKind::Button(b) if b.label.is_none() && node.children.is_empty() => {
            issues.push(Issue::error(
                C::EmptyInteractive,
                Some(id),
                "button has neither a label nor children",
            ));
        }
        NodeKind::Link(l) if l.label.is_none() && node.children.is_empty() => {
            issues.push(Issue::error(
                C::EmptyInteractive,
                Some(id),
                "link has neither a label nor children",
            ));
        }
        _ => {}
    }
    // Un nœud rendu est jugé à chacun de ses rendus (`rendered_rules`) ; les autres dans
    // l'arbre où ils sont écrits, quand l'élément parent y est connu.
    if !rendered {
        unrendered_nesting(doc, node, parent, issues);
    }
    if matches!(node.kind, NodeKind::Slot(_)) && matches!(owner, Some(Owner::Page(_))) {
        issues.push(Issue::error(
            C::SlotOutsideComponent,
            Some(id),
            "slots are only allowed in components and layouts",
        ));
    }
    if let NodeKind::Slot(slot) = &node.kind
        && matches!(owner, Some(Owner::Component(_)))
    {
        if slot.name == PAGE_SLOT {
            issues.push(Issue::error(
                C::InvalidName,
                Some(id),
                "slot name `page` is reserved for layouts",
            ));
        } else if !is_camel_case(&slot.name) {
            issues.push(Issue::error(
                C::InvalidName,
                Some(id),
                format!("slot name `{}` must be camelCase", slot.name),
            ));
        }
    }
    if let Some(slot) = &node.meta.slot
        && !matches!(parent.map(|p| &p.kind), Some(NodeKind::ComponentInstance(_)))
    {
        issues.push(Issue::error(
            C::InvalidSlotTarget,
            Some(id),
            format!("slot `{slot}` is set but the parent is not an instance"),
        ));
    }

    // Style permis selon la primitive, et selon chacun de ses hôtes au rendu pour le placement
    // (mêmes règles que les commandes ; un nœud jamais rendu n'impose aucune contrainte de
    // placement).
    let mut hosts = None;
    for (prop, value) in node.style.entries() {
        let allowed = own_prop_allowed(prop, &node.kind)
            && (!PARENT_PROPS.contains(&prop)
                || hosts
                    .get_or_insert_with(|| rendered_hosts(doc, id))
                    .iter()
                    .all(|host| placement_allowed(prop, *host)));
        if !allowed {
            issues.push(Issue::error(
                C::StyleNotAllowed,
                Some(id),
                format!("`{}` is not allowed on {} here", prop.name(), node.kind.type_name()),
            ));
        }
        if let ResponsiveValue::Size(size) = &value
            && !matches!(prop, StyleProp::MaxWidth | StyleProp::MaxHeight)
            && size.values().any(|s| *s == Size::None)
        {
            issues.push(Issue::error(
                C::StyleNotAllowed,
                Some(id),
                format!("`none` is only valid for max sizes, not `{}`", prop.name()),
            ));
        }
    }

    // Références.
    for color in node_colors(node) {
        if let ColorSource::Token(name) = &color.source
            && doc.tokens.color(name).is_none()
        {
            issues.push(Issue::error(
                C::InvalidReference,
                Some(id),
                format!("color token `{name}` does not exist"),
            ));
        }
    }
    if let Some(fonts) = &node.style.font_family {
        for name in fonts.values() {
            if doc.tokens.font(name).is_none() {
                issues.push(Issue::error(
                    C::InvalidReference,
                    Some(id),
                    format!("font token `{name}` does not exist"),
                ));
            }
        }
    }
    let same_owner = |target: &NodeId| ctx.owner(target) == owner;
    match &node.kind {
        NodeKind::Text(text) => {
            if let TextRole::Label {
                for_input: Some(target),
            } = &text.role
            {
                let ok = doc.node(target).is_some_and(|t| matches!(t.kind, NodeKind::Input(_))) && same_owner(target);
                if !ok {
                    issues.push(Issue::error(
                        C::InvalidReference,
                        Some(id),
                        format!("label target `{target}` is not an input of the same tree"),
                    ));
                }
            }
        }
        NodeKind::Image(image) => {
            image_source_rules(doc, id, &image.source, issues);
            match image.intrinsic {
                None if matches!(image.source, ImageSource::Asset { .. } | ImageSource::Url { .. }) => {
                    issues.push(Issue::error(
                        C::MissingIntrinsic,
                        Some(id),
                        "images from assets or URLs need intrinsic dimensions",
                    ));
                }
                Some(size) if size.width == 0 || size.height == 0 => {
                    issues.push(Issue::error(
                        C::MissingIntrinsic,
                        Some(id),
                        "intrinsic dimensions must be positive",
                    ));
                }
                _ => {}
            }
        }
        NodeKind::Icon(icon) if !is_known_icon(&icon.name) => {
            issues.push(Issue::error(
                C::UnknownIcon,
                Some(id),
                format!("unknown icon `{}`", icon.name),
            ));
        }
        NodeKind::Button(button) => {
            if let Some(crate::node::Action::ToggleVisibility { target }) = &button.action
                && !(doc.node(target).is_some() && same_owner(target))
            {
                issues.push(Issue::error(
                    C::InvalidReference,
                    Some(id),
                    format!("action target `{target}` is not in the same tree"),
                ));
            }
        }
        NodeKind::Input(input) if input.name.trim().is_empty() => {
            issues.push(Issue::error(C::InvalidProp, Some(id), "input name must not be empty"));
        }
        NodeKind::Link(link) => {
            href_target_rules(doc, id, &link.href, issues);
            // Les ancres d'un nœud rendu sont vérifiées sur chaque page qui le rend.
            if !rendered {
                unrendered_anchor_rules(ctx, id, owner, &link.href, issues);
            }
        }
        NodeKind::ComponentInstance(instance) => instance_rules(ctx, node, &instance.component, issues),
        _ => {}
    }

    if let Some(anchor) = &node.meta.anchor
        && !is_kebab_case(anchor)
    {
        issues.push(Issue::error(
            C::InvalidAnchor,
            Some(id),
            format!("anchor `{anchor}` must be kebab-case"),
        ));
    }
}

/// Imbrication d'un nœud jamais rendu, jugée dans l'arbre où il est écrit quand l'élément parent
/// y est connu (parent qui n'est ni une instance ni un slot).
fn unrendered_nesting(doc: &Document, node: &Node, parent: Option<&Node>, issues: &mut Vec<Issue>) {
    let id = &node.id;
    if node.kind.is_interactive()
        && doc
            .ancestors(id)
            .iter()
            .any(|a| doc.node(a).is_some_and(|n| n.kind.is_interactive()))
    {
        issues.push(Issue::error(
            C::NestedInteractive,
            Some(id),
            "interactive element nested inside another one",
        ));
    }
    let Some(parent) = parent.filter(|p| !is_transparent(doc, &p.id)) else {
        return;
    };
    if is_list_item(node) && !is_list(parent) {
        issues.push(Issue::error(
            C::ListItemOutsideList,
            Some(id),
            "list item must be a direct child of a list",
        ));
    }
    if is_list(parent) && !is_transparent(doc, id) && !may_be_list_child(node) {
        issues.push(Issue::error(
            C::ListChildNotItem,
            Some(id),
            "a list can only contain list items",
        ));
    }
}

fn is_list(node: &Node) -> bool {
    node.kind
        .container()
        .is_some_and(|(_, p)| p.role == ContainerRole::List)
}

fn is_list_item(node: &Node) -> bool {
    node.kind
        .container()
        .is_some_and(|(_, p)| p.role == ContainerRole::ListItem)
}

/// Enfant permis d'une liste : élément de liste, ou code libre (qui peut en rendre un).
fn may_be_list_child(node: &Node) -> bool {
    is_list_item(node) || matches!(node.kind, NodeKind::RawCode(_))
}

/// Règles jugées sur le rendu d'une page : éléments de liste, contenu des listes, éléments
/// interactifs imbriqués, ancres (doublons et liens). Le problème est signalé sur le nœud placé
/// dans l'élément parent (l'instance qui rend une racine de composant).
fn rendered_rules(ctx: &Context<'_>, page: &Page, render: &RenderTree, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let at = |index: usize| -> NodeId { render.nodes[render.placement(doc, index)].id.clone() };
    let anchors = ctx.page_anchors(page);
    let mut seen_anchors: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, entry) in render.nodes.iter().enumerate() {
        let Some(node) = doc.node(&entry.id) else { continue };
        let element_parent = render
            .element_parent(doc, index)
            .and_then(|p| doc.node(&render.nodes[p].id));
        if is_list_item(node) && !element_parent.is_some_and(is_list) {
            issues.push(Issue::error(
                C::ListItemOutsideList,
                Some(&at(index)),
                "list item is rendered outside a list",
            ));
        }
        if is_list(node) {
            for child in render.element_children(doc, index) {
                if doc.node(&render.nodes[child].id).is_some_and(|c| !may_be_list_child(c)) {
                    issues.push(Issue::error(
                        C::ListChildNotItem,
                        Some(&at(child)),
                        "a list can only contain list items",
                    ));
                }
            }
        }
        if node.kind.is_interactive() {
            let nested = render
                .ancestors(index)
                .any(|a| doc.node(&render.nodes[a].id).is_some_and(|n| n.kind.is_interactive()));
            if nested {
                issues.push(Issue::error(
                    C::NestedInteractive,
                    Some(&at(index)),
                    "interactive element is rendered inside another one",
                ));
            }
        }
        if let Some(anchor) = &node.meta.anchor {
            let count = seen_anchors.entry(anchor.as_str()).or_default();
            *count += 1;
            if *count == 2 {
                issues.push(Issue::error(
                    C::DuplicateAnchor,
                    Some(&entry.id),
                    format!("anchor `{anchor}` is rendered twice on page `{}`", page.name),
                ));
            }
        }
        if let NodeKind::Link(link) = &node.kind {
            // Lien effectif : surcharge ou défaut d'une prop liée, sinon celui du nœud.
            let bound = render.bound(doc, index, BindableField::Href);
            let (href, culprit) = match bound {
                Some(b) => match b.value {
                    PropValue::Href(href) => (href, b.overridden_by.cloned().unwrap_or_else(|| entry.id.clone())),
                    _ => (&link.href, entry.id.clone()),
                },
                None => (&link.href, entry.id.clone()),
            };
            match href {
                Href::Anchor { anchor } if !anchors.contains(anchor) => issues.push(Issue::error(
                    C::InvalidReference,
                    Some(&culprit),
                    format!("anchor `{anchor}` does not exist on page `{}`", page.name),
                )),
                Href::Page {
                    page: target,
                    anchor: Some(anchor),
                } => {
                    if let Some(target) = doc.page(target)
                        && !ctx.page_anchors(target).contains(anchor)
                    {
                        issues.push(Issue::error(
                            C::InvalidReference,
                            Some(&culprit),
                            format!("anchor `{anchor}` does not exist on page `{}`", target.name),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
}

/// Ancres d'un lien jamais rendu, vérifiées sur la page qui le contient s'il y en a une.
fn unrendered_anchor_rules(
    ctx: &Context<'_>,
    id: &NodeId,
    owner: Option<&Owner>,
    href: &Href,
    issues: &mut Vec<Issue>,
) {
    let (page, anchor) = match href {
        Href::Page {
            page,
            anchor: Some(anchor),
        } => (ctx.doc.page(page), anchor),
        Href::Anchor { anchor } => match owner {
            Some(Owner::Page(page)) => (ctx.doc.page(page), anchor),
            _ => return,
        },
        _ => return,
    };
    if let Some(page) = page
        && !ctx.page_anchors(page).contains(anchor)
    {
        issues.push(Issue::error(
            C::InvalidReference,
            Some(id),
            format!("anchor `{anchor}` does not exist on page `{}`", page.name),
        ));
    }
}

/// Cible d'un lien : page existante, URL externe absolue.
fn href_target_rules(doc: &Document, id: &NodeId, href: &Href, issues: &mut Vec<Issue>) {
    match href {
        Href::Page { page, .. } if doc.page(page).is_none() => issues.push(Issue::error(
            C::InvalidReference,
            Some(id),
            format!("page `{page}` does not exist"),
        )),
        Href::External { url } if !is_http_url(url) => {
            issues.push(Issue::error(
                C::InvalidProp,
                Some(id),
                format!("external URL `{url}` must start with http(s)://"),
            ));
        }
        _ => {}
    }
}

/// URL absolue http(s) avec un hôte.
fn is_http_url(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|r| !r.is_empty() && !r.starts_with('/') && !url.chars().any(char::is_whitespace))
}

/// Source d'image : URL http(s) absolue (`next/image` distant), ou asset image existant.
fn image_source_rules(doc: &Document, id: &NodeId, source: &ImageSource, issues: &mut Vec<Issue>) {
    match source {
        ImageSource::Url { url } if !is_http_url(url) => issues.push(Issue::error(
            C::InvalidProp,
            Some(id),
            format!("image URL `{url}` must be an absolute http(s) URL"),
        )),
        ImageSource::Asset { id: asset } => match doc.asset(asset) {
            None => issues.push(Issue::error(
                C::InvalidReference,
                Some(id),
                format!("asset `{asset}` does not exist"),
            )),
            Some(found) if !found.mime.starts_with("image/") => issues.push(Issue::error(
                C::InvalidReference,
                Some(id),
                format!("asset `{asset}` is not an image (`{}`)", found.mime),
            )),
            Some(_) => {}
        },
        _ => {}
    }
}

/// Valeur de prop (défaut ou surcharge) : mêmes règles que le champ qu'elle remplit.
fn prop_value_rules(doc: &Document, id: &NodeId, value: &PropValue, issues: &mut Vec<Issue>) {
    match value {
        PropValue::Href(href) => href_target_rules(doc, id, href, issues),
        PropValue::Image(source) => image_source_rules(doc, id, source, issues),
        PropValue::Text(_) | PropValue::Bool(_) => {}
    }
}

fn instance_rules(ctx: &Context<'_>, node: &Node, component_id: &ComponentId, issues: &mut Vec<Issue>) {
    let id = &node.id;
    let Some(component) = ctx.doc.component(component_id) else {
        issues.push(Issue::error(
            C::InvalidReference,
            Some(id),
            format!("component `{component_id}` does not exist"),
        ));
        return;
    };
    let NodeKind::ComponentInstance(props) = &node.kind else {
        return;
    };
    let mut seen = BTreeSet::new();
    for over in &props.overrides {
        if !seen.insert(&over.prop) {
            issues.push(Issue::error(
                C::InvalidProp,
                Some(id),
                format!("prop `{}` is overridden twice", over.prop),
            ));
        }
        match component.prop(&over.prop) {
            None => issues.push(Issue::error(
                C::InvalidProp,
                Some(id),
                format!("component `{}` has no prop `{}`", component.name, over.prop),
            )),
            Some(prop) if !prop.binding.field.accepts(&over.value) => issues.push(Issue::error(
                C::InvalidProp,
                Some(id),
                format!("prop `{}` has the wrong type", over.prop),
            )),
            Some(_) => prop_value_rules(ctx.doc, id, &over.value, issues),
        }
    }
    let mut axes = BTreeSet::new();
    for choice in &props.variants {
        if !axes.insert(&choice.axis) {
            issues.push(Issue::error(
                C::InvalidProp,
                Some(id),
                format!("variant axis `{}` is chosen twice", choice.axis),
            ));
        }
        let valid = component
            .axis(&choice.axis)
            .is_some_and(|a| a.options.iter().any(|o| o.name == choice.option));
        if !valid {
            issues.push(Issue::error(
                C::InvalidProp,
                Some(id),
                format!("variant `{}={}` does not exist", choice.axis, choice.option),
            ));
        }
    }
    let slots = component_slots(ctx.doc, component);
    for child in &node.children {
        let target = ctx
            .doc
            .node(child)
            .and_then(|c| c.meta.slot.clone())
            .unwrap_or_else(|| DEFAULT_SLOT.to_owned());
        if !slots.contains(&target) {
            issues.push(Issue::error(
                C::InvalidSlotTarget,
                Some(child),
                format!("component `{}` has no slot `{target}`", component.name),
            ));
        }
    }
}

/// Noms des slots d'un composant (y compris ceux transmis à une instance imbriquée).
fn component_slots(doc: &Document, component: &Component) -> BTreeSet<String> {
    doc.subtree(&component.root)
        .iter()
        .filter_map(|n| match &doc.node(n)?.kind {
            NodeKind::Slot(slot) => Some(slot.name.clone()),
            _ => None,
        })
        .collect()
}

fn pages(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let mut routes: BTreeMap<String, &str> = BTreeMap::new();
    // Nom du paramètre dynamique à chaque emplacement de l'arborescence (`/blog/[]`) : l'App
    // Router exige le même nom pour toutes les routes qui y passent.
    let mut params: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    for page in &doc.pages {
        if page.name.trim().is_empty() {
            issues.push(Issue::error(
                C::InvalidName,
                Some(&page.root),
                "page name must not be empty",
            ));
        }
        let mut names = BTreeSet::new();
        for segment in &page.route {
            let valid = match segment {
                RouteSegment::Static(name) => {
                    !name.is_empty()
                        && name
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                }
                RouteSegment::Param(name) => {
                    name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                }
            };
            if !valid {
                issues.push(Issue::error(
                    C::InvalidRoute,
                    Some(&page.root),
                    format!("invalid route segment in `{}`", route_path(&page.route)),
                ));
            }
            if let RouteSegment::Param(name) = segment
                && !names.insert(name.as_str())
            {
                issues.push(Issue::error(
                    C::InvalidRoute,
                    Some(&page.root),
                    format!("route `{}` uses the parameter `{name}` twice", route_path(&page.route)),
                ));
            }
        }
        let shape = |segments: &[RouteSegment]| -> String {
            segments
                .iter()
                .map(|s| match s {
                    RouteSegment::Static(name) => format!("/{name}"),
                    RouteSegment::Param(_) => "/[]".to_owned(),
                })
                .collect()
        };
        for (rank, segment) in page.route.iter().enumerate() {
            let RouteSegment::Param(name) = segment else { continue };
            let place = shape(&page.route[..=rank]);
            match params.get(&place) {
                Some((other, other_page)) if *other != name.as_str() => issues.push(Issue::error(
                    C::DuplicateRoute,
                    Some(&page.root),
                    format!(
                        "parameter `[{name}]` of `{}` conflicts with `[{other}]` of page `{other_page}` at the same place",
                        route_path(&page.route)
                    ),
                )),
                Some(_) => {}
                None => {
                    params.insert(place, (name.as_str(), page.name.as_str()));
                }
            }
        }
        // Deux paramètres au même rang entrent en conflit dans l'App Router.
        if let Some(other) = routes.insert(shape(&page.route), &page.name) {
            issues.push(Issue::error(
                C::DuplicateRoute,
                Some(&page.root),
                format!("route `{}` conflicts with page `{other}`", route_path(&page.route)),
            ));
        }
        if let Some(layout) = &page.layout
            && doc.layout(layout).is_none()
        {
            issues.push(Issue::error(
                C::InvalidReference,
                Some(&page.root),
                format!("layout `{layout}` does not exist"),
            ));
        }
        if let Some(image) = &page.seo.og_image {
            match doc.asset(image) {
                None => issues.push(Issue::error(
                    C::InvalidReference,
                    Some(&page.root),
                    format!("OG image `{image}` does not exist"),
                )),
                Some(asset)
                    if !matches!(
                        asset.mime.as_str(),
                        "image/png" | "image/jpeg" | "image/gif" | "image/webp"
                    ) =>
                {
                    issues.push(Issue::error(
                        C::InvalidReference,
                        Some(&page.root),
                        format!("OG image `{image}` must be a PNG, JPEG, GIF or WebP image"),
                    ));
                }
                Some(_) => {}
            }
        }
    }
}

fn layouts(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let mut names = BTreeSet::new();
    for layout in &doc.layouts {
        if !is_kebab_case(&layout.name) {
            issues.push(Issue::error(
                C::InvalidName,
                Some(&layout.root),
                format!("layout name `{}` must be kebab-case (route group)", layout.name),
            ));
        }
        if !names.insert(layout.name.clone()) {
            issues.push(Issue::error(
                C::DuplicateName,
                Some(&layout.root),
                format!("layout `{}` already exists", layout.name),
            ));
        }
        let slots: Vec<&str> = doc
            .subtree(&layout.root)
            .iter()
            .filter_map(|n| match &doc.node(n)?.kind {
                NodeKind::Slot(slot) => Some(slot.name.as_str()),
                _ => None,
            })
            .collect();
        let page_slots = slots.iter().filter(|name| **name == PAGE_SLOT).count();
        if page_slots != 1 {
            issues.push(Issue::error(
                C::LayoutPageSlot,
                Some(&layout.root),
                format!(
                    "layout `{}` must contain exactly one `page` slot (found {page_slots})",
                    layout.name
                ),
            ));
        }
        for name in slots.iter().filter(|name| **name != PAGE_SLOT) {
            issues.push(Issue::error(
                C::LayoutPageSlot,
                Some(&layout.root),
                format!(
                    "layout `{}` has a `{name}` slot: layouts only have the `page` slot",
                    layout.name
                ),
            ));
        }
    }
}

fn components(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let mut names = BTreeSet::new();
    for component in &doc.components {
        let root = Some(&component.root);
        let pascal = component.name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && component.name.chars().all(|c| c.is_ascii_alphanumeric());
        if !pascal {
            issues.push(Issue::error(
                C::InvalidName,
                root,
                format!("component name `{}` must be PascalCase", component.name),
            ));
        }
        if !names.insert(component.name.clone()) {
            issues.push(Issue::error(
                C::DuplicateName,
                root,
                format!("component `{}` already exists", component.name),
            ));
        }
        let mut slots = BTreeSet::new();
        for id in doc.subtree(&component.root) {
            if let Some(NodeKind::Slot(slot)) = doc.node(&id).map(|n| &n.kind)
                && !slots.insert(slot.name.clone())
            {
                issues.push(Issue::error(
                    C::DuplicateSlot,
                    Some(&id),
                    format!("slot `{}` is defined twice", slot.name),
                ));
            }
        }
        // Props, axes de variantes et slots deviennent des props TypeScript du composant.
        let mut taken: BTreeMap<&str, &str> = slots.iter().map(|s| (s.as_str(), "slot")).collect();
        let claim = |name: &str, what: &'static str, issues: &mut Vec<Issue>| {
            if let Some(other) = taken.get(name).copied() {
                issues.push(Issue::error(
                    C::DuplicateName,
                    root,
                    format!("{what} `{name}` has the same name as a {other}"),
                ));
            }
        };
        let mut prop_names = BTreeSet::new();
        for prop in &component.props {
            if !prop_names.insert(&prop.name) {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!("prop `{}` is defined twice", prop.name),
                ));
            }
            claim(&prop.name, "prop", issues);
            if !is_camel_case(&prop.name) {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!("prop name `{}` must be camelCase", prop.name),
                ));
            } else if RESERVED_PROP_NAMES.contains(&prop.name.as_str()) && !slots.contains(&prop.name) {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!("prop name `{}` is reserved", prop.name),
                ));
            }
            let target = &prop.binding.node;
            let readable = doc.node(target).and_then(|n| prop.binding.field.read(n)).is_some();
            if !doc.is_within(target, &component.root) || !readable {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!("prop `{}` is bound to an invalid node", prop.name),
                ));
            }
            if !prop.binding.field.accepts(&prop.default) {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!("default of prop `{}` has the wrong type", prop.name),
                ));
            } else {
                prop_value_rules(doc, &component.root, &prop.default, issues);
            }
        }
        let mut bindings = BTreeSet::new();
        for prop in &component.props {
            if !bindings.insert((&prop.binding.node, prop.binding.field)) {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!(
                        "prop `{}` is bound to a field that another prop already fills",
                        prop.name
                    ),
                ));
            }
        }
        let mut axis_names = BTreeSet::new();
        for prop in &component.props {
            taken.entry(prop.name.as_str()).or_insert("prop");
        }
        for axis in &component.variants {
            if !is_camel_case(&axis.name) || RESERVED_PROP_NAMES.contains(&axis.name.as_str()) {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!("variant axis `{}` must be a camelCase, non-reserved name", axis.name),
                ));
            }
            if !axis_names.insert(&axis.name) {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!("variant axis `{}` is defined twice", axis.name),
                ));
            } else if let Some(other) = taken.get(axis.name.as_str()) {
                issues.push(Issue::error(
                    C::DuplicateName,
                    root,
                    format!("variant axis `{}` has the same name as a {other}", axis.name),
                ));
            }
            let mut options = BTreeSet::new();
            for option in &axis.options {
                if option.name.trim().is_empty() || !options.insert(&option.name) {
                    issues.push(Issue::error(
                        C::InvalidProp,
                        root,
                        format!("variant axis `{}` has an empty or duplicate option", axis.name),
                    ));
                }
            }
            if !axis.options.iter().any(|o| o.name == axis.default) {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!("variant axis `{}` has no option `{}`", axis.name, axis.default),
                ));
            }
            for option in &axis.options {
                for over in &option.overrides {
                    if !doc.is_within(&over.node, &component.root) {
                        issues.push(Issue::error(
                            C::InvalidProp,
                            root,
                            format!(
                                "variant `{}={}` targets a node outside the component",
                                axis.name, option.name
                            ),
                        ));
                        continue;
                    }
                    variant_override_rules(doc, &format!("{}={}", axis.name, option.name), over, issues);
                }
            }
        }
        if reaches(doc, &component.id, &component.id, &mut BTreeSet::new()) {
            issues.push(Issue::error(
                C::RecursiveComponent,
                root,
                format!("component `{}` contains itself", component.name),
            ));
        }
    }
}

/// Une surcharge de variante obéit aux règles du style de son nœud cible (mêmes règles que la
/// commande `update_component`) : propriétés permises pour son type et ses hôtes au rendu,
/// propriétés d'état seulement dans un état, `none` réservé aux tailles max, tokens existants.
fn variant_override_rules(
    doc: &Document,
    variant: &str,
    over: &crate::document::VariantOverride,
    issues: &mut Vec<Issue>,
) {
    let Some(node) = doc.node(&over.node) else { return };
    let id = &over.node;
    let mut entries: Vec<(StyleProp, PropChange, bool)> =
        over.style.entries().into_iter().map(|(p, c)| (p, c, false)).collect();
    for state in InteractionState::ALL {
        if let Some(patch) = over.states.get(state) {
            entries.extend(patch.entries().into_iter().map(|(p, c)| (p, c, true)));
        }
    }
    let mut hosts = None;
    for (prop, change, in_state) in entries {
        let PropChange::Merge(patch) = change else { continue };
        let allowed = (in_state || prop.in_base())
            && own_prop_allowed(prop, &node.kind)
            && (!PARENT_PROPS.contains(&prop)
                || hosts
                    .get_or_insert_with(|| rendered_hosts(doc, id))
                    .iter()
                    .all(|host| placement_allowed(prop, *host)));
        if !allowed {
            issues.push(Issue::error(
                C::StyleNotAllowed,
                Some(id),
                format!(
                    "variant `{variant}`: `{}` is not allowed on {} here",
                    prop.name(),
                    node.kind.type_name()
                ),
            ));
        }
        match &patch {
            ResponsiveValuePatch::Size(sizes)
                if !matches!(prop, StyleProp::MaxWidth | StyleProp::MaxHeight)
                    && sizes.values().into_iter().any(|size| *size == Size::None) =>
            {
                issues.push(Issue::error(
                    C::StyleNotAllowed,
                    Some(id),
                    format!(
                        "variant `{variant}`: `none` is only valid for max sizes, not `{}`",
                        prop.name()
                    ),
                ));
            }
            ResponsiveValuePatch::Token(fonts) => {
                for name in fonts.values() {
                    if doc.tokens.font(name).is_none() {
                        issues.push(Issue::error(
                            C::InvalidReference,
                            Some(id),
                            format!("variant `{variant}`: font token `{name}` does not exist"),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    let mut colors = style_patch_colors(&over.style);
    for state in InteractionState::ALL {
        if let Some(patch) = over.states.get(state) {
            for (_, change) in patch.entries() {
                if let PropChange::Merge(p) = change {
                    colors.extend(crate::query::patch_colors(&p));
                }
            }
        }
    }
    for color in colors {
        if let ColorSource::Token(name) = &color.source
            && doc.tokens.color(name).is_none()
        {
            issues.push(Issue::error(
                C::InvalidReference,
                Some(id),
                format!("variant `{variant}`: color token `{name}` does not exist"),
            ));
        }
    }
}

/// Vrai si `from` contient (transitivement) une instance de `target`.
fn reaches(doc: &Document, from: &ComponentId, target: &ComponentId, visited: &mut BTreeSet<ComponentId>) -> bool {
    if !visited.insert(from.clone()) {
        return false;
    }
    let Some(component) = doc.component(from) else {
        return false;
    };
    doc.subtree(&component.root)
        .iter()
        .any(|id| match doc.node(id).map(|n| &n.kind) {
            Some(NodeKind::ComponentInstance(instance)) => {
                instance.component == *target || reaches(doc, &instance.component, target, visited)
            }
            _ => false,
        })
}

fn tokens(doc: &Document, issues: &mut Vec<Issue>) {
    let mut names = BTreeSet::new();
    for color in &doc.tokens.colors {
        if !names.insert(color.name.clone()) {
            issues.push(Issue::error(
                C::InvalidToken,
                None,
                format!("color token `{}` is defined twice", color.name),
            ));
        }
    }
    let mut fonts = BTreeSet::new();
    for font in &doc.tokens.fonts {
        if !fonts.insert(font.name.clone()) {
            issues.push(Issue::error(
                C::InvalidToken,
                None,
                format!("font token `{}` is defined twice", font.name),
            ));
        }
        if let FontFamily::Google { family, weights } = &font.family {
            let valid = !family.trim().is_empty()
                && !weights.is_empty()
                && weights.iter().all(|w| (100..=900).contains(w) && w % 100 == 0);
            if !valid {
                issues.push(Issue::error(
                    C::InvalidToken,
                    None,
                    format!("font token `{}` has an invalid family or weights", font.name),
                ));
            }
        }
    }
    if !(1..=16).contains(&doc.tokens.spacing_unit) {
        issues.push(Issue::error(
            C::InvalidToken,
            None,
            "spacing unit must be between 1 and 16 px",
        ));
    }
}
