//! Propriété : sur 150 actions aléatoires, chaque undo restaure exactement l'état précédent,
//! chaque redo l'état suivant, une transaction refusée ne change rien, et l'arbre reste cohérent.

mod common;

use common::*;
use ir::command::Command;
use ir::*;
use proptest::prelude::*;
use serde_json::{Value, json};

const ACTIONS: usize = 150;

/// Action tirée au hasard ; les paramètres sont réduits modulo l'état courant.
#[derive(Debug, Clone)]
struct Action {
    kind: u8,
    a: u32,
    b: u32,
    c: u32,
}

fn action() -> impl Strategy<Value = Action> {
    (0u8..16, any::<u32>(), any::<u32>(), any::<u32>()).prop_map(|(kind, a, b, c)| Action { kind, a, b, c })
}

fn pick<T: Clone>(items: &[T], seed: u32) -> Option<T> {
    (!items.is_empty()).then(|| items[seed as usize % items.len()].clone())
}

/// Nœuds de toutes les pages et composants, racines comprises.
fn all_nodes(doc: &Document) -> Vec<NodeId> {
    doc.roots()
        .into_iter()
        .flat_map(|(_, root)| doc.subtree(&root))
        .collect()
}

fn non_roots(doc: &Document) -> Vec<NodeId> {
    all_nodes(doc)
        .into_iter()
        .filter(|id| doc.nodes[id].parent.is_some())
        .collect()
}

fn containers(doc: &Document) -> Vec<NodeId> {
    all_nodes(doc)
        .into_iter()
        .filter(|id| doc.nodes[id].kind.accepts_children())
        .collect()
}

const CONTAINERS: [&str; 3] = ["Box", "Stack", "Grid"];
const BREAKPOINTS: [&str; 6] = ["base", "sm", "md", "lg", "xl", "2xl"];

fn style_patch(a: u32, b: u32) -> Value {
    let bp = BREAKPOINTS[b as usize % BREAKPOINTS.len()];
    let value = |options: &[&str]| json!(options[(a / 7) as usize % options.len()]);
    let (prop, v) = match a % 8 {
        0 => ("gap", value(&["0", "2", "4", "8"])),
        1 => ("padding", value(&["1", "4", "6", "12"])),
        2 => ("width", value(&["full", "1/2", "64", "container.md"])),
        3 => ("font_size", value(&["sm", "base", "2xl", "4xl"])),
        4 => ("text_color", value(&["foreground", "primary/80", "muted-foreground"])),
        5 => (
            "background",
            json!({ "kind": "Color", "color": (["muted", "card", "accent/50"][(a / 7) as usize % 3]) }),
        ),
        6 => ("radius", value(&["none", "md", "xl", "full"])),
        _ => {
            return if b % 3 == 0 {
                json!({ "padding_x": null })
            } else {
                json!({ "opacity": { bp: null } })
            };
        }
    };
    json!({ prop: { bp: v } })
}

fn kind_spec(seed: u32) -> Value {
    match seed % 6 {
        0 => json!({ "type": "Box", "role": { "kind": "Section" } }),
        1 => json!({ "type": "Stack" }),
        2 => json!({ "type": "Grid" }),
        3 => json!({ "type": "Text", "content": [{ "text": format!("Texte {seed}") }] }),
        4 => json!({ "type": "Button", "label": "Action" }),
        _ => json!({ "type": "Image", "alt": "Illustration" }),
    }
}

/// Construit les commandes d'une action à partir de l'état courant (`None` : rien à faire).
fn commands_for(doc: &Document, action: &Action) -> Option<Value> {
    let Action { kind, a, b, c } = action.clone();
    Some(match kind {
        0..=2 => {
            let parent = pick(&containers(doc), a)?;
            let len = doc.nodes[&parent].children.len() as u32;
            json!([{ "op": "insert_nodes", "parent": parent.to_string(), "index": b % (len + 1),
                     "nodes": [ { "ref": "$x", "kind": kind_spec(c) },
                                { "parent": "$x", "kind": kind_spec(c / 6) } ] }])
        }
        3 => json!([{ "op": "delete_nodes", "nodes": [pick(&non_roots(doc), a)?.to_string()] }]),
        4 => {
            let node = pick(&non_roots(doc), a)?;
            let targets: Vec<NodeId> = containers(doc)
                .into_iter()
                .filter(|t| !doc.is_within(t, &node))
                .collect();
            let parent = pick(&targets, b)?;
            let len = doc.nodes[&parent].children.iter().filter(|c| **c != node).count() as u32;
            json!([{ "op": "move_node", "node": node.to_string(), "parent": parent.to_string(), "index": c % (len + 1) }])
        }
        5 | 6 => {
            json!([{ "op": "set_style", "node": pick(&all_nodes(doc), a)?.to_string(), "style": style_patch(b, c) }])
        }
        7 => json!([{ "op": "duplicate_node", "node": pick(&non_roots(doc), a)?.to_string() }]),
        8 => json!([{ "op": "wrap_nodes", "nodes": [pick(&non_roots(doc), a)?.to_string()],
                      "container": { "kind": (CONTAINERS[b as usize % 3]) } }]),
        9 => {
            let candidates: Vec<NodeId> = non_roots(doc)
                .into_iter()
                .filter(|id| doc.nodes[id].kind.container().is_some())
                .collect();
            let node = pick(&candidates, a)?;
            if b % 2 == 0 {
                json!([{ "op": "unwrap_node", "node": node.to_string() }])
            } else {
                json!([{ "op": "convert_container", "node": node.to_string(), "to": (CONTAINERS[c as usize % 3]) }])
            }
        }
        10 => {
            let node = pick(&all_nodes(doc), a)?;
            match &doc.nodes[&node].kind {
                NodeKind::Text(_) => json!([{ "op": "set_props", "node": node.to_string(),
                                              "props": { "type": "Text", "content": [{ "text": format!("Édité {b}") }] } }]),
                _ => json!([{ "op": "set_meta", "node": node.to_string(),
                              "meta": { "name": format!("Calque {b}"), "locked": c % 2 == 0 } }]),
            }
        }
        11 => {
            let node = pick(&all_nodes(doc), a)?;
            let visibility = if b % 3 == 0 {
                Value::Null
            } else {
                json!({ BREAKPOINTS[c as usize % 6]: b % 2 == 0 })
            };
            json!([{ "op": "set_visibility", "node": node.to_string(), "visibility": visibility }])
        }
        12 => {
            let candidates: Vec<NodeId> = non_roots(doc)
                .into_iter()
                .filter(|id| doc.owner_of(id).is_some_and(|o| matches!(o, Owner::Page(_))))
                .filter(|id| !matches!(doc.nodes[id].kind, NodeKind::Slot(_)))
                .collect();
            let node = pick(&candidates, a)?;
            json!([{ "op": "create_component", "node": node.to_string(), "name": format!("Bloc{}", b % 10_000), "props": [] }])
        }
        13 => {
            let instances: Vec<NodeId> = all_nodes(doc)
                .into_iter()
                .filter(|id| {
                    matches!(doc.nodes[id].kind, NodeKind::ComponentInstance(_)) && doc.nodes[id].parent.is_some()
                })
                .collect();
            json!([{ "op": "detach_instance", "node": pick(&instances, a)?.to_string() }])
        }
        14 => match a % 3 {
            0 => {
                json!([{ "op": "create_page", "name": format!("Page {b}"), "route": [{ "kind": "Static", "name": format!("p{}", b % 50) }] }])
            }
            1 => json!([{ "op": "set_token", "edit": { "kind": "Color", "name": format!("brand-{}", b % 5),
                                                         "value": { "light": format!("#{:06x}", c & 0xff_ffff) } } }]),
            _ => json!([{ "op": "create_layout", "name": format!("layout-{}", b % 20) }]),
        },
        _ => {
            // Deux commandes dans la même transaction (la seconde peut échouer : tout est annulé).
            let node = pick(&all_nodes(doc), a)?;
            json!([
                { "op": "set_style", "node": node.to_string(), "style": style_patch(b, c) },
                { "op": "set_style", "node": node.to_string(), "style": style_patch(c, b) }
            ])
        }
    })
}

/// Invariants structurels : l'arbre reste cohérent quelle que soit la séquence.
fn assert_tree_consistent(doc: &Document) {
    let broken: Vec<Issue> = validate(doc)
        .into_iter()
        .filter(|i| {
            matches!(
                i.code,
                IssueCode::TreeInconsistent | IssueCode::OrphanNode | IssueCode::Cycle | IssueCode::DuplicateId
            )
        })
        .collect();
    assert!(broken.is_empty(), "{broken:#?}");
}

/// Applique les actions ; retourne les états successifs (un par entrée d'undo).
fn play(session: &mut Session, actions: &[Action]) -> Vec<Document> {
    let mut states = vec![session.document().clone()];
    for action in actions {
        let Some(commands) = commands_for(session.document(), action) else {
            continue;
        };
        let commands: Vec<Command> = serde_json::from_value(commands).expect("generated commands parse");
        let before = session.document().clone();
        match session.apply(Transaction::user(format!("action {}", action.kind), commands)) {
            Ok(applied) if !applied.ops.is_empty() => states.push(session.document().clone()),
            Ok(_) => assert_eq!(*session.document(), before, "a no-op transaction changes nothing"),
            Err(_) => assert_eq!(*session.document(), before, "a refused transaction changes nothing"),
        }
        assert_tree_consistent(session.document());
    }
    states
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn undo_then_redo_150_actions(actions in prop::collection::vec(action(), ACTIONS)) {
        let mut session = session();
        let states = play(&mut session, &actions);
        prop_assert_eq!(session.undo_labels().len(), states.len() - 1);
        for expected in states.iter().rev().skip(1) {
            prop_assert!(session.undo().unwrap().is_some());
            prop_assert_eq!(session.document(), expected);
        }
        prop_assert!(session.undo().unwrap().is_none());
        for expected in states.iter().skip(1) {
            prop_assert!(session.redo().unwrap().is_some());
            prop_assert_eq!(session.document(), expected);
        }
        prop_assert!(session.redo().unwrap().is_none());
    }

    #[test]
    fn interleaved_undo_redo_matches_a_model(
        actions in prop::collection::vec(action(), ACTIONS),
        moves in prop::collection::vec(0u8..4, ACTIONS),
    ) {
        let mut session = session();
        // Modèle : états passés (sommet = courant) et états rétablissables.
        let mut past = vec![session.document().clone()];
        let mut future: Vec<Document> = Vec::new();
        for (action, step) in actions.iter().zip(moves) {
            match step {
                0 => {
                    let undone = session.undo().unwrap();
                    prop_assert_eq!(undone.is_some(), past.len() > 1);
                    if undone.is_some() {
                        future.push(past.pop().unwrap());
                    }
                }
                1 => {
                    let redone = session.redo().unwrap();
                    prop_assert_eq!(redone.is_some(), !future.is_empty());
                    if redone.is_some() {
                        past.push(future.pop().unwrap());
                    }
                }
                _ => {
                    let applied = play(&mut session, std::slice::from_ref(action));
                    if applied.len() > 1 {
                        past.push(session.document().clone());
                        future.clear();
                    }
                }
            }
            prop_assert_eq!(session.document(), past.last().unwrap());
            assert_tree_consistent(session.document());
        }
    }
}

#[test]
fn hundred_consecutive_actions_undo_in_one_pass() {
    // Critère de la spec : « Undo/redo fiable sur 100 actions consécutives ».
    let mut session = session();
    let root = home_root(&session);
    let initial = session.document().clone();
    for i in 0..100 {
        run_json(
            &mut session,
            json!([{ "op": "insert_nodes", "parent": root.to_string(), "index": 0,
                     "nodes": [{ "kind": { "type": "Text", "content": [{ "text": format!("Ligne {i}") }] } }] }]),
        );
    }
    let last = session.document().clone();
    for _ in 0..100 {
        session.undo().unwrap().unwrap();
    }
    assert_eq!(*session.document(), initial);
    for _ in 0..100 {
        session.redo().unwrap().unwrap();
    }
    assert_eq!(*session.document(), last);
}
