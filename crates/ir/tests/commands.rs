//! Commandes : création, structure, style, composants, pages, tokens, périmètres, atomicité.

mod common;

use common::*;
use ir::command::Command;
use ir::*;
use serde_json::json;

fn landing(session: &mut Session) -> Applied {
    let root = home_root(session);
    run_json(
        session,
        json!([{
            "op": "insert_nodes", "parent": root.to_string(),
            "nodes": [
                { "ref": "$main", "kind": { "type": "Box", "role": { "kind": "Main" } } },
                { "ref": "$hero", "parent": "$main", "kind": { "type": "Stack", "role": { "kind": "Section" } },
                  "meta": { "name": "Hero", "anchor": "hero" } },
                { "ref": "$title", "parent": "$hero",
                  "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": [{ "text": "Bonjour" }] } },
                { "ref": "$cta", "parent": "$hero", "kind": { "type": "Button", "label": "Commencer" } },
                { "ref": "$grid", "parent": "$main", "kind": { "type": "Grid", "role": { "kind": "List" } },
                  "style": { "columns": { "base": "1", "md": "3" }, "gap": { "base": "6" } } },
                { "ref": "$card", "parent": "$grid", "kind": { "type": "Box", "role": { "kind": "ListItem" } } },
                { "ref": "$card-title", "parent": "$card", "kind": { "type": "Text", "content": [{ "text": "Rapide" }] } }
            ]
        }]),
    )
}

#[test]
fn insert_nodes_builds_a_flat_tree_with_explicit_defaults() {
    let mut session = session();
    let applied = landing(&mut session);
    let doc = session.document();
    let hero = &applied.created["$hero"];
    let cta = &applied.created["$cta"];
    let main = &applied.created["$main"];
    assert_eq!(applied.inserted, vec![main.clone()]);
    assert_eq!(doc.nodes[main].children.len(), 2);
    assert_eq!(
        doc.nodes[hero].children,
        vec![applied.created["$title"].clone(), cta.clone()]
    );
    // Défauts explicites : Stack en colonne, anneau de focus sur le bouton.
    assert_eq!(
        doc.nodes[hero].style.direction.as_ref().unwrap().base,
        Direction::Column
    );
    assert!(doc.nodes[cta].states.focus_visible.ring.is_some());
    assert_eq!(doc.nodes[hero].meta.anchor.as_deref(), Some("hero"));
    assert_eq!(doc.nodes[hero].meta.source, NodeSource::Visual);
    // Aucune erreur bloquante sur cette landing minimale.
    let errors: Vec<_> = applied.issues.iter().filter(|i| i.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn nodespec_parent_must_reference_a_previous_spec() {
    let mut session = session();
    let root = home_root(&session);
    let error = try_run(
        &mut session,
        Origin::User,
        Scope::full(),
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "parent": "$later", "kind": { "type": "Text" } },
            { "ref": "$later", "kind": { "type": "Box" } }
        ]}]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "UNKNOWN_REF");
}

#[test]
fn leaves_reject_children_and_refs_are_unique() {
    let mut session = session();
    let applied = landing(&mut session);
    let title = applied.created["$title"].to_string();
    let error = try_run(
        &mut session,
        Origin::User,
        Scope::full(),
        json!([{ "op": "insert_nodes", "parent": title, "nodes": [{ "kind": { "type": "Text" } }] }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "INVALID_PARENT");

    let root = home_root(&session).to_string();
    let error = try_run(
        &mut session,
        Origin::User,
        Scope::full(),
        json!([
            { "op": "insert_nodes", "parent": root, "nodes": [{ "ref": "$a", "kind": { "type": "Box" } }] },
            { "op": "insert_nodes", "parent": root, "nodes": [{ "ref": "$a", "kind": { "type": "Box" } }] }
        ]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "DUPLICATE_REF");
}

#[test]
fn a_failed_transaction_changes_nothing() {
    let mut session = session();
    landing(&mut session);
    let before = session.document().clone();
    let root = home_root(&session).to_string();
    let result = try_run(
        &mut session,
        Origin::User,
        Scope::full(),
        json!([
            { "op": "insert_nodes", "parent": root, "nodes": [{ "kind": { "type": "Box" } }] },
            { "op": "delete_nodes", "nodes": ["n_0000000000"] }
        ]),
    );
    assert!(matches!(result, Err(CommandError::InCommand { index: 1, .. })));
    assert_eq!(*session.document(), before);
    assert_eq!(session.undo_labels().len(), 1);
}

#[test]
fn overriding_lg_on_an_absent_property_keeps_base_neutral() {
    let mut session = session();
    let applied = landing(&mut session);
    let hero = applied.created["$hero"].clone();
    let title = applied.created["$title"].clone();
    run(
        &mut session,
        vec![
            Command::SetStyle {
                node: r(&hero),
                state: None,
                style: json(json!({ "gap": { "lg": "8" } })),
            },
            Command::SetStyle {
                node: r(&title),
                state: None,
                style: json(json!({ "font_size": { "lg": "5xl" } })),
            },
        ],
    );
    let doc = session.document();
    let gap = doc.nodes[&hero].style.gap.as_ref().unwrap();
    assert_eq!(gap.base, Space::S0);
    assert_eq!(gap.lg, Some(Space::S8));
    assert_eq!(gap.declared(), vec![Breakpoint::Base, Breakpoint::Lg]);
    // Propriété héritée : la base neutre est la valeur effective héritée (défaut `base`).
    let size = doc.nodes[&title].style.font_size.as_ref().unwrap();
    assert_eq!(size.base, FontSize::Base);
    assert_eq!(*size.resolve(Breakpoint::Xl), FontSize::Xl5);

    // `null` efface la surcharge, `null` sur la propriété la supprime.
    run(
        &mut session,
        vec![Command::SetStyle {
            node: r(&hero),
            state: None,
            style: json(json!({ "gap": { "lg": null } })),
        }],
    );
    assert_eq!(session.document().nodes[&hero].style.gap.as_ref().unwrap().lg, None);
    run(
        &mut session,
        vec![Command::SetStyle {
            node: r(&hero),
            state: None,
            style: json(json!({ "gap": null })),
        }],
    );
    assert!(session.document().nodes[&hero].style.gap.is_none());
}

#[test]
fn inherited_neutral_comes_from_ancestors() {
    let mut session = session();
    let applied = landing(&mut session);
    let hero = applied.created["$hero"].clone();
    let title = applied.created["$title"].clone();
    run(
        &mut session,
        vec![Command::SetStyle {
            node: r(&hero),
            state: None,
            style: json(json!({ "font_size": { "base": "lg" } })),
        }],
    );
    run(
        &mut session,
        vec![Command::SetStyle {
            node: r(&title),
            state: None,
            style: json(json!({ "font_size": { "md": "2xl" } })),
        }],
    );
    assert_eq!(
        session.document().nodes[&title].style.font_size.as_ref().unwrap().base,
        FontSize::Lg
    );
}

#[test]
fn state_only_props_require_a_state() {
    let mut session = session();
    let applied = landing(&mut session);
    let cta = applied.created["$cta"].clone();
    let result = session.apply(Transaction::user(
        "scale",
        vec![Command::SetStyle {
            node: r(&cta),
            state: None,
            style: json(json!({ "scale": { "base": "105" } })),
        }],
    ));
    assert!(result.is_err());
    run(
        &mut session,
        vec![Command::SetStyle {
            node: r(&cta),
            state: Some(InteractionState::Hover),
            style: json(
                json!({ "scale": { "base": "105" }, "background": { "base": { "kind": "Color", "color": "primary/90" } } }),
            ),
        }],
    );
    let hover = &session.document().nodes[&cta].states.hover;
    assert_eq!(hover.scale.as_ref().unwrap().base, Scale::S105);
    assert!(hover.background.is_some());
    let result = session.apply(Transaction::user(
        "gap",
        vec![Command::SetStyle {
            node: r(&cta),
            state: Some(InteractionState::Hover),
            style: json(json!({ "gap": { "base": "2" } })),
        }],
    ));
    assert!(result.is_err());
}

#[test]
fn move_duplicate_wrap_unwrap_and_convert() {
    let mut session = session();
    let applied = landing(&mut session);
    let main = applied.created["$main"].clone();
    let hero = applied.created["$hero"].clone();
    let grid = applied.created["$grid"].clone();
    let card = applied.created["$card"].clone();

    // Cycle refusé.
    let error = session
        .apply(Transaction::user(
            "move",
            vec![Command::MoveNode {
                node: r(&main),
                parent: r(&card),
                index: 0,
            }],
        ))
        .unwrap_err();
    assert_eq!(error.code(), "INVALID_OPERATION");
    assert!(error.to_string().contains("cycle"), "{error}");

    let applied = run(&mut session, vec![Command::DuplicateNode { node: r(&card) }]);
    let copy = applied.inserted[0].clone();
    let doc = session.document();
    assert_eq!(doc.nodes[&grid].children, vec![card.clone(), copy.clone()]);
    assert_ne!(doc.nodes[&copy].children, doc.nodes[&card].children);
    assert_eq!(doc.nodes[&copy].children.len(), 1);

    let applied = run_json(
        &mut session,
        json!([{ "op": "wrap_nodes", "nodes": [card.to_string(), copy.to_string()],
                 "container": { "ref": "$row", "kind": "Stack", "style": { "gap": { "base": "4" } } } }]),
    );
    let row = applied.created["$row"].clone();
    let doc = session.document();
    assert_eq!(doc.nodes[&grid].children, vec![row.clone()]);
    assert_eq!(doc.nodes[&row].children, vec![card.clone(), copy.clone()]);
    assert_eq!(
        doc.nodes[&row].style.direction.as_ref().unwrap().base,
        Direction::Column
    );

    run(
        &mut session,
        vec![Command::ConvertContainer {
            node: r(&row),
            to: ContainerKind::Box,
        }],
    );
    let doc = session.document();
    assert!(matches!(doc.nodes[&row].kind, NodeKind::Box(_)));
    assert!(doc.nodes[&row].style.direction.is_none());
    assert!(doc.nodes[&row].style.gap.is_none());

    run(&mut session, vec![Command::UnwrapNode { node: r(&row) }]);
    let doc = session.document();
    assert_eq!(doc.nodes[&grid].children, vec![card.clone(), copy.clone()]);
    assert!(!doc.nodes.contains_key(&row));

    run(
        &mut session,
        vec![Command::MoveNode {
            node: r(&grid),
            parent: r(&main),
            index: 0,
        }],
    );
    assert_eq!(
        session.document().nodes[&main].children,
        vec![grid.clone(), hero.clone()]
    );

    run(
        &mut session,
        vec![Command::DeleteNodes {
            nodes: vec![r(&card), r(&grid)],
        }],
    );
    let doc = session.document();
    assert!(!doc.nodes.contains_key(&grid) && !doc.nodes.contains_key(&card) && !doc.nodes.contains_key(&copy));
}

#[test]
fn wrap_requires_contiguous_siblings() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$a", "kind": { "type": "Box" } },
            { "ref": "$b", "kind": { "type": "Box" } },
            { "ref": "$c", "kind": { "type": "Box" } }
        ]}]),
    );
    let (a, c) = (&applied.created["$a"], &applied.created["$c"]);
    let error = session
        .apply(Transaction::user(
            "wrap",
            json(json!([{ "op": "wrap_nodes", "nodes": [a.to_string(), c.to_string()], "container": { "kind": "Box" } }])),
        ))
        .unwrap_err();
    assert!(error.to_string().contains("contiguous"), "{error}");
}

#[test]
fn set_props_checks_kind_and_set_meta_updates_a11y() {
    let mut session = session();
    let applied = landing(&mut session);
    let title = applied.created["$title"].clone();
    run_json(
        &mut session,
        json!([
            { "op": "set_props", "node": title.to_string(), "props": { "type": "Text", "content": [{ "text": "Salut", "strong": true }] } },
            { "op": "set_meta", "node": title.to_string(), "meta": { "name": "Titre", "a11y_label": "Titre principal" } }
        ]),
    );
    let node = &session.document().nodes[&title];
    let NodeKind::Text(text) = &node.kind else {
        panic!("text")
    };
    assert_eq!(text.plain_text(), "Salut");
    assert!(text.content[0].strong);
    assert_eq!(node.meta.name.as_deref(), Some("Titre"));
    assert_eq!(node.a11y.label.as_deref(), Some("Titre principal"));
    let error = session
        .apply(Transaction::user(
            "props",
            json(json!([{ "op": "set_props", "node": title.to_string(), "props": { "type": "Image", "alt": "x" } }])),
        ))
        .unwrap_err();
    assert_eq!(error.code(), "KIND_MISMATCH");
}

#[test]
fn burger_menu_action_targets_a_node_created_later_in_the_same_command() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$nav", "kind": { "type": "Stack", "role": { "kind": "Nav" } } },
            { "parent": "$nav", "kind": { "type": "Button", "action": { "kind": "ToggleVisibility", "target": "$menu" } },
              "meta": { "a11y_label": "Menu" } },
            { "parent": "$nav", "kind": { "type": "Icon", "name": "menu" } },
            { "ref": "$menu", "parent": "$nav", "kind": { "type": "Stack", "role": { "kind": "List" } },
              "visibility": { "base": false, "md": true } }
        ]}]),
    );
    let doc = session.document();
    let nav = &doc.nodes[&applied.created["$nav"]];
    let NodeKind::Button(button) = &doc.nodes[&nav.children[0]].kind else {
        panic!("button")
    };
    assert_eq!(
        button.action,
        Some(Action::ToggleVisibility {
            target: applied.created["$menu"].clone()
        })
    );
    // Icône décorative par défaut.
    assert!(doc.nodes[&nav.children[1]].a11y.hidden);
    let menu = &doc.nodes[&applied.created["$menu"]];
    assert_eq!(
        menu.visibility.as_ref().map(|v| (v.base, v.md)),
        Some((false, Some(true)))
    );
}

#[test]
fn components_create_update_detach_delete() {
    let mut session = session();
    let applied = landing(&mut session);
    let card = applied.created["$card"].clone();
    let card_title = applied.created["$card-title"].clone();
    run_json(
        &mut session,
        json!([{ "op": "set_style", "node": card.to_string(), "style": { "col_span": { "base": "1" }, "padding": { "base": "6" } } }]),
    );

    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": card.to_string(), "name": "FeatureCard",
                 "props": [{ "name": "title", "node": card_title.to_string(), "field": "Text" }] }]),
    );
    let component_id = applied.components[0].clone();
    let instance = applied.inserted[0].clone();
    let doc = session.document();
    let component = doc.component(&component_id).unwrap();
    assert_eq!(component.root, card);
    assert_eq!(component.props[0].default, PropValue::Text("Rapide".into()));
    assert!(doc.nodes[&card].parent.is_none());
    // Le placement suit l'instance, le style propre reste dans le composant.
    assert!(doc.nodes[&instance].style.col_span.is_some());
    assert!(doc.nodes[&card].style.col_span.is_none());
    assert!(doc.nodes[&card].style.padding_top.is_some());
    assert_eq!(doc.owner_of(&card_title), Some(Owner::Component(component_id.clone())));

    // Variante + surcharge de prop sur l'instance.
    run_json(
        &mut session,
        json!([
            { "op": "update_component", "component": component_id.to_string(),
              "variants": [{ "name": "tone", "default": "plain", "options": [
                  { "name": "plain", "overrides": [] },
                  { "name": "accent", "overrides": [{ "node": card.to_string(),
                      "style": { "background": { "base": { "kind": "Color", "color": "accent" } } } }] }
              ]}] },
            { "op": "set_props", "node": instance.to_string(), "props": { "type": "ComponentInstance",
              "overrides": [{ "prop": "title", "value": { "kind": "Text", "value": "Fiable" } }],
              "variants": [{ "axis": "tone", "option": "accent" }] } }
        ]),
    );
    let error = session
        .apply(Transaction::user(
            "delete",
            vec![Command::DeleteComponent {
                component: component_id.clone(),
            }],
        ))
        .unwrap_err();
    assert_eq!(error.code(), "IN_USE");

    let applied = run(&mut session, vec![Command::DetachInstance { node: r(&instance) }]);
    let detached = applied.inserted[0].clone();
    let doc = session.document();
    assert!(!doc.nodes.contains_key(&instance));
    let copy = &doc.nodes[&detached];
    assert!(copy.style.col_span.is_some(), "instance style moves onto the copy");
    assert!(copy.style.background.is_some(), "variant applied");
    let NodeKind::Text(text) = &doc.nodes[&copy.children[0]].kind else {
        panic!("text")
    };
    assert_eq!(text.plain_text(), "Fiable");

    run(
        &mut session,
        vec![Command::DeleteComponent {
            component: component_id.clone(),
        }],
    );
    let doc = session.document();
    assert!(doc.component(&component_id).is_none());
    assert!(!doc.nodes.contains_key(&card));
}

#[test]
fn removing_a_prop_cleans_instance_overrides() {
    let mut session = session();
    let applied = landing(&mut session);
    let card = applied.created["$card"].clone();
    let card_title = applied.created["$card-title"].clone();
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": card.to_string(), "name": "Card",
                 "props": [{ "name": "title", "node": card_title.to_string(), "field": "Text" }] }]),
    );
    let (component, instance) = (applied.components[0].clone(), applied.inserted[0].clone());
    run_json(
        &mut session,
        json!([{ "op": "set_props", "node": instance.to_string(), "props": { "type": "ComponentInstance",
                 "overrides": [{ "prop": "title", "value": { "kind": "Text", "value": "X" } }] } }]),
    );
    run_json(
        &mut session,
        json!([{ "op": "update_component", "component": component.to_string(), "props": [] }]),
    );
    let NodeKind::ComponentInstance(props) = &session.document().nodes[&instance].kind else {
        panic!()
    };
    assert!(props.overrides.is_empty());
}

#[test]
fn slots_receive_instance_children_on_detach() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$panel", "kind": { "type": "Stack" } },
            { "parent": "$panel", "kind": { "type": "Text", "content": [{ "text": "Header" }] } },
            { "parent": "$panel", "kind": { "type": "Slot" } }
        ]}]),
    );
    let panel = applied.created["$panel"].clone();
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": panel.to_string(), "name": "Panel", "props": [] }]),
    );
    let instance = applied.inserted[0].clone();
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": instance.to_string(), "nodes": [
            { "ref": "$inside", "kind": { "type": "Text", "content": [{ "text": "Contenu" }] } }
        ]}]),
    );
    let inside = applied.created["$inside"].clone();
    assert!(
        session
            .validate()
            .iter()
            .all(|i| i.code != IssueCode::InvalidSlotTarget)
    );
    let applied = run(&mut session, vec![Command::DetachInstance { node: r(&instance) }]);
    let copy = &session.document().nodes[&applied.inserted[0]];
    assert_eq!(copy.children.len(), 2);
    assert_eq!(copy.children[1], inside);
    assert!(
        copy.children
            .iter()
            .all(|c| !matches!(session.document().nodes[c].kind, NodeKind::Slot(_)))
    );
}

#[test]
fn layouts_and_pages() {
    let mut session = session();
    let applied = run_json(&mut session, json!([{ "op": "create_layout", "name": "site" }]));
    let layout = applied.layouts[0].clone();
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_page", "name": "Blog", "route": [{ "kind": "Static", "name": "blog" }, { "kind": "Param", "name": "slug" }],
                 "layout": layout.to_string() }]),
    );
    let page = applied.pages[0].clone();
    let doc = session.document();
    assert_eq!(route_path(&doc.page(&page).unwrap().route), "/blog/[slug]");
    assert_eq!(doc.page(&page).unwrap().seo.title, "Blog");
    let layout_root = &doc.nodes[&doc.layout(&layout).unwrap().root];
    assert!(matches!(&doc.nodes[&layout_root.children[0]].kind, NodeKind::Slot(s) if s.name == "page"));

    let error = session
        .apply(Transaction::user(
            "x",
            vec![Command::DeleteLayout { layout: layout.clone() }],
        ))
        .unwrap_err();
    assert_eq!(error.code(), "IN_USE");
    run_json(
        &mut session,
        json!([{ "op": "update_page", "page": page.to_string(), "layout": null, "name": "Articles" }]),
    );
    assert_eq!(session.document().page(&page).unwrap().layout, None);
    run(
        &mut session,
        vec![
            Command::DeleteLayout { layout: layout.clone() },
            Command::DeletePage { page: page.clone() },
        ],
    );
    let doc = session.document();
    assert!(doc.layouts.is_empty());
    assert_eq!(doc.pages.len(), 1);
    assert!(ir::validate(doc).iter().all(|i| i.code != IssueCode::OrphanNode));
}

#[test]
fn duplicate_routes_are_reported() {
    let mut session = session();
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_page", "name": "Bis", "route": [] }]),
    );
    assert!(applied.issues.iter().any(|i| i.code == IssueCode::DuplicateRoute));
}

#[test]
fn tokens_in_use_cannot_be_deleted() {
    let mut session = session();
    let applied = landing(&mut session);
    let title = applied.created["$title"].clone();
    run_json(
        &mut session,
        json!([
            { "op": "set_token", "edit": { "kind": "Color", "name": "brand", "value": { "light": "#1D4ED8" } } },
            { "op": "set_style", "node": title.to_string(), "style": { "text_color": { "base": "brand" } } }
        ]),
    );
    assert_eq!(
        session
            .document()
            .tokens
            .color(&"brand".parse().unwrap())
            .unwrap()
            .light
            .as_str(),
        "#1d4ed8"
    );
    let error = session
        .apply(Transaction::user(
            "x",
            json(json!([{ "op": "set_token", "edit": { "kind": "Color", "name": "brand", "value": null } }])),
        ))
        .unwrap_err();
    assert_eq!(error.code(), "IN_USE");
    run_json(
        &mut session,
        json!([{ "op": "set_token", "edit": { "kind": "SpacingUnit", "px": 5 } }]),
    );
    assert_eq!(session.document().tokens.spacing_unit, 5);
}

#[test]
fn assets_are_protected_while_referenced() {
    let mut session = session();
    let root = home_root(&session);
    let asset: Asset = json(
        json!({ "id": "a_0123456789", "file_name": "hero.jpg", "mime": "image/jpeg",
                                    "storage_path": "p/hero.jpg", "width": 1600, "height": 900, "bytes": 1234 }),
    );
    run(&mut session, vec![Command::AddAsset { asset: asset.clone() }]);
    run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "kind": { "type": "Image", "source": { "kind": "Asset", "id": "a_0123456789" }, "alt": "Hero",
                        "intrinsic": { "width": 1600, "height": 900 } } }
        ]}]),
    );
    let error = session
        .apply(Transaction::user(
            "x",
            vec![Command::RemoveAsset {
                asset: asset.id.clone(),
            }],
        ))
        .unwrap_err();
    assert_eq!(error.code(), "IN_USE");
}

#[test]
fn ai_origin_cannot_use_ui_only_commands_or_raw_code() {
    let mut session = session();
    let root = home_root(&session);
    let error = try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([{ "op": "set_platform", "node": root.to_string(), "scope": "WebOnly" }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "FORBIDDEN");
    let error = try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [{ "kind": { "type": "RawCode", "code": "<div/>" } }] }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "FORBIDDEN");
    let applied = try_run(
        &mut session,
        ai(),
        Scope::full(),
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [{ "ref": "$x", "kind": { "type": "Box" } }] }]),
    )
    .unwrap();
    assert_eq!(
        session.document().nodes[&applied.created["$x"]].meta.source,
        NodeSource::Ai
    );
}

#[test]
fn web_overrides_only_accept_data_and_aria_attributes() {
    let mut session = session();
    let root = home_root(&session);
    let ok = json!([{ "op": "set_web_overrides", "node": root.to_string(),
        "overrides": { "extra_classes": ["backdrop-blur"], "extra_attributes": [{ "name": "data-test", "value": "x" }] } }]);
    run_json(&mut session, ok);
    for bad in ["onclick", "style", "className", "data-"] {
        let result = try_run(
            &mut session,
            Origin::User,
            Scope::full(),
            json!([{ "op": "set_web_overrides", "node": root.to_string(),
                     "overrides": { "extra_attributes": [{ "name": bad, "value": "x" }] } }]),
        );
        assert!(result.is_err(), "{bad} must be refused");
    }
}

#[test]
fn scope_content_only_allows_texts_images_and_links() {
    let mut session = session();
    let applied = landing(&mut session);
    let title = applied.created["$title"].to_string();
    let content = Scope {
        content_only: true,
        ..Scope::full()
    };
    try_run(
        &mut session,
        Origin::User,
        content.clone(),
        json!([{ "op": "set_props", "node": title, "props": { "type": "Text", "content": [{ "text": "Nouveau" }] } }]),
    )
    .unwrap();
    let error = try_run(
        &mut session,
        Origin::User,
        content.clone(),
        json!([{ "op": "set_props", "node": title, "props": { "type": "Text", "role": { "kind": "Paragraph" } } }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
    let error = try_run(
        &mut session,
        Origin::User,
        content,
        json!([{ "op": "set_style", "node": title, "style": { "font_size": { "base": "xl" } } }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
}

#[test]
fn scope_breakpoint_only_touches_that_breakpoint() {
    let mut session = session();
    let applied = landing(&mut session);
    let hero = applied.created["$hero"].to_string();
    let md = Scope {
        breakpoint: Some(Breakpoint::Md),
        ..Scope::full()
    };
    try_run(
        &mut session,
        ai(),
        md.clone(),
        json!([{ "op": "set_style", "node": hero, "style": { "direction": { "md": "row" } } }]),
    )
    .unwrap();
    let node = &session.document().nodes[&applied.created["$hero"]];
    assert_eq!(
        node.style.direction.as_ref().unwrap().base,
        Direction::Column,
        "base untouched"
    );
    for style in [json!({ "direction": { "base": "row" } }), json!({ "direction": null })] {
        let error = try_run(
            &mut session,
            ai(),
            md.clone(),
            json!([{ "op": "set_style", "node": hero, "style": style }]),
        )
        .unwrap_err();
        assert_eq!(error.code(), "OUT_OF_SCOPE");
    }
    let error = try_run(
        &mut session,
        ai(),
        md,
        json!([{ "op": "delete_nodes", "nodes": [hero] }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
}

#[test]
fn scope_subtree_limits_targets_to_the_selection() {
    let mut session = session();
    let applied = landing(&mut session);
    let hero = applied.created["$hero"].clone();
    let selection = Scope {
        subtree: Some(vec![hero.clone()]),
        ..Scope::full()
    };
    try_run(
        &mut session,
        ai(),
        selection.clone(),
        json!([{ "op": "insert_nodes", "parent": hero.to_string(), "nodes": [{ "kind": { "type": "Text", "content": [{ "text": "Sous-titre" }] } }] }]),
    )
    .unwrap();
    let grid = applied.created["$grid"].to_string();
    let error = try_run(
        &mut session,
        ai(),
        selection.clone(),
        json!([{ "op": "set_style", "node": grid, "style": { "gap": { "base": "2" } } }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
    let error = try_run(
        &mut session,
        ai(),
        selection,
        json!([{ "op": "set_token", "edit": { "kind": "SpacingUnit", "px": 4 } }]),
    )
    .unwrap_err();
    assert_eq!(error.code(), "OUT_OF_SCOPE");
}

#[test]
fn errors_serialize_for_the_llm() {
    let error = CommandError::NodeNotFound("n_0000000000".into());
    let value = error.to_tool_result();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "NODE_NOT_FOUND");
}
