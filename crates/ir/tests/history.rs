//! Historique : undo/redo, gestes, brouillons IA (tout accepter, par unité, rejet).

mod common;

use common::*;
use ir::command::Command;
use ir::*;
use serde_json::json;

fn add_box(session: &mut Session, name: &str) -> NodeId {
    let root = home_root(session);
    let applied = run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": format!("${name}"), "kind": { "type": "Box" }, "meta": { "name": name } }
        ]}]),
    );
    applied.created[&format!("${name}")].clone()
}

#[test]
fn undo_and_redo_restore_exact_states() {
    let mut session = session();
    let s0 = session.document().clone();
    let a = add_box(&mut session, "a");
    let s1 = session.document().clone();
    run(
        &mut session,
        vec![Command::SetStyle {
            node: r(&a),
            state: None,
            style: json(json!({ "padding": { "base": "4", "lg": "8" } })),
        }],
    );
    let s2 = session.document().clone();

    assert!(session.undo().unwrap().is_some());
    assert_eq!(*session.document(), s1);
    let changes = session.undo().unwrap().unwrap();
    assert!(changes.nodes.contains(&a));
    assert_eq!(*session.document(), s0);
    assert_eq!(session.undo().unwrap(), None);

    session.redo().unwrap();
    assert_eq!(*session.document(), s1);
    session.redo().unwrap();
    assert_eq!(*session.document(), s2);
    assert_eq!(session.redo().unwrap(), None);
}

#[test]
fn a_new_transaction_clears_the_redo_stack() {
    let mut session = session();
    add_box(&mut session, "a");
    session.undo().unwrap();
    assert!(session.can_redo());
    add_box(&mut session, "b");
    assert!(!session.can_redo());
}

#[test]
fn a_gesture_is_a_single_undo_entry() {
    let mut session = session();
    let a = add_box(&mut session, "a");
    let before = session.document().clone();
    session.begin_gesture("padding slider");
    for step in ["1", "2", "3", "4"] {
        run(
            &mut session,
            vec![Command::SetStyle {
                node: r(&a),
                state: None,
                style: json(json!({ "padding_top": { "base": step } })),
            }],
        );
    }
    session.end_gesture();
    assert_eq!(session.undo_labels(), vec!["test", "padding slider"]);
    session.undo().unwrap();
    assert_eq!(*session.document(), before);
}

#[test]
fn ai_draft_is_outside_history_and_commits_as_one_undo() {
    let mut session = session();
    let root = home_root(&session);
    let before = session.document().clone();
    session.begin_draft("run-1").unwrap();
    // Le brouillon refuse les transactions d'une autre origine.
    assert!(matches!(
        session.apply(Transaction::user("x", vec![])),
        Err(CommandError::DraftInProgress)
    ));
    let first = try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$main", "kind": { "type": "Box", "role": { "kind": "Main" } } },
            { "parent": "$main", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": [{ "text": "Titre" }] } }
        ]}]),
    )
    .unwrap();
    // Une référence locale reste visible pendant tout le run.
    try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([{ "op": "set_style", "node": "$main", "style": { "padding": { "base": "4" } } }]),
    )
    .unwrap();
    assert_eq!(session.draft_units().len(), 2);
    assert!(session.undo().is_err(), "undo is locked during a draft");
    let after = session.document().clone();
    assert!(after.nodes.contains_key(&first.created["$main"]));

    session.commit_draft(DraftAccept::All).unwrap();
    assert_eq!(*session.document(), after);
    assert_eq!(session.undo_labels(), vec!["test"]);
    assert!(matches!(session.last_origin(), Some(Origin::Ai { .. })));
    session.undo().unwrap();
    assert_eq!(*session.document(), before);
    session.redo().unwrap();
    assert_eq!(*session.document(), after);
}

#[test]
fn discarding_a_draft_restores_the_document() {
    let mut session = session();
    let root = home_root(&session);
    let before = session.document().clone();
    session.begin_draft("run-1").unwrap();
    try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [{ "kind": { "type": "Box" } }] }]),
    )
    .unwrap();
    let changes = session.discard_draft();
    assert!(!changes.is_empty());
    assert_eq!(*session.document(), before);
    assert!(!session.has_draft());
    assert!(session.undo_labels().is_empty());
}

#[test]
fn accepting_units_replays_only_the_selected_commands() {
    let mut session = session();
    let root = home_root(&session);
    session.begin_draft("run-1").unwrap();
    let applied = try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([
            { "op": "insert_nodes", "parent": root.to_string(), "nodes": [{ "ref": "$a", "kind": { "type": "Box" }, "meta": { "name": "A" } }] },
            { "op": "insert_nodes", "parent": root.to_string(), "nodes": [{ "ref": "$b", "kind": { "type": "Box" }, "meta": { "name": "B" } }] },
            { "op": "set_style", "node": "$a", "style": { "padding": { "base": "2" } } }
        ]),
    )
    .unwrap();
    let units = session.draft_units();
    assert_eq!(units.len(), 3);
    session.commit_draft(DraftAccept::Units(vec![units[1].id])).unwrap();
    let doc = session.document();
    assert!(!doc.nodes.contains_key(&applied.created["$a"]));
    assert!(doc.nodes.contains_key(&applied.created["$b"]));
    assert_eq!(doc.nodes[&root].children, vec![applied.created["$b"].clone()]);
}

#[test]
fn a_unit_depending_on_a_rejected_unit_is_refused_and_the_draft_kept() {
    let mut session = session();
    let root = home_root(&session);
    session.begin_draft("run-1").unwrap();
    try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([
            { "op": "insert_nodes", "parent": root.to_string(), "nodes": [{ "ref": "$a", "kind": { "type": "Box" } }] },
            { "op": "set_style", "node": "$a", "style": { "padding": { "base": "2" } } }
        ]),
    )
    .unwrap();
    let draft_state = session.document().clone();
    let units = session.draft_units();
    let error = session.commit_draft(DraftAccept::Units(vec![units[1].id])).unwrap_err();
    assert_eq!(error.code(), "UNIT_DEPENDENCY");
    assert!(session.has_draft());
    assert_eq!(*session.document(), draft_state);
}

#[test]
fn a_draft_introducing_blocking_errors_cannot_be_committed() {
    let mut session = session();
    let root = home_root(&session);
    session.begin_draft("run-1").unwrap();
    try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "kind": { "type": "Image", "alt": "" } }
        ]}]),
    )
    .unwrap();
    let error = session.commit_draft(DraftAccept::All).unwrap_err();
    let CommandError::ValidationFailed(issues) = error else {
        panic!("validation failure expected")
    };
    assert!(issues.iter().any(|i| i.code == IssueCode::A11yImgAlt));
    assert!(session.has_draft());
    // Le modèle corrige dans le même run, puis la validation passe.
    let units = session.draft_units();
    let image = units[0].changes.nodes.iter().find(|id| **id != root).unwrap().clone();
    try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([{ "op": "set_props", "node": image.to_string(), "props": { "type": "Image", "alt": "Bureau lumineux" } }]),
    )
    .unwrap();
    session.commit_draft(DraftAccept::All).unwrap();
    assert!(!session.has_draft());
}

#[test]
fn pre_existing_errors_do_not_block_a_draft() {
    let mut session = session();
    let root = home_root(&session);
    run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [{ "kind": { "type": "Image", "alt": "" } }] }]),
    );
    session.begin_draft("run-2").unwrap();
    try_run(
        &mut session,
        Origin::Ai { run_id: "run-2".into() },
        Scope::full(),
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [{ "kind": { "type": "Box" } }] }]),
    )
    .unwrap();
    session.commit_draft(DraftAccept::All).unwrap();
}
