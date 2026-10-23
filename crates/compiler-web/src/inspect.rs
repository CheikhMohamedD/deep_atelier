//! Inspection d'un nœud pour l'inspecteur de l'éditeur : propriétés de style permises, classes
//! Tailwind produites par chaque valeur déclarée (par breakpoint et par état d'interaction),
//! problèmes de validation du nœud et entité qui le contient.

use ir::query::allowed_style_props;
use ir::{Breakpoint, ComponentId, Document, InteractionState, Issue, LayoutId, NodeId, Owner, PageId, StyleProp};
use serde::Serialize;
use ts_rs::TS;

use crate::classes::prop_classes;

/// Ce que l'inspecteur affiche d'un nœud, en plus du nœud lui-même (lu dans le document).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct NodeInspection {
    pub node: NodeId,
    /// Page, layout ou composant qui contient le nœud.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub owner: Option<OwnerRef>,
    /// Propriétés de style acceptées hors états, dans l'ordre de déclaration de l'IR.
    pub allowed: Vec<StyleProp>,
    /// Classes de chaque valeur déclarée.
    pub classes: Vec<PropClasses>,
    /// Problèmes de validation portés par ce nœud.
    pub issues: Vec<Issue>,
}

/// Classes produites par une propriété à un breakpoint, hors état ou dans un état.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PropClasses {
    pub prop: StyleProp,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub state: Option<InteractionState>,
    pub breakpoint: Breakpoint,
    /// Vide quand l'export n'émet rien (valeur initiale, ou état égal au style hors état).
    pub classes: Vec<String>,
}

/// Entité qui contient un nœud.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OwnerRef {
    Page { id: PageId, name: String },
    Layout { id: LayoutId, name: String },
    Component { id: ComponentId, name: String },
}

/// Inspection d'un nœud ; `None` s'il n'existe pas.
pub fn inspect_node(doc: &Document, id: &NodeId) -> Option<NodeInspection> {
    let node = doc.node(id)?;
    let owner = doc.owner_of(id).and_then(|owner| match owner {
        Owner::Page(id) => doc.page(&id).map(|p| OwnerRef::Page {
            name: p.name.clone(),
            id,
        }),
        Owner::Layout(id) => doc.layout(&id).map(|l| OwnerRef::Layout {
            name: l.name.clone(),
            id,
        }),
        Owner::Component(id) => doc.component(&id).map(|c| OwnerRef::Component {
            name: c.name.clone(),
            id,
        }),
    });
    let mut classes = Vec::new();
    let mut push = |state: Option<InteractionState>, entries: Vec<(StyleProp, ir::ResponsiveValue)>| {
        for (prop, value) in entries {
            for bp in value.declared() {
                classes.push(PropClasses {
                    prop,
                    state,
                    breakpoint: bp,
                    classes: prop_classes(doc, node, prop, &value.at(bp), bp, state),
                });
            }
        }
    };
    push(None, node.style.entries());
    for state in InteractionState::ALL {
        push(Some(state), node.states.get(state).entries());
    }
    let issues = ir::validate(doc)
        .into_iter()
        .filter(|issue| issue.node.as_ref() == Some(id))
        .collect();
    Some(NodeInspection {
        node: id.clone(),
        owner,
        allowed: allowed_style_props(doc, id),
        classes,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::landing;
    use ir::{IdGen, Session, Transaction};
    use serde_json::json;

    fn apply(session: &mut Session, commands: serde_json::Value) -> ir::Applied {
        let commands = serde_json::from_value(commands).expect("commands");
        session.apply(Transaction::user("test", commands)).expect("applies")
    }

    #[test]
    fn inspects_classes_per_breakpoint_and_state() {
        let mut session = Session::new(Document::new("Site", &mut IdGen::from_seed(1)), 2);
        let root = session.document().pages[0].root.clone();
        let applied = apply(
            &mut session,
            json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
                { "ref": "$row", "kind": { "type": "Stack" },
                  "style": { "direction": { "base": "column", "md": "row" }, "padding_top": { "base": "4", "lg": "8" },
                             "gap": { "base": "0" } },
                  "hover": { "background": { "base": { "kind": "Color", "color": "muted" } } } }
            ] }]),
        );
        let row = applied.created["$row"].clone();
        let doc = session.document();
        let inspection = inspect_node(doc, &row).expect("node exists");
        let find = |prop: StyleProp, state: Option<InteractionState>, bp: Breakpoint| {
            inspection
                .classes
                .iter()
                .find(|c| c.prop == prop && c.state == state && c.breakpoint == bp)
                .map(|c| c.classes.clone())
        };
        assert_eq!(
            find(StyleProp::Direction, None, Breakpoint::Md),
            Some(vec!["md:flex-row".to_owned()])
        );
        assert_eq!(
            find(StyleProp::PaddingTop, None, Breakpoint::Base),
            Some(vec!["pt-4".to_owned()])
        );
        assert_eq!(
            find(StyleProp::PaddingTop, None, Breakpoint::Lg),
            Some(vec!["lg:pt-8".to_owned()])
        );
        // Valeur initiale à `base` : rien n'est émis à l'export.
        assert_eq!(find(StyleProp::Gap, None, Breakpoint::Base), Some(Vec::new()));
        assert_eq!(
            find(StyleProp::Background, Some(InteractionState::Hover), Breakpoint::Base),
            Some(vec!["hover:bg-muted".to_owned()])
        );
        assert!(matches!(inspection.owner, Some(OwnerRef::Page { .. })));
        // Un Stack accepte `direction` ; posé dans la racine de page (un Box), pas `grow`.
        assert!(inspection.allowed.contains(&StyleProp::Direction));
        assert!(!inspection.allowed.contains(&StyleProp::Columns));
        assert!(!inspection.allowed.contains(&StyleProp::Grow));
        assert!(
            !inspection.allowed.contains(&StyleProp::Ring),
            "state-only props are excluded"
        );
    }

    #[test]
    fn reports_the_issues_of_the_node_only() {
        let doc = landing();
        let image = doc
            .nodes
            .values()
            .find(|n| matches!(n.kind, ir::NodeKind::Image(_)))
            .map(|n| n.id.clone())
            .expect("the landing has an image");
        let inspection = inspect_node(&doc, &image).expect("node exists");
        assert!(inspection.issues.iter().all(|i| i.node.as_ref() == Some(&image)));
        assert!(inspect_node(&doc, &"n_zzzzzzzzzz".parse().expect("id")).is_none());
    }
}
