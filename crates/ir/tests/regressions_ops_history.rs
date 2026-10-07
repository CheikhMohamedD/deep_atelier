//! Régressions de la revue (a), groupe ops / historique : acceptation partielle d'un brouillon
//! (dépendances par valeur entière, entités créées par une unité rejetée, valeurs copiées ou
//! héritées, sous-arbres retirés, tokens référencés), erreurs déjà présentes avant un brouillon,
//! forme des sous-arbres insérés.

mod common;

use common::*;
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

fn ai_run(session: &mut Session, commands: serde_json::Value) -> Applied {
    try_run(session, ai(), Scope::full(), commands).expect("AI transaction applies")
}

/// L'acceptation est refusée pour dépendance, sans toucher au brouillon.
fn assert_unit_dependency(session: &mut Session, accepted: Vec<ChangeUnitId>) {
    let draft_state = session.document().clone();
    let error = session.commit_draft(DraftAccept::Units(accepted)).unwrap_err();
    assert_eq!(error.code(), "UNIT_DEPENDENCY", "{error:?}");
    assert!(session.has_draft());
    assert_eq!(*session.document(), draft_state);
}

fn has_color_token(doc: &Document, name: &str) -> bool {
    doc.tokens.color(&name.parse().unwrap()).is_some()
}

// Constat 0 : une op à valeur entière d'une unité acceptée embarque les modifications d'une
// unité rejetée plus ancienne.

#[test]
fn accepting_a_token_edit_built_on_a_rejected_token_edit_is_a_unit_dependency() {
    let mut session = session();
    let a = add_box(&mut session, "a");
    let tokens_before = session.document().tokens.clone();
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "set_token", "edit": { "kind": "Color", "name": "brand", "value": { "light": "#1D4ED8" } } },
            { "op": "set_style", "node": a.to_string(), "style": { "padding_bottom": { "base": "2" } } },
            { "op": "set_token", "edit": { "kind": "Color", "name": "highlight", "value": { "light": "#DC2626" } } }
        ]),
    );
    let units = session.draft_units();
    assert_eq!(units.len(), 3);
    // L'op `set_tokens` du second `set_token` contient `brand`, rejeté : elle ne peut pas être
    // acceptée sans lui.
    assert_unit_dependency(&mut session, vec![units[1].id, units[2].id]);

    // Une unité indépendante reste acceptable seule.
    session.commit_draft(DraftAccept::Units(vec![units[1].id])).unwrap();
    let doc = session.document();
    assert!(!has_color_token(doc, "brand"));
    assert!(!has_color_token(doc, "highlight"));
    assert_eq!(doc.tokens, tokens_before);
    assert!(doc.nodes[&a].style.padding_bottom.is_some());
}

#[test]
fn accepting_a_style_value_built_on_a_rejected_override_is_a_unit_dependency() {
    let mut session = session();
    let a = add_box(&mut session, "a");
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "set_style", "node": a.to_string(), "style": { "padding_top": { "md": "4" } } },
            { "op": "set_style", "node": a.to_string(), "style": { "padding_bottom": { "base": "2" } } },
            { "op": "set_style", "node": a.to_string(), "style": { "padding_top": { "lg": "8" } } }
        ]),
    );
    let units = session.draft_units();
    assert_eq!(units.len(), 3);
    // `padding_top` de l'unité 2 porte aussi la surcharge `md` de l'unité 0, rejetée.
    assert_unit_dependency(&mut session, vec![units[2].id]);
    assert_unit_dependency(&mut session, vec![units[1].id, units[2].id]);

    // Une autre propriété du même nœud est indépendante.
    session.commit_draft(DraftAccept::Units(vec![units[1].id])).unwrap();
    let node = &session.document().nodes[&a];
    assert!(node.style.padding_top.is_none());
    assert!(node.style.padding_bottom.is_some());
}

#[test]
fn accepting_a_component_built_from_a_rejected_edit_is_a_unit_dependency() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$card", "kind": { "type": "Box" }, "meta": { "name": "Card" } },
            { "ref": "$label", "parent": "$card", "kind": { "type": "Text", "content": [{ "text": "Rapide" }] } }
        ]}]),
    );
    let card = applied.created["$card"].clone();
    let label = applied.created["$label"].clone();
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "set_style", "node": label.to_string(), "style": { "padding_top": { "base": "4" } } },
            { "op": "create_component", "node": card.to_string(), "name": "Card", "props": [] }
        ]),
    );
    let units = session.draft_units();
    // Le sous-arbre réinséré comme racine du composant contient le style rejeté du libellé.
    assert_unit_dependency(&mut session, vec![units[1].id]);
}

// Constat 3 : une op `Put*` rejouée seule sur une entité créée par une unité rejetée.

#[test]
fn editing_an_entity_created_by_a_rejected_unit_is_a_unit_dependency() {
    let mut session = session();
    let a = add_box(&mut session, "a");
    session.begin_draft("run-1").unwrap();
    let created = ai_run(
        &mut session,
        json!([{ "op": "create_page", "name": "About", "route": [{ "kind": "Static", "name": "about" }] }]),
    );
    let page = created.pages[0].clone();
    ai_run(
        &mut session,
        json!([{ "op": "update_page", "page": page.to_string(), "name": "À propos" }]),
    );
    let units = session.draft_units();
    assert_eq!(units.len(), 2);
    assert_unit_dependency(&mut session, vec![units[1].id]);

    // Un lien vers la page créée par l'unité rejetée en dépend aussi.
    ai_run(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": a.to_string(), "nodes": [
            { "kind": { "type": "Link", "href": { "kind": "Page", "page": page.to_string() }, "label": "À propos" } }
        ]}]),
    );
    let units = session.draft_units();
    assert_eq!(units.len(), 3);
    assert_unit_dependency(&mut session, vec![units[2].id]);
    session.discard_draft();

    // Même chose pour un composant créé puis renommé dans le même run.
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([{ "op": "create_component", "node": a.to_string(), "name": "Panel", "props": [] }]),
    );
    let component = session.document().components[0].id.clone();
    ai_run(
        &mut session,
        json!([{ "op": "update_component", "component": component.to_string(), "name": "Bloc" }]),
    );
    let units = session.draft_units();
    assert_eq!(units.len(), 2);
    assert_unit_dependency(&mut session, vec![units[1].id]);
}

// Constat 1 : une erreur déjà présente dont seul le message change ne bloque pas le brouillon.

#[test]
fn a_pre_existing_error_whose_message_changes_does_not_block_a_draft() {
    let mut session = session();
    let root = home_root(&session);
    run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": [{ "text": "A" }] } },
            { "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": [{ "text": "B" }] } }
        ]}]),
    );
    assert!(
        session
            .validate()
            .iter()
            .any(|i| i.code == IssueCode::A11yMultipleH1 && i.is_error())
    );
    let page = session.document().pages[0].id.clone();
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([{ "op": "update_page", "page": page.to_string(), "name": "Home" }]),
    );
    session.commit_draft(DraftAccept::All).unwrap();
    assert!(!session.has_draft());
    assert_eq!(session.document().pages[0].name, "Home");
}

// Constat 2 : `InsertSubtree` doit refuser un sous-arbre dont les listes d'enfants ne
// correspondent pas aux liens `parent`, sans modifier le document.

#[test]
fn insert_subtree_rejects_children_lists_that_disagree_with_parent_links() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$a", "kind": { "type": "Box" } },
            { "ref": "$b", "parent": "$a", "kind": { "type": "Box" } },
            { "ref": "$x", "kind": { "type": "Box" } }
        ]}]),
    );
    let a = applied.created["$a"].clone();
    let b = applied.created["$b"].clone();
    let x = applied.created["$x"].clone();
    let original = session.document().clone();

    let mut doc = original.clone();
    let reinsert = Op::RemoveSubtree { root: a.clone() }.apply(&mut doc).unwrap();
    let removed = doc.clone();
    let Op::InsertSubtree { parent, index, nodes } = reinsert.clone() else {
        panic!("the inverse of a removal is an insertion")
    };
    assert_eq!(nodes.len(), 2);

    let rejects = |nodes: Vec<Node>| {
        let mut target = removed.clone();
        let result = Op::InsertSubtree {
            parent: parent.clone(),
            index,
            nodes,
        }
        .apply(&mut target);
        assert!(matches!(result, Err(OpError::MalformedSubtree(_))), "{result:?}");
        assert_eq!(target, removed);
    };

    // `b` désigne `a` comme parent mais n'est pas dans ses enfants.
    let mut orphan = nodes.clone();
    orphan[0].children.clear();
    rejects(orphan);

    // `a` liste un nœud existant hors du sous-arbre.
    let mut foreign = nodes.clone();
    foreign[0].children.push(x.clone());
    rejects(foreign);

    // `a` liste `b` deux fois.
    let mut twice = nodes.clone();
    twice[0].children.push(b.clone());
    rejects(twice);

    // `a` liste `b`, qui désigne un autre parent.
    let mut wrong_parent = nodes.clone();
    wrong_parent[1].parent = Some(root.clone());
    rejects(wrong_parent);

    // Cycle détaché de la racine : `b` et `c` se désignent l'un l'autre.
    let mut cycle = nodes.clone();
    let mut c = cycle[1].clone();
    c.id = "n_zzzzzzzzzz".parse().unwrap();
    assert!(!removed.nodes.contains_key(&c.id));
    cycle[0].children.clear();
    cycle[1].parent = Some(c.id.clone());
    cycle[1].children = vec![c.id.clone()];
    c.parent = Some(b.clone());
    c.children = vec![b.clone()];
    cycle.push(c);
    rejects(cycle);

    // Le sous-arbre d'origine se réinsère et l'inverse le retire exactement.
    let mut target = removed.clone();
    let inverse = reinsert.apply(&mut target).unwrap();
    assert_eq!(target, original);
    inverse.apply(&mut target).unwrap();
    assert_eq!(target, removed);
}

// Relecture, constats 0 et 3 : une copie (`duplicate_node`, `detach_instance`) embarque les
// valeurs du brouillon, donc les modifications d'une unité rejetée.

fn add_card(session: &mut Session) -> (NodeId, NodeId) {
    let root = home_root(session);
    let applied = run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$card", "kind": { "type": "Box" }, "meta": { "name": "Card" } },
            { "ref": "$label", "parent": "$card", "kind": { "type": "Text", "content": [{ "text": "Rapide" }] } }
        ]}]),
    );
    (applied.created["$card"].clone(), applied.created["$label"].clone())
}

#[test]
fn duplicating_a_node_edited_by_a_rejected_unit_is_a_unit_dependency() {
    let mut session = session();
    let a = add_box(&mut session, "a");
    let b = add_box(&mut session, "b");
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "set_style", "node": a.to_string(), "style": { "padding_top": { "md": "4" } } },
            { "op": "set_style", "node": b.to_string(), "style": { "padding_top": { "md": "4" } } },
            { "op": "duplicate_node", "node": a.to_string() }
        ]),
    );
    let units = session.draft_units();
    assert_eq!(units.len(), 3);
    // La copie porte la surcharge `md` de l'unité 0.
    assert_unit_dependency(&mut session, vec![units[2].id]);
    assert_unit_dependency(&mut session, vec![units[1].id, units[2].id]);

    // Le style rejeté d'un autre nœud n'empêche pas la copie.
    session
        .commit_draft(DraftAccept::Units(vec![units[0].id, units[2].id]))
        .unwrap();
    let root = home_root(&session);
    let doc = session.document();
    assert!(doc.nodes[&b].style.padding_top.is_none());
    let copy = &doc.nodes[&doc.nodes[&root].children[1]];
    assert_ne!(copy.id, a);
    assert_eq!(copy.style.padding_top, doc.nodes[&a].style.padding_top);
}

#[test]
fn detaching_an_instance_whose_component_was_edited_by_a_rejected_unit_is_a_unit_dependency() {
    let mut session = session();
    let (card, label) = add_card(&mut session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": card.to_string(), "name": "Card", "props": [] }]),
    );
    let instance = applied.inserted[0].clone();
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "set_style", "node": label.to_string(), "style": { "padding_top": { "md": "4" } } },
            { "op": "detach_instance", "node": instance.to_string() }
        ]),
    );
    let units = session.draft_units();
    assert_eq!(units.len(), 2);
    // Le libellé copié porte la surcharge rejetée.
    assert_unit_dependency(&mut session, vec![units[1].id]);
    session.discard_draft();

    // Même chose pour la racine du composant.
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "set_style", "node": card.to_string(), "style": { "padding_top": { "base": "4" } } },
            { "op": "detach_instance", "node": instance.to_string() }
        ]),
    );
    let units = session.draft_units();
    assert_unit_dependency(&mut session, vec![units[1].id]);
}

#[test]
fn a_neutral_value_inherited_from_a_rejected_edit_is_a_unit_dependency() {
    let mut session = session();
    let (card, label) = add_card(&mut session);
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "set_style", "node": card.to_string(), "style": { "font_size": { "base": "xl" } } },
            { "op": "set_style", "node": label.to_string(), "style": { "font_size": { "md": "2xl" } } }
        ]),
    );
    let units = session.draft_units();
    // La base posée sur le libellé est la taille héritée de la carte dans le brouillon (`xl`).
    assert_unit_dependency(&mut session, vec![units[1].id]);
}

// Relecture, constats 1 et 4 : un retrait rejoué sur le document d'avant le brouillon emporte
// les nœuds qu'une unité rejetée avait supprimés ou sortis du sous-arbre.

#[test]
fn unwrapping_after_a_rejected_child_deletion_is_a_unit_dependency() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$w", "kind": { "type": "Box" } },
            { "ref": "$d", "parent": "$w", "kind": { "type": "Box" } },
            { "ref": "$e", "parent": "$w", "kind": { "type": "Box" } }
        ]}]),
    );
    let (w, d, e) = (
        applied.created["$w"].clone(),
        applied.created["$d"].clone(),
        applied.created["$e"].clone(),
    );
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "delete_nodes", "nodes": [d.to_string()] },
            { "op": "unwrap_node", "node": w.to_string() }
        ]),
    );
    let units = session.draft_units();
    // Rejoué seul, le retrait de `w` supprimerait `d`, dont la suppression est rejetée.
    assert_unit_dependency(&mut session, vec![units[1].id]);
    session.discard_draft();

    // Le déplacement rejeté d'un enfant hors du nœud aussi.
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "move_node", "node": e.to_string(), "parent": root.to_string(), "index": 0 },
            { "op": "unwrap_node", "node": w.to_string() }
        ]),
    );
    let units = session.draft_units();
    assert_unit_dependency(&mut session, vec![units[1].id]);
    session.discard_draft();

    // Une suppression simple reste indépendante : supprimer `w` sur le document d'origine
    // emporte `d`, comme la commande le ferait.
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "delete_nodes", "nodes": [d.to_string()] },
            { "op": "delete_nodes", "nodes": [w.to_string()] }
        ]),
    );
    let units = session.draft_units();
    session.commit_draft(DraftAccept::Units(vec![units[1].id])).unwrap();
    let doc = session.document();
    assert!(!doc.nodes.contains_key(&w));
    assert!(!doc.nodes.contains_key(&d));
    assert!(!doc.nodes.contains_key(&e));
}

#[test]
fn creating_a_component_after_a_rejected_child_removal_is_a_unit_dependency() {
    let mut session = session();
    let root = home_root(&session);
    let (card, label) = add_card(&mut session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": card.to_string(), "index": 0, "nodes": [
            { "ref": "$d", "kind": { "type": "Box" } }
        ]}]),
    );
    let d = applied.created["$d"].clone();
    let base = session.document().clone();
    for rejected in [
        json!({ "op": "delete_nodes", "nodes": [d.to_string()] }),
        json!({ "op": "move_node", "node": d.to_string(), "parent": root.to_string(), "index": 0 }),
    ] {
        session.begin_draft("run-1").unwrap();
        ai_run(
            &mut session,
            json!([rejected, { "op": "create_component", "node": card.to_string(), "name": "Card", "props": [] }]),
        );
        let units = session.draft_units();
        assert_eq!(units.len(), 2);
        // Le composant rejoué seul perdrait `d` : supprimé avec la carte, absent du composant.
        assert_unit_dependency(&mut session, vec![units[1].id]);
        session.discard_draft();
        assert_eq!(*session.document(), base);
    }
    assert!(session.document().nodes[&card].children.contains(&label));
}

// Relecture, constats 2 et 5 : un style qui nomme un token créé par une unité rejetée.

#[test]
fn a_style_naming_a_token_created_by_a_rejected_unit_is_a_unit_dependency() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$t", "kind": { "type": "Text", "content": [{ "text": "Bonjour" }] } }
        ]}]),
    );
    let t = applied.created["$t"].clone();
    let a = add_box(&mut session, "a");
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "set_token", "edit": { "kind": "Color", "name": "brand", "value": { "light": "#1D4ED8", "dark": "#BFDBFE" } } },
            { "op": "set_token", "edit": { "kind": "Font", "name": "display", "value": { "kind": "System", "stack": "serif" } } },
            { "op": "set_style", "node": t.to_string(), "style": { "text_color": { "base": "brand" } } },
            { "op": "set_style", "node": a.to_string(), "style": { "background": { "base": { "kind": "Color", "color": "brand" } } } },
            { "op": "set_style", "node": t.to_string(), "style": { "font_family": { "base": "display" } } },
            { "op": "insert_nodes", "parent": a.to_string(), "nodes": [
                { "kind": { "type": "Text", "content": [{ "text": "Nouveau" }] }, "style": { "text_color": { "base": "brand" } } }
            ]},
            { "op": "set_style", "node": t.to_string(), "style": { "padding_top": { "base": "2" } } }
        ]),
    );
    let units = session.draft_units();
    assert_eq!(units.len(), 7);
    for unit in &units[2..6] {
        assert_unit_dependency(&mut session, vec![unit.id]);
    }
    assert_unit_dependency(&mut session, vec![units[0].id, units[4].id]);
    // Avec le token qu'elle nomme, l'unité est acceptable.
    session
        .commit_draft(DraftAccept::Units(vec![units[0].id, units[2].id, units[6].id]))
        .unwrap();
    let doc = session.document();
    assert!(has_color_token(doc, "brand"));
    assert!(doc.tokens.font(&"display".parse().unwrap()).is_none());
    assert!(doc.nodes[&t].style.text_color.is_some());
    assert!(doc.nodes[&a].style.background.is_none());
}

// Relecture, constat 6 : corriger une erreur et en introduire une autre au même emplacement.

#[test]
fn a_new_error_in_the_slot_of_a_fixed_error_blocks_the_draft() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$t", "kind": { "type": "Text", "content": [{ "text": "Bonjour" }] },
              "style": { "text_color": { "base": "missing" } } }
        ]}]),
    );
    let t = applied.created["$t"].clone();
    let invalid_reference = |issue: &Issue, token: &str| {
        issue.code == IssueCode::InvalidReference && issue.node.as_ref() == Some(&t) && issue.message.contains(token)
    };
    assert!(
        session
            .validate()
            .iter()
            .any(|i| i.is_error() && invalid_reference(i, "`missing`"))
    );
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([
            { "op": "set_token", "edit": { "kind": "Color", "name": "missing", "value": { "light": "#111111", "dark": "#EEEEEE" } } },
            { "op": "set_style", "node": t.to_string(), "style": { "font_family": { "base": "nosuchfont" } } }
        ]),
    );
    let error = session.commit_draft(DraftAccept::All).unwrap_err();
    let CommandError::ValidationFailed(issues) = error else {
        panic!("validation failure expected, got {error:?}")
    };
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert!(invalid_reference(&issues[0], "`nosuchfont`"));
    assert!(session.has_draft());

    // La correction seule passe.
    session.discard_draft();
    session.begin_draft("run-1").unwrap();
    ai_run(
        &mut session,
        json!([{ "op": "set_token", "edit": { "kind": "Color", "name": "missing", "value": { "light": "#111111", "dark": "#EEEEEE" } } }]),
    );
    session.commit_draft(DraftAccept::All).unwrap();
}
