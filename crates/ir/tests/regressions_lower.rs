//! Régressions de l'abaissement des commandes (revue de l'étape a) : périmètre de sélection,
//! copies de `RawCode` par l'IA, propriétés de placement et slots des nœuds déplacés, extraction
//! de composants, surcharges de variantes, suppression de pages liées, prop `Visible`. Puis
//! relecture du correctif : slot et placement d'un nœud déplacé dans une instance, références
//! qui franchissent la frontière d'un composant extrait, plateforme et échappatoires web au
//! détachement, placement cédé au conteneur à l'enveloppement.

mod common;

use common::*;
use ir::command::Command;
use ir::*;
use serde_json::json;

/// Accueil : main > hero (titre, bouton).
fn landing(session: &mut Session) -> Applied {
    let root = home_root(session);
    run_json(
        session,
        json!([{
            "op": "insert_nodes", "parent": root.to_string(),
            "nodes": [
                { "ref": "$main", "kind": { "type": "Box", "role": { "kind": "Main" } } },
                { "ref": "$hero", "parent": "$main", "kind": { "type": "Stack", "role": { "kind": "Section" } } },
                { "ref": "$title", "parent": "$hero",
                  "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": [{ "text": "Bonjour" }] } },
                { "ref": "$cta", "parent": "$hero", "kind": { "type": "Button", "label": "Commencer" } }
            ]
        }]),
    )
}

fn selection(root: &NodeId) -> Scope {
    Scope {
        subtree: Some(vec![root.clone()]),
        ..Scope::full()
    }
}

fn issues_with(session: &Session, code: IssueCode) -> Vec<Issue> {
    session.validate().into_iter().filter(|i| i.code == code).collect()
}

fn raw_code_count(doc: &Document) -> usize {
    doc.nodes
        .values()
        .filter(|n| matches!(n.kind, NodeKind::RawCode(_)))
        .count()
}

/// Composant `Panel` à deux slots (`children`, `aside`) et une instance sur l'accueil.
fn panel(session: &mut Session) -> (ComponentId, NodeId) {
    let root = home_root(session);
    let applied = run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$panel", "kind": { "type": "Stack" } },
            { "parent": "$panel", "kind": { "type": "Slot" } },
            { "parent": "$panel", "kind": { "type": "Slot", "name": "aside" } }
        ]}]),
    );
    let node = applied.created["$panel"].clone();
    let applied = run_json(
        session,
        json!([{ "op": "create_component", "node": node.to_string(), "name": "Panel", "props": [] }]),
    );
    (applied.components[0].clone(), applied.inserted[0].clone())
}

// ---------------------------------------------------------------- périmètre de sélection (#4)

#[test]
fn selection_root_cannot_be_duplicated_into_its_parent() {
    let mut session = session();
    let applied = landing(&mut session);
    let hero = applied.created["$hero"].clone();
    let before = session.document().clone();
    let error = try_run(
        &mut session,
        ai(),
        selection(&hero),
        json!([{ "op": "duplicate_node", "node": hero.to_string() }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
    assert_eq!(*session.document(), before);
    // Un nœud intérieur reste duplicable : la copie reste dans la sélection.
    try_run(
        &mut session,
        ai(),
        selection(&hero),
        json!([{ "op": "duplicate_node", "node": applied.created["$title"].to_string() }]),
    )
    .unwrap();
}

#[test]
fn selection_root_cannot_be_wrapped_in_its_parent() {
    let mut session = session();
    let applied = landing(&mut session);
    let hero = applied.created["$hero"].clone();
    let before = session.document().clone();
    let error = try_run(
        &mut session,
        ai(),
        selection(&hero),
        json!([{ "op": "wrap_nodes", "nodes": [hero.to_string()], "container": { "kind": "Box" } }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
    assert_eq!(*session.document(), before);
    try_run(
        &mut session,
        ai(),
        selection(&hero),
        json!([{ "op": "wrap_nodes", "nodes": [applied.created["$title"].to_string(), applied.created["$cta"].to_string()],
                 "container": { "kind": "Stack" } }]),
    )
    .unwrap();
}

#[test]
fn selection_root_cannot_be_unwrapped_into_its_parent() {
    let mut session = session();
    let applied = landing(&mut session);
    let hero = applied.created["$hero"].clone();
    let before = session.document().clone();
    let error = try_run(
        &mut session,
        ai(),
        selection(&hero),
        json!([{ "op": "unwrap_node", "node": hero.to_string() }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
    assert_eq!(*session.document(), before);
}

#[test]
fn selection_root_cannot_be_replaced_in_its_parent() {
    let mut session = session();
    let applied = landing(&mut session);
    let hero = applied.created["$hero"].clone();
    let before = session.document().clone();
    let error = try_run(
        &mut session,
        ai(),
        selection(&hero),
        json!([{ "op": "replace_node", "node": hero.to_string(), "nodes": [{ "kind": { "type": "Box" } }] }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
    assert_eq!(*session.document(), before);
    try_run(
        &mut session,
        ai(),
        selection(&hero),
        json!([{ "op": "replace_node", "node": applied.created["$title"].to_string(),
                 "nodes": [{ "kind": { "type": "Text", "content": [{ "text": "Salut" }] } }] }]),
    )
    .unwrap();
}

#[test]
fn selection_root_cannot_be_swapped_for_an_instance_or_a_detached_copy() {
    let mut session = session();
    let applied = landing(&mut session);
    let hero = applied.created["$hero"].clone();
    let error = try_run(
        &mut session,
        ai(),
        selection(&hero),
        json!([{ "op": "create_component", "node": hero.to_string(), "name": "Hero", "props": [] }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");

    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": hero.to_string(), "name": "Hero", "props": [] }]),
    );
    let instance = applied.inserted[0].clone();
    let before = session.document().clone();
    let error = try_run(
        &mut session,
        ai(),
        selection(&instance),
        json!([{ "op": "detach_instance", "node": instance.to_string() }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
    assert_eq!(*session.document(), before);
}

// ---------------------------------------------------------------- RawCode copié par l'IA (#5, #11)

#[test]
fn ai_cannot_create_raw_code_by_duplication() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$wrap", "kind": { "type": "Box" } },
            { "ref": "$raw", "parent": "$wrap", "kind": { "type": "RawCode", "code": "<div/>" } }
        ]}]),
    );
    for node in ["$raw", "$wrap"] {
        let error = try_run(
            &mut session,
            ai(),
            Scope::full(),
            json!([{ "op": "duplicate_node", "node": applied.created[node].to_string() }]),
        )
        .unwrap_err();
        assert_eq!(error.code(), "FORBIDDEN", "{node}");
        assert_eq!(raw_code_count(session.document()), 1);
    }
    // L'utilisateur garde le droit de dupliquer.
    run(
        &mut session,
        vec![Command::DuplicateNode {
            node: r(&applied.created["$raw"]),
        }],
    );
    assert_eq!(raw_code_count(session.document()), 2);
}

#[test]
fn ai_cannot_create_raw_code_by_detaching_an_instance() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$embed", "kind": { "type": "Box" } },
            { "parent": "$embed", "kind": { "type": "RawCode", "code": "<div/>" } }
        ]}]),
    );
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": applied.created["$embed"].to_string(), "name": "Embed", "props": [] }]),
    );
    let instance = applied.inserted[0].clone();
    let error = try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([{ "op": "detach_instance", "node": instance.to_string() }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "FORBIDDEN");
    assert_eq!(raw_code_count(session.document()), 1);
    run(&mut session, vec![Command::DetachInstance { node: r(&instance) }]);
    assert_eq!(raw_code_count(session.document()), 2);
}

// ---------------------------------------------------------------- nœuds déplacés (#6)

/// Grille (carte avec `col_span` et `align_self`), Stack (bouton qui `grow`) et Stack vide.
fn placement(session: &mut Session) -> Applied {
    let root = home_root(session);
    run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$grid", "kind": { "type": "Grid" }, "style": { "columns": { "base": "2" } } },
            { "ref": "$card", "parent": "$grid", "kind": { "type": "Box" },
              "style": { "col_span": { "base": "2" }, "align_self": { "base": "center" } } },
            { "parent": "$card", "kind": { "type": "Text", "content": [{ "text": "Carte" }] } },
            { "ref": "$row", "kind": { "type": "Stack" } },
            { "ref": "$btn", "parent": "$row", "kind": { "type": "Button", "label": "Go" }, "style": { "grow": { "base": true } } },
            { "ref": "$column", "kind": { "type": "Stack" } }
        ]}]),
    )
}

#[test]
fn wrap_drops_placement_props_the_new_container_does_not_allow() {
    let mut session = session();
    let applied = placement(&mut session);
    let card = applied.created["$card"].clone();
    assert!(issues_with(&session, IssueCode::StyleNotAllowed).is_empty());
    run_json(
        &mut session,
        json!([{ "op": "wrap_nodes", "nodes": [card.to_string()], "container": { "kind": "Box" } }]),
    );
    let style = &session.document().nodes[&card].style;
    assert!(style.col_span.is_none());
    assert!(style.align_self.is_none());
    let errors = issues_with(&session, IssueCode::StyleNotAllowed);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn unwrap_drops_placement_props_the_new_parent_does_not_allow() {
    let mut session = session();
    let applied = placement(&mut session);
    let btn = applied.created["$btn"].clone();
    run(
        &mut session,
        vec![Command::UnwrapNode {
            node: r(&applied.created["$row"]),
        }],
    );
    assert!(session.document().nodes[&btn].style.grow.is_none());
    let errors = issues_with(&session, IssueCode::StyleNotAllowed);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn move_drops_only_the_placement_props_the_new_parent_does_not_allow() {
    let mut session = session();
    let applied = placement(&mut session);
    let card = applied.created["$card"].clone();
    run(
        &mut session,
        vec![Command::MoveNode {
            node: r(&card),
            parent: r(&applied.created["$column"]),
            index: 0,
        }],
    );
    let style = &session.document().nodes[&card].style;
    assert!(style.col_span.is_none(), "col_span needs a Grid parent");
    assert!(style.align_self.is_some(), "align_self is valid in a Stack");
    let errors = issues_with(&session, IssueCode::StyleNotAllowed);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn moved_instance_children_keep_a_consistent_slot() {
    let mut session = session();
    let (_, instance) = panel(&mut session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": instance.to_string(), "nodes": [
            { "ref": "$note", "kind": { "type": "Text", "content": [{ "text": "Note" }] }, "meta": { "slot": "aside" } },
            { "ref": "$more", "kind": { "type": "Text", "content": [{ "text": "Plus" }] }, "meta": { "slot": "aside" } },
            { "ref": "$body", "kind": { "type": "Text", "content": [{ "text": "Corps" }] } }
        ]}]),
    );
    let (note, more, body) = (
        applied.created["$note"].clone(),
        applied.created["$more"].clone(),
        applied.created["$body"].clone(),
    );
    assert!(issues_with(&session, IssueCode::InvalidSlotTarget).is_empty());

    // Des enfants qui visent des slots différents ne peuvent pas partager un conteneur.
    let error = try_run(
        &mut session,
        Origin::User,
        Scope::full(),
        json!([{ "op": "wrap_nodes", "nodes": [more.to_string(), body.to_string()], "container": { "kind": "Box" } }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "INVALID_COMMAND");

    // Envelopper : le conteneur reprend le slot, les enfants n'en ciblent plus.
    let applied = run_json(
        &mut session,
        json!([{ "op": "wrap_nodes", "nodes": [note.to_string(), more.to_string()],
                 "container": { "ref": "$side", "kind": "Stack" } }]),
    );
    let side = applied.created["$side"].clone();
    let doc = session.document();
    assert_eq!(doc.nodes[&side].meta.slot.as_deref(), Some("aside"));
    assert_eq!(doc.nodes[&note].meta.slot, None);
    let errors = issues_with(&session, IssueCode::InvalidSlotTarget);
    assert!(errors.is_empty(), "{errors:#?}");

    // Désenvelopper : les enfants reprennent le slot du conteneur.
    run(&mut session, vec![Command::UnwrapNode { node: r(&side) }]);
    assert_eq!(session.document().nodes[&note].meta.slot.as_deref(), Some("aside"));
    assert!(issues_with(&session, IssueCode::InvalidSlotTarget).is_empty());

    // Sortir d'une instance efface le slot.
    let root = home_root(&session);
    run(
        &mut session,
        vec![Command::MoveNode {
            node: r(&note),
            parent: r(&root),
            index: 0,
        }],
    );
    assert_eq!(session.document().nodes[&note].meta.slot, None);
    let errors = issues_with(&session, IssueCode::InvalidSlotTarget);
    assert!(errors.is_empty(), "{errors:#?}");
}

// ---------------------------------------------------------------- détachement (#7)

#[test]
fn detach_keeps_the_instance_slot_a11y_and_lock() {
    let mut session = session();
    let (_, panel_instance) = panel(&mut session);
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$badge", "kind": { "type": "Box" } },
            { "parent": "$badge", "kind": { "type": "Text", "content": [{ "text": "Nouveau" }] } }
        ]}]),
    );
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": applied.created["$badge"].to_string(), "name": "Badge", "props": [] }]),
    );
    let badge = applied.inserted[0].clone();
    run_json(
        &mut session,
        json!([
            { "op": "move_node", "node": badge.to_string(), "parent": panel_instance.to_string(), "index": 0 },
            { "op": "set_meta", "node": badge.to_string(),
              "meta": { "slot": "aside", "locked": true, "a11y_label": "Badge", "a11y_hidden": true } }
        ]),
    );
    let applied = run(&mut session, vec![Command::DetachInstance { node: r(&badge) }]);
    let copy = &session.document().nodes[&applied.inserted[0]];
    assert_eq!(copy.parent.as_ref(), Some(&panel_instance));
    assert_eq!(copy.meta.slot.as_deref(), Some("aside"));
    assert!(copy.meta.locked);
    assert!(copy.a11y.hidden);
    assert_eq!(copy.a11y.label.as_deref(), Some("Badge"));
}

// ---------------------------------------------------------------- création de composant (#8, #9)

#[test]
fn create_component_moves_the_slot_to_the_instance() {
    let mut session = session();
    let (_, instance) = panel(&mut session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": instance.to_string(), "nodes": [
            { "ref": "$side", "kind": { "type": "Box" }, "meta": { "slot": "aside" } },
            { "parent": "$side", "kind": { "type": "Text", "content": [{ "text": "À côté" }] } }
        ]}]),
    );
    let side = applied.created["$side"].clone();
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": side.to_string(), "name": "Side", "props": [] }]),
    );
    let doc = session.document();
    assert_eq!(doc.nodes[&applied.inserted[0]].meta.slot.as_deref(), Some("aside"));
    assert_eq!(doc.nodes[&side].meta.slot, None);
    let errors = issues_with(&session, IssueCode::InvalidSlotTarget);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn create_component_refuses_nodes_the_enclosing_component_depends_on() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$card", "kind": { "type": "Box" } },
            { "ref": "$inner", "parent": "$card", "kind": { "type": "Box" } },
            { "ref": "$t", "parent": "$inner", "kind": { "type": "Text", "content": [{ "text": "Titre" }] } },
            { "ref": "$tinted", "parent": "$card", "kind": { "type": "Box" } },
            { "ref": "$free", "parent": "$card", "kind": { "type": "Box" } },
            { "parent": "$free", "kind": { "type": "Text", "content": [{ "text": "Libre" }] } }
        ]}]),
    );
    let (card, inner, tinted, free) = (
        applied.created["$card"].clone(),
        applied.created["$inner"].clone(),
        applied.created["$tinted"].clone(),
        applied.created["$free"].clone(),
    );
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": card.to_string(), "name": "Card",
                 "props": [{ "name": "title", "node": applied.created["$t"].to_string(), "field": "Text" }] }]),
    );
    run_json(
        &mut session,
        json!([{ "op": "update_component", "component": applied.components[0].to_string(),
                 "variants": [{ "name": "tone", "default": "plain", "options": [
                     { "name": "plain", "overrides": [] },
                     { "name": "accent", "overrides": [{ "node": tinted.to_string(),
                         "style": { "background": { "base": { "kind": "Color", "color": "accent" } } } }] }
                 ]}] }]),
    );
    for (node, name) in [(&inner, "Inner"), (&tinted, "Tinted")] {
        let error = try_run(
            &mut session,
            Origin::User,
            Scope::full(),
            json!([{ "op": "create_component", "node": node.to_string(), "name": name, "props": [] }]),
        )
        .unwrap_err();
        assert_eq!(error.code(), "INVALID_COMMAND", "{name}");
    }
    let errors = issues_with(&session, IssueCode::InvalidProp);
    assert!(errors.is_empty(), "{errors:#?}");
    // Un nœud dont rien ne dépend reste extractible.
    run_json(
        &mut session,
        json!([{ "op": "create_component", "node": free.to_string(), "name": "Free", "props": [] }]),
    );
}

#[test]
fn create_component_refuses_to_take_the_layout_page_slot() {
    let mut session = session();
    let applied = run_json(&mut session, json!([{ "op": "create_layout", "name": "site" }]));
    let layout = applied.layouts[0].clone();
    let doc = session.document();
    let slot = doc.nodes[&doc.layout(&layout).unwrap().root].children[0].clone();
    let applied = run_json(
        &mut session,
        json!([{ "op": "wrap_nodes", "nodes": [slot.to_string()], "container": { "ref": "$holder", "kind": "Box" } }]),
    );
    let error = try_run(
        &mut session,
        Origin::User,
        Scope::full(),
        json!([{ "op": "create_component", "node": applied.created["$holder"].to_string(), "name": "Holder", "props": [] }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "INVALID_COMMAND");
    let errors = issues_with(&session, IssueCode::LayoutPageSlot);
    assert!(errors.is_empty(), "{errors:#?}");
}

// ---------------------------------------------------------------- surcharges de variantes (#10)

#[test]
fn update_component_rejects_invalid_variant_overrides() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$badge", "kind": { "type": "Box" } },
            { "ref": "$label", "parent": "$badge", "kind": { "type": "Text", "content": [{ "text": "Nouveau" }] } }
        ]}]),
    );
    let (badge, label) = (applied.created["$badge"].clone(), applied.created["$label"].clone());
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": badge.to_string(), "name": "Badge", "props": [] }]),
    );
    let (component, instance) = (applied.components[0].clone(), applied.inserted[0].clone());
    let variants = |over: serde_json::Value| {
        json!([{ "op": "update_component", "component": component.to_string(),
                 "variants": [{ "name": "tone", "default": "plain", "options": [
                     { "name": "plain", "overrides": [] },
                     { "name": "loud", "overrides": [over] }
                 ]}] }])
    };
    let invalid = [
        (
            json!({ "node": badge.to_string(), "style": { "background": { "base": { "kind": "Color", "color": "nope" } } } }),
            "ENTITY_NOT_FOUND",
        ),
        (
            json!({ "node": badge.to_string(), "states": { "hover": { "text_color": { "base": "nope" } } } }),
            "ENTITY_NOT_FOUND",
        ),
        (
            json!({ "node": label.to_string(), "style": { "font_family": { "base": "nope" } } }),
            "ENTITY_NOT_FOUND",
        ),
        (
            json!({ "node": badge.to_string(), "style": { "columns": { "base": "3" } } }),
            "INVALID_COMMAND",
        ),
        (
            json!({ "node": label.to_string(), "style": { "col_span": { "base": "2" } } }),
            "INVALID_COMMAND",
        ),
        (
            json!({ "node": badge.to_string(), "style": { "width": { "base": "none" } } }),
            "INVALID_COMMAND",
        ),
    ];
    for (over, code) in invalid {
        let error = try_run(&mut session, Origin::User, Scope::full(), variants(over.clone())).unwrap_err();
        assert_eq!(error.code(), code, "{over}");
    }
    assert!(session.document().component(&component).unwrap().variants.is_empty());

    // Une surcharge valide passe, et son application au détachement ne crée aucune erreur.
    let errors_before = session.validate().into_iter().filter(Issue::is_error).count();
    run_json(
        &mut session,
        variants(json!({ "node": badge.to_string(),
            "style": { "background": { "base": { "kind": "Color", "color": "accent" } }, "max_width": { "base": "none" } },
            "states": { "hover": { "background": { "base": { "kind": "Color", "color": "primary/90" } } } } })),
    );
    run_json(
        &mut session,
        json!([
            { "op": "set_props", "node": instance.to_string(),
              "props": { "type": "ComponentInstance", "variants": [{ "axis": "tone", "option": "loud" }] } },
            { "op": "detach_instance", "node": instance.to_string() }
        ]),
    );
    let errors_after = session.validate().into_iter().filter(Issue::is_error).count();
    assert_eq!(errors_after, errors_before);
}

// ---------------------------------------------------------------- suppression de page (#12)

#[test]
fn delete_page_refuses_pages_linked_from_elsewhere() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([
            { "op": "create_page", "name": "About", "route": [{ "kind": "Static", "name": "about" }] },
            { "op": "create_page", "name": "Legal", "route": [{ "kind": "Static", "name": "legal" }] },
            { "op": "create_page", "name": "Contact", "route": [{ "kind": "Static", "name": "contact" }] }
        ]),
    );
    let (about, legal, contact) = (
        applied.pages[0].clone(),
        applied.pages[1].clone(),
        applied.pages[2].clone(),
    );
    let doc = session.document();
    let contact_root = doc.page(&contact).unwrap().root.clone();
    run_json(
        &mut session,
        json!([
            { "op": "insert_nodes", "parent": root.to_string(), "nodes": [
                { "kind": { "type": "Link", "label": "À propos", "href": { "kind": "Page", "page": about.to_string() } } },
                { "ref": "$nav", "kind": { "type": "Box" } },
                { "ref": "$out", "parent": "$nav",
                  "kind": { "type": "Link", "label": "Ailleurs", "href": { "kind": "External", "url": "https://example.com" } } }
            ]},
            { "op": "insert_nodes", "parent": contact_root.to_string(), "nodes": [
                { "kind": { "type": "Link", "label": "Contact", "href": { "kind": "Page", "page": contact.to_string() } } }
            ]}
        ]),
    );
    // Lien depuis l'accueil.
    let error = session
        .apply(Transaction::user(
            "x",
            vec![Command::DeletePage { page: about.clone() }],
        ))
        .unwrap_err();
    assert_eq!(error.code(), "IN_USE");
    assert!(session.document().page(&about).is_some());

    // Défaut d'une prop de composant.
    let doc = session.document();
    let nav = doc.nodes[&root].children[1].clone();
    let out = doc.nodes[&nav].children[0].clone();
    run_json(
        &mut session,
        json!([{ "op": "create_component", "node": nav.to_string(), "name": "Nav",
                 "props": [{ "name": "target", "node": out.to_string(), "field": "Href",
                             "default": { "kind": "Href", "value": { "kind": "Page", "page": legal.to_string() } } }] }]),
    );
    let error = session
        .apply(Transaction::user(
            "x",
            vec![Command::DeletePage { page: legal.clone() }],
        ))
        .unwrap_err();
    assert_eq!(error.code(), "IN_USE");

    // Les liens de la page vers elle-même partent avec elle.
    run(&mut session, vec![Command::DeletePage { page: contact.clone() }]);
    assert!(session.document().page(&contact).is_none());
    let errors = issues_with(&session, IssueCode::InvalidReference);
    assert!(errors.is_empty(), "{errors:#?}");
}

// ---------------------------------------------------------------- prop `Visible` (#35)

#[test]
fn detached_visible_prop_set_to_true_shows_a_hidden_node() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$card", "kind": { "type": "Box" } },
            { "ref": "$badge", "parent": "$card", "kind": { "type": "Text", "content": [{ "text": "Nouveau" }] },
              "visibility": { "base": false } }
        ]}]),
    );
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": applied.created["$card"].to_string(), "name": "Card",
                 "props": [{ "name": "showBadge", "node": applied.created["$badge"].to_string(), "field": "Visible" }] }]),
    );
    let (component, instance) = (applied.components[0].clone(), applied.inserted[0].clone());
    assert_eq!(
        session.document().component(&component).unwrap().props[0].default,
        PropValue::Bool(false)
    );
    run_json(
        &mut session,
        json!([{ "op": "set_props", "node": instance.to_string(), "props": { "type": "ComponentInstance",
                 "overrides": [{ "prop": "showBadge", "value": { "kind": "Bool", "value": true } }] } }]),
    );
    let applied = run(&mut session, vec![Command::DetachInstance { node: r(&instance) }]);
    let doc = session.document();
    let badge = &doc.nodes[&doc.nodes[&applied.inserted[0]].children[0]];
    assert_eq!(BindableField::Visible.read(badge), Some(PropValue::Bool(true)));
}

// ---------------------------------------------------------------- relecture : slot d'un nœud déplacé

/// Composant `Card` (Box contenant un seul slot `children`) et une instance sur l'accueil.
fn card(session: &mut Session) -> (ComponentId, NodeId) {
    let root = home_root(session);
    let applied = run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$card", "kind": { "type": "Box" } },
            { "parent": "$card", "kind": { "type": "Slot" } }
        ]}]),
    );
    let node = applied.created["$card"].clone();
    let applied = run_json(
        session,
        json!([{ "op": "create_component", "node": node.to_string(), "name": "Card", "props": [] }]),
    );
    (applied.components[0].clone(), applied.inserted[0].clone())
}

#[test]
fn moving_into_an_instance_keeps_the_slot_only_if_its_component_has_it() {
    let mut session = session();
    let (panel_component, first_panel) = panel(&mut session);
    let (_, card_instance) = card(&mut session);
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$second", "kind": { "type": "ComponentInstance", "component": panel_component.to_string() } }
        ]}]),
    );
    let second_panel = applied.created["$second"].clone();
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": first_panel.to_string(), "nodes": [
            { "ref": "$note", "kind": { "type": "Text", "content": [{ "text": "Note" }] }, "meta": { "slot": "aside" } },
            { "ref": "$tip", "kind": { "type": "Text", "content": [{ "text": "Astuce" }] }, "meta": { "slot": "aside" } }
        ]}]),
    );
    let (note, tip) = (applied.created["$note"].clone(), applied.created["$tip"].clone());
    assert!(issues_with(&session, IssueCode::InvalidSlotTarget).is_empty());

    // `Card` n'a pas de slot `aside` : le nœud rejoint son slot par défaut.
    run(
        &mut session,
        vec![Command::MoveNode {
            node: r(&note),
            parent: r(&card_instance),
            index: 0,
        }],
    );
    assert_eq!(session.document().nodes[&note].meta.slot, None);
    // Une autre instance de `Panel` a bien un slot `aside` : il est conservé.
    run(
        &mut session,
        vec![Command::MoveNode {
            node: r(&tip),
            parent: r(&second_panel),
            index: 0,
        }],
    );
    assert_eq!(session.document().nodes[&tip].meta.slot.as_deref(), Some("aside"));
    let errors = issues_with(&session, IssueCode::InvalidSlotTarget);
    assert!(errors.is_empty(), "{errors:#?}");
}

// ---------------------------------------------------------------- relecture : placement dans une instance

/// Composant `Row` (Stack en ligne contenant un slot) et une instance sur l'accueil.
fn row(session: &mut Session) -> NodeId {
    let root = home_root(session);
    let applied = run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$row", "kind": { "type": "Stack" }, "style": { "direction": { "base": "row" } } },
            { "parent": "$row", "kind": { "type": "Slot" } }
        ]}]),
    );
    let node = applied.created["$row"].clone();
    let applied = run_json(
        session,
        json!([{ "op": "create_component", "node": node.to_string(), "name": "Row", "props": [] }]),
    );
    applied.inserted[0].clone()
}

#[test]
fn moving_into_an_instance_checks_placement_props_against_the_slot_parent() {
    let mut session = session();
    let applied = placement(&mut session);
    let (btn, grid_card) = (applied.created["$btn"].clone(), applied.created["$card"].clone());
    let row_instance = row(&mut session);
    let (_, card_instance) = card(&mut session);

    // Le slot de `Row` est rendu dans une Stack : `grow` et `align_self` restent, `col_span` part.
    run(
        &mut session,
        vec![
            Command::MoveNode {
                node: r(&btn),
                parent: r(&row_instance),
                index: 0,
            },
            Command::MoveNode {
                node: r(&grid_card),
                parent: r(&row_instance),
                index: 1,
            },
        ],
    );
    let doc = session.document();
    assert!(
        doc.nodes[&btn].style.grow.is_some(),
        "grow is valid where the slot renders"
    );
    assert!(doc.nodes[&grid_card].style.align_self.is_some());
    assert!(doc.nodes[&grid_card].style.col_span.is_none(), "col_span needs a Grid");

    // Le slot de `Card` est rendu dans une Box : `grow` n'y a pas de sens.
    run(
        &mut session,
        vec![Command::MoveNode {
            node: r(&btn),
            parent: r(&card_instance),
            index: 0,
        }],
    );
    assert!(session.document().nodes[&btn].style.grow.is_none());
}

#[test]
fn unwrapping_into_an_instance_keeps_placement_props_valid_where_the_slot_renders() {
    let mut session = session();
    let row_instance = row(&mut session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": row_instance.to_string(), "nodes": [
            { "ref": "$group", "kind": { "type": "Stack" } },
            { "ref": "$go", "parent": "$group", "kind": { "type": "Button", "label": "Go" }, "style": { "grow": { "base": true } } }
        ]}]),
    );
    let go = applied.created["$go"].clone();
    run(
        &mut session,
        vec![Command::UnwrapNode {
            node: r(&applied.created["$group"]),
        }],
    );
    let doc = session.document();
    assert_eq!(doc.nodes[&go].parent.as_ref(), Some(&row_instance));
    assert!(doc.nodes[&go].style.grow.is_some());
}

// ---------------------------------------------------------------- relecture : références et extraction

#[test]
fn create_component_refuses_references_across_the_new_component_boundary() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$menu", "kind": { "type": "Box" } },
            { "parent": "$menu", "kind": { "type": "Text", "content": [{ "text": "Menu" }] } },
            { "ref": "$toggle", "kind": { "type": "Button", "label": "Menu",
                "action": { "kind": "ToggleVisibility", "target": "$menu" } } },
            { "ref": "$field", "kind": { "type": "Box" } },
            { "ref": "$email", "parent": "$field", "kind": { "type": "Input", "input_type": "email" } },
            { "kind": { "type": "Text", "content": [{ "text": "E-mail" }], "for_input": "$email" } },
            { "ref": "$bar", "kind": { "type": "Box" } },
            { "parent": "$bar", "kind": { "type": "Button", "label": "Ouvrir",
                "action": { "kind": "ToggleVisibility", "target": "$menu" } } },
            { "ref": "$both", "kind": { "type": "Box" } },
            { "ref": "$panel", "parent": "$both", "kind": { "type": "Box" } },
            { "parent": "$panel", "kind": { "type": "Text", "content": [{ "text": "Contenu" }] } },
            { "parent": "$both", "kind": { "type": "Button", "label": "Afficher",
                "action": { "kind": "ToggleVisibility", "target": "$panel" } } }
        ]}]),
    );
    let references_before = issues_with(&session, IssueCode::InvalidReference).len();
    // Cible d'un bouton resté dehors, champ d'une étiquette restée dehors, bouton qui sort vers sa cible.
    for (node, name) in [("$menu", "Menu"), ("$field", "Field"), ("$bar", "Bar")] {
        let before = session.document().clone();
        let error = try_run(
            &mut session,
            Origin::User,
            Scope::full(),
            json!([{ "op": "create_component", "node": applied.created[node].to_string(), "name": name, "props": [] }]),
        )
        .unwrap_err();
        assert_eq!(error.code(), "INVALID_COMMAND", "{name}");
        assert_eq!(*session.document(), before);
    }
    // Un sous-arbre qui contient le bouton et sa cible reste extractible.
    run_json(
        &mut session,
        json!([{ "op": "create_component", "node": applied.created["$both"].to_string(), "name": "Both", "props": [] }]),
    );
    assert_eq!(
        issues_with(&session, IssueCode::InvalidReference).len(),
        references_before
    );
}

// ---------------------------------------------------------------- relecture : plateforme au détachement

/// Composant `Badge` (Box et texte) et une instance sur l'accueil.
fn badge(session: &mut Session) -> (NodeId, NodeId) {
    let root = home_root(session);
    let applied = run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$badge", "kind": { "type": "Box" } },
            { "parent": "$badge", "kind": { "type": "Text", "content": [{ "text": "Nouveau" }] } }
        ]}]),
    );
    let node = applied.created["$badge"].clone();
    let applied = run_json(
        session,
        json!([{ "op": "create_component", "node": node.to_string(), "name": "Badge", "props": [] }]),
    );
    (node, applied.inserted[0].clone())
}

#[test]
fn detach_keeps_the_instance_platform_and_web_overrides() {
    let mut session = session();
    let (component_root, instance) = badge(&mut session);
    run_json(
        &mut session,
        json!([
            { "op": "set_web_overrides", "node": component_root.to_string(), "overrides": {
                "extra_classes": ["rounded-full"],
                "extra_attributes": [{ "name": "data-kind", "value": "pill" }, { "name": "data-testid", "value": "root" }] } },
            { "op": "set_platform", "node": instance.to_string(), "scope": "WebOnly" },
            { "op": "set_web_overrides", "node": instance.to_string(), "overrides": {
                "extra_classes": ["backdrop-blur"],
                "extra_attributes": [{ "name": "data-testid", "value": "badge" }] } }
        ]),
    );
    let applied = run(&mut session, vec![Command::DetachInstance { node: r(&instance) }]);
    let copy = &session.document().nodes[&applied.inserted[0]];
    assert_eq!(copy.platform, PlatformScope::WebOnly);
    let web = copy.platform_overrides.web.as_ref().expect("web overrides are kept");
    assert_eq!(
        web.extra_classes,
        vec!["rounded-full".to_owned(), "backdrop-blur".to_owned()]
    );
    let attributes: Vec<(&str, &str)> = web
        .extra_attributes
        .iter()
        .map(|a| (a.name.as_str(), a.value.as_str()))
        .collect();
    assert_eq!(attributes, vec![("data-kind", "pill"), ("data-testid", "badge")]);
}

#[test]
fn detach_refuses_an_instance_whose_platform_excludes_its_component_root() {
    let mut session = session();
    let (component_root, instance) = badge(&mut session);
    run_json(
        &mut session,
        json!([
            { "op": "set_platform", "node": component_root.to_string(), "scope": "NativeOnly" },
            { "op": "set_platform", "node": instance.to_string(), "scope": "WebOnly" }
        ]),
    );
    let before = session.document().clone();
    let error = try_run(
        &mut session,
        Origin::User,
        Scope::full(),
        json!([{ "op": "detach_instance", "node": instance.to_string() }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "INVALID_COMMAND");
    assert_eq!(*session.document(), before);
}

// ---------------------------------------------------------------- relecture : placement à l'enveloppement

#[test]
fn wrap_moves_the_placement_of_a_single_node_to_the_container() {
    let mut session = session();
    let applied = placement(&mut session);
    let card = applied.created["$card"].clone();
    run_json(
        &mut session,
        json!([{ "op": "set_style", "node": card.to_string(), "style": { "margin_top": { "base": "4" } } }]),
    );
    let original = session.document().nodes[&card].style.clone();
    let applied = run_json(
        &mut session,
        json!([{ "op": "wrap_nodes", "nodes": [card.to_string()], "container": { "ref": "$wrapper", "kind": "Box" } }]),
    );
    let doc = session.document();
    let wrapper = &doc.nodes[&applied.created["$wrapper"]].style;
    assert_eq!(wrapper.col_span, original.col_span);
    assert_eq!(wrapper.align_self, original.align_self);
    assert_eq!(wrapper.margin_top, original.margin_top);
    let moved = &doc.nodes[&card].style;
    assert!(moved.col_span.is_none() && moved.align_self.is_none() && moved.margin_top.is_none());
    let errors = issues_with(&session, IssueCode::StyleNotAllowed);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn wrap_moves_placement_shared_by_all_wrapped_nodes_to_the_container() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$bar", "kind": { "type": "Stack" }, "style": { "direction": { "base": "row" } } },
            { "ref": "$a", "parent": "$bar", "kind": { "type": "Button", "label": "A" },
              "style": { "grow": { "base": true }, "order": { "base": "first" }, "margin_top": { "base": "2" } } },
            { "ref": "$b", "parent": "$bar", "kind": { "type": "Button", "label": "B" },
              "style": { "grow": { "base": true }, "margin_top": { "base": "2" } } }
        ]}]),
    );
    let (a, b) = (applied.created["$a"].clone(), applied.created["$b"].clone());
    let applied = run_json(
        &mut session,
        json!([{ "op": "wrap_nodes", "nodes": [a.to_string(), b.to_string()], "container": { "ref": "$pair", "kind": "Box" } }]),
    );
    let doc = session.document();
    let pair = &doc.nodes[&applied.created["$pair"]].style;
    assert!(pair.grow.is_some(), "shared grow moves to the container");
    assert!(pair.order.is_none(), "order is not shared");
    assert!(
        pair.margin_top.is_none(),
        "margins space the wrapped nodes among themselves"
    );
    for id in [&a, &b] {
        assert!(doc.nodes[id].style.grow.is_none());
        assert!(doc.nodes[id].style.margin_top.is_some());
    }
    let errors = issues_with(&session, IssueCode::StyleNotAllowed);
    assert!(errors.is_empty(), "{errors:#?}");
}
