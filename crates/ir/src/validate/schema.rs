//! Règles de schéma : arbre, imbrication, style par primitive, références, routes, noms.

use std::collections::{BTreeMap, BTreeSet};

use crate::document::{Document, Owner, RouteSegment, route_path};
use crate::id::{ComponentId, NodeId, is_kebab_case};
use crate::node::{ContainerRole, DEFAULT_SLOT, Href, ImageSource, Node, NodeKind, PAGE_SLOT, TextRole};
use crate::query::node_colors;
use crate::style::color::ColorSource;
use crate::style::style::StyleProp;
use crate::style::values::Size;
use crate::tokens::FontFamily;
use crate::validate::{Context, Issue, IssueCode as C, is_known_icon};

pub(super) fn check(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    document_level(ctx.doc, issues);
    tree(ctx, issues);
    for node in ctx.doc.nodes.values() {
        node_rules(ctx, node, issues);
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
    if let Some(favicon) = &doc.settings.favicon
        && doc.asset(favicon).is_none()
    {
        issues.push(Issue::error(
            C::InvalidReference,
            None,
            format!("favicon asset `{favicon}` does not exist"),
        ));
    }
}

fn tree(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let mut root_count: BTreeMap<NodeId, usize> = BTreeMap::new();
    for (_, root) in doc.roots() {
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
            // Hors de tout arbre enregistré : cycle ou orphelin.
            let mut current = node.parent.clone();
            let mut steps = 0;
            while let Some(p) = current {
                steps += 1;
                if p == *key || steps > doc.nodes.len() {
                    issues.push(Issue::error(C::Cycle, Some(key), "node is part of a parent cycle"));
                    break;
                }
                current = doc.node(&p).and_then(|n| n.parent.clone());
            }
            if steps <= doc.nodes.len() {
                issues.push(Issue::error(
                    C::OrphanNode,
                    Some(key),
                    "node is not reachable from any page, layout or component",
                ));
            }
        }
    }
}

fn container_kind_of(node: Option<&Node>) -> Option<&'static str> {
    node.and_then(|n| n.kind.container()).map(|(k, _)| match k {
        crate::node::ContainerKind::Box => "Box",
        crate::node::ContainerKind::Stack => "Stack",
        crate::node::ContainerKind::Grid => "Grid",
    })
}

fn node_rules(ctx: &Context<'_>, node: &Node, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let id = &node.id;
    let owner = ctx.owner(id);
    let parent = node.parent.as_ref().and_then(|p| doc.node(p));

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
    if let Some((_, props)) = node.kind.container()
        && props.role == ContainerRole::ListItem
        && !parent
            .and_then(|p| p.kind.container())
            .is_some_and(|(_, p)| p.role == ContainerRole::List)
    {
        issues.push(Issue::error(
            C::ListItemOutsideList,
            Some(id),
            "list item must be a direct child of a list",
        ));
    }
    if matches!(node.kind, NodeKind::Slot(_)) && matches!(owner, Some(Owner::Page(_))) {
        issues.push(Issue::error(
            C::SlotOutsideComponent,
            Some(id),
            "slots are only allowed in components and layouts",
        ));
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

    // Style autorisé selon la primitive et le parent.
    let own = container_kind_of(Some(node));
    let parent_kind = container_kind_of(parent);
    for (prop, value) in node.style.entries() {
        let allowed = match prop {
            StyleProp::Columns => own == Some("Grid"),
            StyleProp::Direction | StyleProp::Wrap => own == Some("Stack"),
            StyleProp::Gap | StyleProp::Align | StyleProp::Justify => matches!(own, Some("Stack" | "Grid")),
            StyleProp::ObjectFit => matches!(node.kind, NodeKind::Image(_)),
            StyleProp::ColSpan => parent_kind == Some("Grid"),
            StyleProp::Grow | StyleProp::Shrink => parent_kind == Some("Stack"),
            StyleProp::AlignSelf | StyleProp::Order => matches!(parent_kind, Some("Stack" | "Grid")),
            _ => true,
        };
        if !allowed {
            issues.push(Issue::error(
                C::StyleNotAllowed,
                Some(id),
                format!("`{}` is not allowed on {} here", prop.name(), node.kind.type_name()),
            ));
        }
        if let crate::style::style::ResponsiveValue::Size(size) = &value
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
            match &image.source {
                ImageSource::Asset { id: asset } if doc.asset(asset).is_none() => {
                    issues.push(Issue::error(
                        C::InvalidReference,
                        Some(id),
                        format!("asset `{asset}` does not exist"),
                    ));
                }
                _ => {}
            }
            if matches!(image.source, ImageSource::Asset { .. } | ImageSource::Url { .. }) && image.intrinsic.is_none()
            {
                issues.push(Issue::error(
                    C::MissingIntrinsic,
                    Some(id),
                    "images from assets or URLs need intrinsic dimensions",
                ));
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
        NodeKind::Link(link) => href_rules(ctx, id, owner, &link.href, issues),
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

fn href_rules(ctx: &Context<'_>, id: &NodeId, owner: Option<&Owner>, href: &Href, issues: &mut Vec<Issue>) {
    match href {
        Href::Page { page, anchor } => match ctx.doc.page(page) {
            None => issues.push(Issue::error(
                C::InvalidReference,
                Some(id),
                format!("page `{page}` does not exist"),
            )),
            Some(target) => {
                if let Some(anchor) = anchor
                    && !ctx.page_anchors(target).contains(anchor)
                {
                    issues.push(Issue::error(
                        C::InvalidReference,
                        Some(id),
                        format!("anchor `{anchor}` does not exist on page `{}`", target.name),
                    ));
                }
            }
        },
        Href::Anchor { anchor } => {
            if let Some(Owner::Page(page)) = owner
                && let Some(page) = ctx.doc.page(page)
                && !ctx.page_anchors(page).contains(anchor)
            {
                issues.push(Issue::error(
                    C::InvalidReference,
                    Some(id),
                    format!("anchor `{anchor}` does not exist on this page"),
                ));
            }
        }
        Href::External { url } if !(url.starts_with("https://") || url.starts_with("http://")) => {
            issues.push(Issue::error(
                C::InvalidProp,
                Some(id),
                format!("external URL `{url}` must start with http(s)://"),
            ));
        }
        _ => {}
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
            Some(_) => {}
        }
    }
    for choice in &props.variants {
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
    let slots: BTreeSet<String> = ctx
        .doc
        .subtree(&component.root)
        .iter()
        .filter_map(|n| match &ctx.doc.node(n)?.kind {
            NodeKind::Slot(slot) => Some(slot.name.clone()),
            _ => None,
        })
        .collect();
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

fn pages(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let mut routes: BTreeMap<String, &str> = BTreeMap::new();
    for page in &doc.pages {
        if page.name.trim().is_empty() {
            issues.push(Issue::error(
                C::InvalidName,
                Some(&page.root),
                "page name must not be empty",
            ));
        }
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
        }
        // Deux paramètres au même rang entrent en conflit dans l'App Router.
        let shape: String = page
            .route
            .iter()
            .map(|s| match s {
                RouteSegment::Static(name) => format!("/{name}"),
                RouteSegment::Param(_) => "/[]".to_owned(),
            })
            .collect();
        if let Some(other) = routes.insert(shape, &page.name) {
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
        if let Some(image) = &page.seo.og_image
            && doc.asset(image).is_none()
        {
            issues.push(Issue::error(
                C::InvalidReference,
                Some(&page.root),
                format!("OG image `{image}` does not exist"),
            ));
        }
        // Ancres uniques dans la page (layout compris).
        let mut anchors: BTreeMap<String, usize> = BTreeMap::new();
        let mut roots = vec![page.root.clone()];
        if let Some(layout) = page.layout.as_ref().and_then(|l| doc.layout(l)) {
            roots.push(layout.root.clone());
        }
        for id in roots.iter().flat_map(|r| doc.subtree(r)) {
            if let Some(anchor) = doc.node(&id).and_then(|n| n.meta.anchor.clone()) {
                let count = anchors.entry(anchor.clone()).or_default();
                *count += 1;
                if *count == 2 {
                    issues.push(Issue::error(
                        C::DuplicateAnchor,
                        Some(&id),
                        format!("anchor `{anchor}` is used twice on page `{}`", page.name),
                    ));
                }
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
        let page_slots = doc
            .subtree(&layout.root)
            .iter()
            .filter(|n| matches!(doc.node(n).map(|n| &n.kind), Some(NodeKind::Slot(s)) if s.name == PAGE_SLOT))
            .count();
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
        let mut prop_names = BTreeSet::new();
        for prop in &component.props {
            if !prop_names.insert(&prop.name) {
                issues.push(Issue::error(
                    C::InvalidProp,
                    root,
                    format!("prop `{}` is defined twice", prop.name),
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
            }
        }
        for axis in &component.variants {
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
                    }
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
