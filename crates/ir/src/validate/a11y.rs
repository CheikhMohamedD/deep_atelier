//! Accessibilité : alt, titres, noms accessibles, landmark `Main`, langue.

use crate::id::NodeId;
use crate::node::{ContainerRole, Node, NodeKind, TextRole};
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
    for node in doc.nodes.values() {
        if ctx.owner(&node.id).is_none() {
            continue;
        }
        node_rules(ctx, node, issues);
    }
    for page in &doc.pages {
        let order = ctx.render_order(page);
        let mut h1 = 0;
        let mut previous = 0u8;
        let mut mains = 0;
        for id in &order {
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

/// Vrai si le sous-arbre contient un texte visible ou une icône étiquetée (nom accessible).
fn has_named_content(ctx: &Context<'_>, id: &NodeId) -> bool {
    ctx.doc.subtree(id).iter().skip(1).any(|child| {
        ctx.doc.node(child).is_some_and(|n| {
            !n.a11y.hidden
                && match &n.kind {
                    NodeKind::Text(text) => !text.plain_text().trim().is_empty(),
                    NodeKind::Icon(_) | NodeKind::Image(_) => n.a11y.label.is_some(),
                    NodeKind::ComponentInstance(_) => true,
                    _ => false,
                }
        })
    })
}

fn non_empty(text: &Option<String>) -> bool {
    text.as_deref().is_some_and(|t| !t.trim().is_empty())
}

fn node_rules(ctx: &Context<'_>, node: &Node, issues: &mut Vec<Issue>) {
    let id = &node.id;
    if node.a11y.hidden {
        return;
    }
    match &node.kind {
        NodeKind::Image(image) if image.alt.trim().is_empty() => {
            issues.push(Issue::error(
                C::A11yImgAlt,
                Some(id),
                "image needs alt text (or be marked hidden if decorative)",
            ));
        }
        NodeKind::Button(button) => {
            if !(non_empty(&button.label) || non_empty(&node.a11y.label) || has_named_content(ctx, id)) {
                issues.push(Issue::error(
                    C::A11yAccessibleName,
                    Some(id),
                    "button has no accessible name",
                ));
            }
        }
        NodeKind::Link(link) => {
            if !(non_empty(&link.label) || non_empty(&node.a11y.label) || has_named_content(ctx, id)) {
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
