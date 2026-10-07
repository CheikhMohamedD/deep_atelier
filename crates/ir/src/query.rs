//! Requêtes en lecture : usages de tokens, plan compact du document (outil `get_outline`).

use serde::Serialize;

use crate::document::{Document, Owner};
use crate::id::{NodeId, TokenName};
use crate::node::{Node, NodeKind};
use crate::style::color::ColorRef;
use crate::style::style::{InteractionState, PropChange, ResponsiveValue, ResponsiveValuePatch, StylePatch};

/// Références de couleur d'une valeur de style.
pub fn value_colors(value: &ResponsiveValue) -> Vec<&ColorRef> {
    match value {
        ResponsiveValue::Color(v) => v.values().collect(),
        ResponsiveValue::Background(v) => v.values().flat_map(|b| b.colors()).collect(),
        ResponsiveValue::Ring(v) => v.values().map(|r| &r.color).collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn patch_colors(patch: &ResponsiveValuePatch) -> Vec<ColorRef> {
    match patch {
        ResponsiveValuePatch::Color(p) => p.values().into_iter().cloned().collect(),
        ResponsiveValuePatch::Background(p) => p.values().into_iter().flat_map(|b| b.colors()).cloned().collect(),
        ResponsiveValuePatch::Ring(p) => p.values().into_iter().map(|r| r.color.clone()).collect(),
        _ => Vec::new(),
    }
}

/// Références de couleur portées par un nœud (style, états, segments de texte).
pub fn node_colors(node: &Node) -> Vec<ColorRef> {
    let mut out: Vec<ColorRef> = Vec::new();
    for (_, value) in node.style.entries() {
        out.extend(value_colors(&value).into_iter().cloned());
    }
    for state in InteractionState::ALL {
        for (_, value) in node.states.get(state).entries() {
            out.extend(value_colors(&value).into_iter().cloned());
        }
    }
    if let NodeKind::Text(text) = &node.kind {
        out.extend(text.content.iter().filter_map(|run| run.color.clone()));
    }
    out
}

/// Références de couleur d'un patch de style.
pub fn style_patch_colors(patch: &StylePatch) -> Vec<ColorRef> {
    patch
        .entries()
        .iter()
        .filter_map(|(_, change)| match change {
            PropChange::Merge(p) => Some(patch_colors(p)),
            PropChange::Remove => None,
        })
        .flatten()
        .collect()
}

/// Couleurs utilisées par les surcharges de variantes des composants.
fn variant_colors(doc: &Document) -> Vec<ColorRef> {
    let mut out = Vec::new();
    for component in &doc.components {
        for axis in &component.variants {
            for option in &axis.options {
                for over in &option.overrides {
                    out.extend(style_patch_colors(&over.style));
                    for state in InteractionState::ALL {
                        if let Some(patch) = over.states.get(state) {
                            for (_, change) in patch.entries() {
                                if let PropChange::Merge(p) = change {
                                    out.extend(patch_colors(&p));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// Vrai si un token de couleur est référencé quelque part.
pub fn color_token_in_use(doc: &Document, name: &TokenName) -> bool {
    let uses = |c: &ColorRef| c.token_name() == Some(name);
    doc.nodes.values().any(|n| node_colors(n).iter().any(uses)) || variant_colors(doc).iter().any(uses)
}

/// Vrai si un token de police est référencé quelque part.
pub fn font_token_in_use(doc: &Document, name: &TokenName) -> bool {
    let in_patch = |patch: &StylePatch| {
        patch.entries().iter().any(|(_, change)| {
            matches!(change, PropChange::Merge(ResponsiveValuePatch::Token(p)) if p.values().contains(&name))
        })
    };
    doc.nodes.values().any(|n| {
        n.style
            .font_family
            .as_ref()
            .is_some_and(|f| f.values().any(|v| v == name))
    }) || doc.components.iter().any(|c| {
        c.variants.iter().any(|a| {
            a.options
                .iter()
                .any(|o| o.overrides.iter().any(|ov| in_patch(&ov.style)))
        })
    })
}

/// Entrée du plan compact (`get_outline`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutlineNode {
    pub id: NodeId,
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Extrait du texte (60 caractères au plus).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<OutlineNode>,
    /// Nombre d'enfants omis au-delà de la profondeur demandée.
    #[serde(skip_serializing_if = "is_zero")]
    pub truncated: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn excerpt(node: &Node) -> Option<String> {
    let text = match &node.kind {
        NodeKind::Text(text) => text.plain_text(),
        NodeKind::Button(button) => button.label.clone()?,
        NodeKind::Link(link) => link.label.clone()?,
        NodeKind::Image(image) => image.alt.clone(),
        NodeKind::Icon(icon) => icon.name.clone(),
        NodeKind::Slot(slot) => slot.name.clone(),
        _ => return None,
    };
    let mut short: String = text.chars().take(60).collect();
    if text.chars().count() > 60 {
        short.push('…');
    }
    Some(short)
}

/// Plan compact d'un sous-arbre, limité en profondeur.
pub fn outline(doc: &Document, root: &NodeId, max_depth: usize) -> Option<OutlineNode> {
    let node = doc.node(root)?;
    let (children, truncated) = if max_depth == 0 {
        (Vec::new(), node.children.len())
    } else {
        (
            node.children
                .iter()
                .filter_map(|c| outline(doc, c, max_depth - 1))
                .collect(),
            0,
        )
    };
    Some(OutlineNode {
        id: node.id.clone(),
        kind: node.kind.type_name(),
        name: node.meta.name.clone(),
        text: excerpt(node),
        children,
        truncated,
    })
}

/// Nœuds demandés (outil `get_nodes`) ; les ids inconnus sont ignorés.
pub fn get_nodes(doc: &Document, ids: &[NodeId]) -> Vec<Node> {
    ids.iter().filter_map(|id| doc.node(id).cloned()).collect()
}

/// Libellé lisible d'un propriétaire (`page "Accueil"`).
pub fn owner_label(doc: &Document, owner: &Owner) -> String {
    match owner {
        Owner::Page(id) => doc
            .page(id)
            .map_or_else(|| id.to_string(), |p| format!("page \"{}\"", p.name)),
        Owner::Layout(id) => doc
            .layout(id)
            .map_or_else(|| id.to_string(), |l| format!("layout \"{}\"", l.name)),
        Owner::Component(id) => doc
            .component(id)
            .map_or_else(|| id.to_string(), |c| format!("component \"{}\"", c.name)),
    }
}
