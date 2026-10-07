//! Régressions de la revue (a), lot « validation (schéma, ordre de rendu) », ré-audité après la
//! fusion de la PR #1 (ADR 0001 § 14, point 21) : ordre de rendu des pages (slots imbriqués,
//! composant dans son propre slot, longues pages, slot `page`, prop `Visible`), règles de schéma
//! évaluées au rendu comme les commandes les évaluent (placement, listes, interactifs, ancres),
//! règles que les commandes imposent mais que la validation oubliait (variantes, noms, choix de
//! variantes), routes, images, racines, contraste et accessibilité dans leur contexte de rendu.

mod common;

use common::*;
use ir::*;
use serde_json::json;

fn insert(session: &mut Session, nodes: serde_json::Value) -> Applied {
    let root = home_root(session);
    insert_into(session, &root, nodes)
}

fn insert_into(session: &mut Session, parent: &NodeId, nodes: serde_json::Value) -> Applied {
    run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": parent.to_string(), "nodes": nodes }]),
    )
}

fn has(issues: &[Issue], code: IssueCode, node: &NodeId) -> bool {
    issues.iter().any(|i| i.code == code && i.node.as_ref() == Some(node))
}

fn count(issues: &[Issue], code: IssueCode) -> usize {
    issues.iter().filter(|i| i.code == code).count()
}

/// Composant créé à partir d'un sous-arbre inséré sur l'accueil (l'instance créée est ensuite
/// supprimée) ; renvoie son id et les ids des nœuds nommés.
fn component(
    session: &mut Session,
    name: &str,
    nodes: serde_json::Value,
    props: serde_json::Value,
) -> (ComponentId, std::collections::BTreeMap<String, NodeId>) {
    let applied = insert(session, nodes);
    let props: Vec<serde_json::Value> = props
        .as_array()
        .map(|list| {
            list.iter()
                .map(|p| {
                    let mut spec = json!({ "name": p["name"], "node": applied.created[p["node"].as_str().unwrap()].to_string(), "field": p["field"] });
                    if !p["default"].is_null() {
                        spec["default"] = p["default"].clone();
                    }
                    spec
                })
                .collect()
        })
        .unwrap_or_default();
    let created = applied.created.clone();
    let extracted = run_json(
        session,
        json!([{ "op": "create_component", "node": created["$root"].to_string(), "name": name, "props": props }]),
    );
    run(
        session,
        vec![Command::DeleteNodes {
            nodes: vec![r(&extracted.inserted[0])],
        }],
    );
    (extracted.components[0].clone(), created)
}

fn instance_of(component: &ComponentId) -> serde_json::Value {
    json!({ "type": "ComponentInstance", "component": component.to_string() })
}

fn heading(level: &str, text: &str) -> serde_json::Value {
    json!({ "type": "Text", "role": { "kind": "Heading", "level": level }, "content": [{ "text": text }] })
}

fn main_landmark() -> serde_json::Value {
    json!({ "kind": { "type": "Box", "role": { "kind": "Main" } } })
}

/// Composant à un seul slot `children` sous une racine `Stack`.
fn shell(session: &mut Session, name: &str) -> ComponentId {
    component(
        session,
        name,
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "parent": "$root", "kind": { "type": "Slot" } }
        ]),
        json!([]),
    )
    .0
}

// ---------------------------------------------------------------- ordre de rendu

// Le contenu d'un slot transmis d'un composant à un autre est rendu.
#[test]
fn nested_slot_content_is_rendered() {
    let mut session = session();
    let inner = shell(&mut session, "Inner");
    let (outer, _) = component(
        &mut session,
        "Outer",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$inner", "parent": "$root", "kind": instance_of(&inner) },
            { "parent": "$inner", "kind": { "type": "Slot" } }
        ]),
        json!([]),
    );
    insert(
        &mut session,
        json!([
            { "ref": "$outer", "kind": instance_of(&outer) },
            { "parent": "$outer", "kind": heading("h1", "Titre") },
            main_landmark()
        ]),
    );
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMissingH1), 0, "{issues:#?}");
}

// Une instance placée dans le contenu du slot d'une instance du même composant est rendue.
#[test]
fn a_component_inside_its_own_slot_content_is_rendered() {
    let mut session = session();
    let card = shell(&mut session, "Card");
    insert(
        &mut session,
        json!([
            { "ref": "$outer", "kind": instance_of(&card) },
            { "ref": "$inner", "parent": "$outer", "kind": instance_of(&card) },
            { "parent": "$inner", "kind": heading("h1", "Titre") },
            main_landmark()
        ]),
    );
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMissingH1), 0, "{issues:#?}");
}

// Une page qui réutilise beaucoup un composant est rendue jusqu'au bout.
#[test]
fn long_pages_are_rendered_entirely() {
    let mut session = session();
    let mut nodes = vec![json!({ "ref": "$root", "kind": { "type": "Stack" } })];
    for i in 0..7 {
        nodes.push(
            json!({ "parent": "$root", "kind": { "type": "Text", "content": [{ "text": format!("Ligne {i}") }] } }),
        );
    }
    let (item, _) = component(&mut session, "Item", json!(nodes), json!([]));
    let mut page: Vec<serde_json::Value> = (0..40).map(|_| json!({ "kind": instance_of(&item) })).collect();
    page.push(json!({ "kind": heading("h1", "Fin") }));
    page.push(main_landmark());
    insert(&mut session, json!(page));
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMissingH1), 0, "{issues:#?}");
}

// Le slot `page` d'un composant n'est pas celui du layout : la page n'est rendue qu'une fois, et
// ce nom est réservé aux layouts.
#[test]
fn a_component_slot_named_page_is_not_the_layout_page_slot() {
    let mut session = session();
    let (frame, created) = component(
        &mut session,
        "Frame",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$slot", "parent": "$root", "kind": { "type": "Slot", "name": "page" } }
        ]),
        json!([]),
    );
    let layout = run_json(&mut session, json!([{ "op": "create_layout", "name": "site" }])).layouts[0].clone();
    let layout_root = session.document().layout(&layout).unwrap().root.clone();
    let page = session.document().pages[0].id.clone();
    run_json(
        &mut session,
        json!([
            { "op": "insert_nodes", "parent": layout_root.to_string(), "index": 0, "nodes": [
                { "ref": "$frame", "kind": instance_of(&frame) },
                { "parent": "$frame", "kind": { "type": "Text", "content": [{ "text": "Bandeau" }] }, "meta": { "slot": "page" } }
            ] },
            { "op": "update_page", "page": page.to_string(), "layout": layout.to_string() }
        ]),
    );
    insert(
        &mut session,
        json!([{ "kind": heading("h1", "Accueil") }, main_landmark()]),
    );
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMultipleH1), 0, "{issues:#?}");
    assert_eq!(count(&issues, IssueCode::A11yMultipleMain), 0, "{issues:#?}");
    assert!(has(&issues, IssueCode::InvalidName, &created["$slot"]), "{issues:#?}");
}

// Un nœud masqué par une prop `Visible` à `false` n'est pas rendu.
#[test]
fn a_node_hidden_by_a_visible_prop_is_not_rendered() {
    let mut session = session();
    let (hero, _) = component(
        &mut session,
        "Hero",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$title", "parent": "$root", "kind": heading("h1", "Héros") }
        ]),
        json!([{ "name": "showTitle", "node": "$title", "field": "Visible" }]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$hero", "kind": instance_of(&hero) },
            { "kind": heading("h1", "Accueil") },
            main_landmark()
        ]),
    );
    run_json(
        &mut session,
        json!([{ "op": "set_props", "node": applied.created["$hero"].to_string(), "props": { "type": "ComponentInstance",
                 "overrides": [{ "prop": "showTitle", "value": { "kind": "Bool", "value": false } }] } }]),
    );
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMultipleH1), 0, "{issues:#?}");
}

// ---------------------------------------------------------------- schéma évalué au rendu

// Les propriétés de placement suivent l'hôte au rendu, comme dans les commandes.
#[test]
fn placement_follows_the_rendered_host() {
    let mut session = session();
    let (row, _) = component(
        &mut session,
        "Row",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" }, "style": { "direction": { "base": "row" } } },
            { "parent": "$root", "kind": { "type": "Slot" } }
        ]),
        json!([]),
    );
    let (card, _) = component(
        &mut session,
        "Card",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "parent": "$root", "kind": { "type": "Text", "content": [{ "text": "Carte" }] } }
        ]),
        json!([]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$row", "kind": instance_of(&row) },
            { "ref": "$item", "parent": "$row", "kind": { "type": "Box" }, "style": { "grow": { "base": true } } },
            { "ref": "$grid", "kind": { "type": "Grid" }, "style": { "columns": { "base": "2" } } },
            { "parent": "$grid", "kind": instance_of(&card) }
        ]),
    );
    let card_root = session.document().component(&card).unwrap().root.clone();
    run_json(
        &mut session,
        json!([{ "op": "set_style", "node": card_root.to_string(), "style": { "col_span": { "base": "2" } } }]),
    );
    let issues = session.validate();
    assert!(
        !has(&issues, IssueCode::StyleNotAllowed, &applied.created["$item"]),
        "{issues:#?}"
    );
    assert!(!has(&issues, IssueCode::StyleNotAllowed, &card_root), "{issues:#?}");
}

// Un élément de liste est jugé à sa place au rendu ; une liste ne contient que des éléments.
#[test]
fn lists_are_checked_at_render() {
    let mut session = session();
    let (feature, _) = component(
        &mut session,
        "Feature",
        json!([
            { "ref": "$root", "kind": { "type": "Box", "role": { "kind": "ListItem" } } },
            { "parent": "$root", "kind": { "type": "Text", "content": [{ "text": "Rapide" }] } }
        ]),
        json!([]),
    );
    let (bullets, _) = component(
        &mut session,
        "Bullets",
        json!([
            { "ref": "$root", "kind": { "type": "Stack", "role": { "kind": "List" } } },
            { "parent": "$root", "kind": { "type": "Slot" } }
        ]),
        json!([]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$list", "kind": { "type": "Grid", "role": { "kind": "List" } } },
            { "parent": "$list", "kind": instance_of(&feature) },
            { "parent": "$list", "kind": instance_of(&feature) },
            { "ref": "$bullets", "kind": instance_of(&bullets) },
            { "ref": "$point", "parent": "$bullets", "kind": { "type": "Box", "role": { "kind": "ListItem" } } },
            { "ref": "$stray", "kind": instance_of(&feature) },
            { "ref": "$plain", "kind": { "type": "Stack", "role": { "kind": "List" } } },
            { "ref": "$text", "parent": "$plain", "kind": { "type": "Text", "content": [{ "text": "Pas un élément" }] } }
        ]),
    );
    let issues = session.validate();
    let feature_root = session.document().component(&feature).unwrap().root.clone();
    assert!(
        !has(&issues, IssueCode::ListItemOutsideList, &feature_root),
        "{issues:#?}"
    );
    assert!(
        !has(&issues, IssueCode::ListItemOutsideList, &applied.created["$point"]),
        "{issues:#?}"
    );
    assert!(
        has(&issues, IssueCode::ListItemOutsideList, &applied.created["$stray"]),
        "an instance that renders a list item outside a list: {issues:#?}"
    );
    assert!(
        has(&issues, IssueCode::ListChildNotItem, &applied.created["$text"]),
        "a list renders only list items: {issues:#?}"
    );
}

// Un élément interactif rendu dans un autre est signalé, à travers les composants et les slots.
#[test]
fn nested_interactive_elements_are_found_through_components() {
    let mut session = session();
    let (cta, _) = component(
        &mut session,
        "Cta",
        json!([{ "ref": "$root", "kind": { "type": "Button", "label": "Go" } }]),
        json!([]),
    );
    let (card_link, _) = component(
        &mut session,
        "CardLink",
        json!([
            { "ref": "$root", "kind": { "type": "Link", "href": { "kind": "External", "url": "https://example.com" } } },
            { "parent": "$root", "kind": { "type": "Slot" } }
        ]),
        json!([]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$link", "kind": { "type": "Link", "href": { "kind": "External", "url": "https://example.com" } } },
            { "ref": "$cta", "parent": "$link", "kind": instance_of(&cta) },
            { "ref": "$card", "kind": instance_of(&card_link) },
            { "ref": "$buy", "parent": "$card", "kind": { "type": "Button", "label": "Acheter" } }
        ]),
    );
    let issues = session.validate();
    assert!(
        has(&issues, IssueCode::NestedInteractive, &applied.created["$cta"]),
        "{issues:#?}"
    );
    assert!(
        has(&issues, IssueCode::NestedInteractive, &applied.created["$buy"]),
        "{issues:#?}"
    );
}

// Les ancres rendues par un composant comptent pour la page : liens valides, doublons signalés.
#[test]
fn anchors_rendered_by_components_belong_to_the_page() {
    let mut session = session();
    let (pricing, created) = component(
        &mut session,
        "Pricing",
        json!([
            { "ref": "$root", "kind": { "type": "Box", "role": { "kind": "Section" } }, "meta": { "anchor": "pricing" } },
            { "parent": "$root", "kind": { "type": "Text", "content": [{ "text": "Tarifs" }] } }
        ]),
        json!([]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "kind": instance_of(&pricing) },
            { "ref": "$nav", "kind": { "type": "Link", "label": "Tarifs", "href": { "kind": "Anchor", "anchor": "pricing" } } }
        ]),
    );
    let issues = session.validate();
    assert!(
        !has(&issues, IssueCode::InvalidReference, &applied.created["$nav"]),
        "{issues:#?}"
    );
    assert_eq!(count(&issues, IssueCode::DuplicateAnchor), 0, "{issues:#?}");

    insert(&mut session, json!([{ "kind": instance_of(&pricing) }]));
    let issues = session.validate();
    assert!(
        has(&issues, IssueCode::DuplicateAnchor, &created["$root"]),
        "{issues:#?}"
    );
}

// Un lien vers une ancre écrit dans un composant est vérifié sur chaque page qui le rend.
#[test]
fn anchor_links_in_components_are_checked_where_rendered() {
    let mut session = session();
    let (nav, created) = component(
        &mut session,
        "Nav",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$link", "parent": "$root", "kind": { "type": "Link", "label": "Contact", "href": { "kind": "Anchor", "anchor": "contact" } } }
        ]),
        json!([]),
    );
    insert(
        &mut session,
        json!([
            { "kind": instance_of(&nav) },
            { "kind": { "type": "Box", "role": { "kind": "Section" } }, "meta": { "anchor": "contact" } }
        ]),
    );
    let issues = session.validate();
    assert!(
        !has(&issues, IssueCode::InvalidReference, &created["$link"]),
        "{issues:#?}"
    );

    let blog = run_json(
        &mut session,
        json!([{ "op": "create_page", "name": "Blog", "route": [{ "kind": "Static", "name": "blog" }] }]),
    )
    .pages[0]
        .clone();
    let blog_root = session.document().page(&blog).unwrap().root.clone();
    insert_into(&mut session, &blog_root, json!([{ "kind": instance_of(&nav) }]));
    let issues = session.validate();
    assert!(
        has(&issues, IssueCode::InvalidReference, &created["$link"]),
        "{issues:#?}"
    );
}

// ---------------------------------------------------------------- règles des commandes

// Les surcharges de variantes obéissent aux règles de style de leur nœud cible.
#[test]
fn variant_overrides_are_validated() {
    let mut session = session();
    let (card, created) = component(
        &mut session,
        "Card",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$text", "parent": "$root", "kind": { "type": "Text", "content": [{ "text": "Carte" }] } }
        ]),
        json!([]),
    );
    let text = created["$text"].clone();
    let mut doc = session.document().clone();
    let axis: VariantAxis = json(json!({
        "name": "tone",
        "default": "dark",
        "options": [{ "name": "dark", "overrides": [{
            "node": text.to_string(),
            "style": { "columns": { "base": "2" }, "text_color": { "base": "ghost" }, "width": { "base": "none" }, "font_family": { "base": "display" } }
        }] }]
    }));
    doc.components.iter_mut().find(|c| c.id == card).unwrap().variants = vec![axis];
    let issues = validate(&doc);
    let on_text: Vec<&Issue> = issues.iter().filter(|i| i.node.as_ref() == Some(&text)).collect();
    assert!(
        on_text
            .iter()
            .any(|i| i.code == IssueCode::StyleNotAllowed && i.message.contains("columns")),
        "{issues:#?}"
    );
    assert!(
        on_text
            .iter()
            .any(|i| i.code == IssueCode::StyleNotAllowed && i.message.contains("none")),
        "{issues:#?}"
    );
    assert!(
        on_text
            .iter()
            .any(|i| i.code == IssueCode::InvalidReference && i.message.contains("ghost")),
        "{issues:#?}"
    );
    assert!(
        on_text
            .iter()
            .any(|i| i.code == IssueCode::InvalidReference && i.message.contains("display")),
        "{issues:#?}"
    );
}

// Les noms qui deviennent des props TypeScript sont valides et ne se chevauchent pas.
#[test]
fn component_names_are_validated() {
    let mut session = session();
    let (card, created) = component(
        &mut session,
        "Card",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$title", "parent": "$root", "kind": { "type": "Text", "content": [{ "text": "Titre" }] } },
            { "ref": "$slot", "parent": "$root", "kind": { "type": "Slot" } },
            { "ref": "$aside", "parent": "$root", "kind": { "type": "Slot", "name": "aside" } }
        ]),
        json!([{ "name": "title", "node": "$title", "field": "Text" }]),
    );
    let root = created["$root"].clone();
    let base = session.document().clone();

    let with = |edit: &dyn Fn(&mut Document)| {
        let mut doc = base.clone();
        edit(&mut doc);
        validate(&doc)
    };
    let with_component = |edit: &dyn Fn(&mut Component)| {
        with(&|doc: &mut Document| edit(doc.components.iter_mut().find(|c| c.id == card).unwrap()))
    };
    let axis = |name: &str, options: &[&str]| -> VariantAxis {
        json(json!({ "name": name, "default": options.first().copied().unwrap_or(""),
                     "options": options.iter().map(|o| json!({ "name": o })).collect::<Vec<_>>() }))
    };

    // Nom de prop invalide.
    let issues = with_component(&|c| c.props[0].name = "Bad Name".into());
    assert!(has(&issues, IssueCode::InvalidProp, &root), "{issues:#?}");
    // Prop qui porte le nom du slot par défaut.
    let issues = with_component(&|c| c.props[0].name = "children".into());
    assert!(has(&issues, IssueCode::DuplicateName, &root), "{issues:#?}");
    // Axe de variantes invalide, en double, ou homonyme d'une prop ; options vides ou en double.
    let issues = with_component(&|c| c.variants = vec![axis("Size Axis", &["sm"])]);
    assert!(has(&issues, IssueCode::InvalidProp, &root), "{issues:#?}");
    let issues = with_component(&|c| c.variants = vec![axis("size", &["sm"]), axis("size", &["lg"])]);
    assert!(has(&issues, IssueCode::InvalidProp, &root), "{issues:#?}");
    let issues = with_component(&|c| c.variants = vec![axis("title", &["sm"])]);
    assert!(has(&issues, IssueCode::DuplicateName, &root), "{issues:#?}");
    let issues = with_component(&|c| c.variants = vec![axis("size", &["sm", "sm"])]);
    assert!(has(&issues, IssueCode::InvalidProp, &root), "{issues:#?}");
    let issues = with_component(&|c| c.variants = vec![axis("size", &[""])]);
    assert!(has(&issues, IssueCode::InvalidProp, &root), "{issues:#?}");
    // Slot au nom invalide, ou homonyme d'une prop.
    let issues = with(&|doc| {
        if let NodeKind::Slot(slot) = &mut doc.nodes.get_mut(&created["$aside"]).unwrap().kind {
            slot.name = "my slot".into();
        }
    });
    assert!(has(&issues, IssueCode::InvalidName, &created["$aside"]), "{issues:#?}");
    let issues = with(&|doc| {
        if let NodeKind::Slot(slot) = &mut doc.nodes.get_mut(&created["$aside"]).unwrap().kind {
            slot.name = "title".into();
        }
    });
    assert!(has(&issues, IssueCode::DuplicateName, &root), "{issues:#?}");
    // Le document de départ est valide sur ces points.
    let issues = validate(&base);
    assert!(
        !issues.iter().any(|i| matches!(
            i.code,
            IssueCode::InvalidProp | IssueCode::InvalidName | IssueCode::DuplicateName
        )),
        "{issues:#?}"
    );
}

// Un layout n'a que son slot `page`.
#[test]
fn layouts_only_have_their_page_slot() {
    let mut session = session();
    let layout = run_json(&mut session, json!([{ "op": "create_layout", "name": "site" }])).layouts[0].clone();
    let layout_root = session.document().layout(&layout).unwrap().root.clone();
    insert_into(
        &mut session,
        &layout_root,
        json!([{ "kind": { "type": "Slot", "name": "aside" } }]),
    );
    let issues = session.validate();
    assert!(has(&issues, IssueCode::LayoutPageSlot, &layout_root), "{issues:#?}");
}

// Deux choix pour le même axe de variantes sur une instance sont refusés.
#[test]
fn an_instance_chooses_one_option_per_axis() {
    let mut session = session();
    let (card, _) = component(
        &mut session,
        "Card",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "parent": "$root", "kind": { "type": "Text", "content": [{ "text": "Carte" }] } }
        ]),
        json!([]),
    );
    run_json(
        &mut session,
        json!([{ "op": "update_component", "component": card.to_string(), "variants": [
            { "name": "tone", "default": "light", "options": [{ "name": "light" }, { "name": "dark" }] }
        ] }]),
    );
    let applied = insert(&mut session, json!([{ "ref": "$card", "kind": instance_of(&card) }]));
    let instance = applied.created["$card"].clone();
    run_json(
        &mut session,
        json!([{ "op": "set_props", "node": instance.to_string(), "props": { "type": "ComponentInstance",
                 "variants": [{ "axis": "tone", "option": "light" }, { "axis": "tone", "option": "dark" }] } }]),
    );
    assert!(has(&session.validate(), IssueCode::InvalidProp, &instance));
}

// ---------------------------------------------------------------- routes, images, arbre

// Les paramètres d'une route sont distincts, et deux routes nomment pareil un paramètre au même
// rang sous le même préfixe (contrainte de l'App Router).
#[test]
fn route_parameters_are_consistent() {
    let mut session = session();
    let pages = run_json(
        &mut session,
        json!([
            { "op": "create_page", "name": "Article", "route": [{ "kind": "Static", "name": "blog" }, { "kind": "Param", "name": "slug" }] },
            { "op": "create_page", "name": "Édition", "route": [{ "kind": "Static", "name": "blog" }, { "kind": "Param", "name": "id" }, { "kind": "Static", "name": "edit" }] },
            { "op": "create_page", "name": "Double", "route": [{ "kind": "Static", "name": "docs" }, { "kind": "Param", "name": "part" }, { "kind": "Param", "name": "part" }] }
        ]),
    )
    .pages;
    let doc = session.document();
    let root = |i: usize| doc.page(&pages[i]).unwrap().root.clone();
    let issues = session.validate();
    assert!(
        has(&issues, IssueCode::DuplicateRoute, &root(0)) || has(&issues, IssueCode::DuplicateRoute, &root(1)),
        "{issues:#?}"
    );
    assert!(has(&issues, IssueCode::InvalidRoute, &root(2)), "{issues:#?}");
}

// Les sources d'image sont des URL http(s) absolues, des assets image, avec des dimensions
// positives ; favicon et image OG sont des images.
#[test]
fn image_sources_are_checked() {
    let mut session = session();
    run_json(
        &mut session,
        json!([{ "op": "add_asset", "asset": { "id": "a_0000000pdf", "file_name": "doc.pdf", "mime": "application/pdf",
                 "storage_path": "assets/doc.pdf", "bytes": 1024 } }]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$ftp", "kind": { "type": "Image", "alt": "A", "source": { "kind": "Url", "url": "ftp://example.com/a.png" },
              "intrinsic": { "width": 10, "height": 10 } } },
            { "ref": "$relative", "kind": { "type": "Image", "alt": "B", "source": { "kind": "Url", "url": "/a.png" },
              "intrinsic": { "width": 10, "height": 10 } } },
            { "ref": "$zero", "kind": { "type": "Image", "alt": "C", "source": { "kind": "Url", "url": "https://example.com/c.png" },
              "intrinsic": { "width": 0, "height": 0 } } },
            { "ref": "$pdf", "kind": { "type": "Image", "alt": "D", "source": { "kind": "Asset", "id": "a_0000000pdf" },
              "intrinsic": { "width": 10, "height": 10 } } },
            { "ref": "$ok", "kind": { "type": "Image", "alt": "E", "source": { "kind": "Url", "url": "https://example.com/e.png" },
              "intrinsic": { "width": 10, "height": 10 } } }
        ]),
    );
    run_json(
        &mut session,
        json!([{ "op": "update_settings", "favicon": "a_0000000pdf" }]),
    );
    let issues = session.validate();
    let created = |r: &str| applied.created[r].clone();
    assert!(has(&issues, IssueCode::InvalidProp, &created("$ftp")), "{issues:#?}");
    assert!(
        has(&issues, IssueCode::InvalidProp, &created("$relative")),
        "{issues:#?}"
    );
    assert!(
        has(&issues, IssueCode::MissingIntrinsic, &created("$zero")),
        "{issues:#?}"
    );
    assert!(
        has(&issues, IssueCode::InvalidReference, &created("$pdf")),
        "{issues:#?}"
    );
    assert!(
        issues
            .iter()
            .any(|i| i.code == IssueCode::InvalidReference && i.node.is_none() && i.message.contains("favicon")),
        "{issues:#?}"
    );
    let ok = created("$ok");
    assert!(
        !issues.iter().any(|i| i.node.as_ref() == Some(&ok) && i.is_error()),
        "{issues:#?}"
    );
}

// Un nœud d'un cycle de parents n'est pas en plus signalé comme orphelin.
#[test]
fn a_parent_cycle_is_not_reported_as_orphans() {
    let session = session();
    let mut doc = session.document().clone();
    let a: NodeId = "n_cyclenodea".parse().unwrap();
    let b: NodeId = "n_cyclenodeb".parse().unwrap();
    let mut node_a = Node::new(
        a.clone(),
        Some(b.clone()),
        NodeKind::Box(ContainerProps {
            role: ContainerRole::Generic,
        }),
    );
    node_a.children = vec![b.clone()];
    let mut node_b = Node::new(
        b.clone(),
        Some(a.clone()),
        NodeKind::Box(ContainerProps {
            role: ContainerRole::Generic,
        }),
    );
    node_b.children = vec![a.clone()];
    doc.nodes.insert(a.clone(), node_a);
    doc.nodes.insert(b.clone(), node_b);
    let issues = validate(&doc);
    for id in [&a, &b] {
        assert!(has(&issues, IssueCode::Cycle, id), "{issues:#?}");
        assert!(!has(&issues, IssueCode::OrphanNode, id), "{issues:#?}");
    }
}

// Les racines de page et de layout sont des conteneurs.
#[test]
fn page_and_layout_roots_are_containers() {
    let mut session = session();
    run_json(&mut session, json!([{ "op": "create_layout", "name": "site" }]));
    let mut doc = session.document().clone();
    let page_root = doc.pages[0].root.clone();
    let layout_root = doc.layouts[0].root.clone();
    let text = NodeKind::Text(TextProps {
        role: TextRole::Paragraph,
        content: vec![TextRun::plain("Racine")],
    });
    doc.nodes.get_mut(&page_root).unwrap().kind = text.clone();
    doc.nodes.get_mut(&layout_root).unwrap().kind = text;
    let issues = validate(&doc);
    assert!(has(&issues, IssueCode::RootNotContainer, &page_root), "{issues:#?}");
    assert!(has(&issues, IssueCode::RootNotContainer, &layout_root), "{issues:#?}");
}

// ---------------------------------------------------------------- contraste et a11y au rendu

// Le texte d'un slot est jugé sur le fond du composant qui l'accueille.
#[test]
fn slot_text_contrast_uses_the_component_background() {
    let mut session = session();
    let (dark, _) = component(
        &mut session,
        "DarkCard",
        json!([
            { "ref": "$root", "kind": { "type": "Box" }, "style": { "background": { "base": { "kind": "Color", "color": "foreground" } } } },
            { "parent": "$root", "kind": { "type": "Slot" } }
        ]),
        json!([]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$card", "kind": instance_of(&dark) },
            { "ref": "$text", "parent": "$card", "kind": { "type": "Text", "content": [{ "text": "Clair sur sombre" }] },
              "style": { "text_color": { "base": "background" } } }
        ]),
    );
    let issues = session.validate();
    assert!(
        !has(&issues, IssueCode::A11yContrast, &applied.created["$text"]),
        "{issues:#?}"
    );
}

// Le texte d'un composant est jugé sur le fond de chaque page qui le rend.
#[test]
fn component_text_contrast_uses_the_host_background() {
    let mut session = session();
    let (badge, created) = component(
        &mut session,
        "Badge",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$label", "parent": "$root", "kind": { "type": "Text", "content": [{ "text": "Nouveau" }] },
              "style": { "text_color": { "base": "primary-foreground" } } }
        ]),
        json!([]),
    );
    insert(
        &mut session,
        json!([
            { "ref": "$band", "kind": { "type": "Box" }, "style": { "background": { "base": { "kind": "Color", "color": "primary" } } } },
            { "parent": "$band", "kind": instance_of(&badge) }
        ]),
    );
    let issues = session.validate();
    assert!(
        !has(&issues, IssueCode::A11yContrast, &created["$label"]),
        "{issues:#?}"
    );

    insert(&mut session, json!([{ "kind": instance_of(&badge) }]));
    let issues = session.validate();
    assert!(
        has(&issues, IssueCode::A11yContrast, &created["$label"]),
        "light text straight on the light page: {issues:#?}"
    );
}

// Un texte alternatif ou un libellé lié à une prop est jugé avec la valeur de chaque instance.
#[test]
fn bound_alt_and_labels_use_each_instance_value() {
    let mut session = session();
    let (figure, created) = component(
        &mut session,
        "Figure",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$img", "parent": "$root", "kind": { "type": "Image", "alt": "",
              "source": { "kind": "Url", "url": "https://example.com/a.png" }, "intrinsic": { "width": 10, "height": 10 } } }
        ]),
        json!([{ "name": "alt", "node": "$img", "field": "ImageAlt" }]),
    );
    let (cta, _) = component(
        &mut session,
        "Cta",
        json!([{ "ref": "$root", "kind": { "type": "Button", "label": "Go" } }]),
        json!([{ "name": "label", "node": "$root", "field": "Label" }]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$photo", "kind": instance_of(&figure) },
            { "ref": "$cta", "kind": instance_of(&cta) }
        ]),
    );
    run_json(
        &mut session,
        json!([
            { "op": "set_props", "node": applied.created["$photo"].to_string(), "props": { "type": "ComponentInstance",
              "overrides": [{ "prop": "alt", "value": { "kind": "Text", "value": "Photo" } }] } },
            { "op": "set_props", "node": applied.created["$cta"].to_string(), "props": { "type": "ComponentInstance",
              "overrides": [{ "prop": "label", "value": { "kind": "Text", "value": " " } }] } }
        ]),
    );
    let issues = session.validate();
    assert!(!has(&issues, IssueCode::A11yImgAlt, &created["$img"]), "{issues:#?}");
    assert!(
        has(&issues, IssueCode::A11yAccessibleName, &applied.created["$cta"]),
        "empty label from the override: {issues:#?}"
    );

    insert(&mut session, json!([{ "kind": instance_of(&figure) }]));
    let issues = session.validate();
    assert!(
        has(&issues, IssueCode::A11yImgAlt, &created["$img"]),
        "the default alt is empty: {issues:#?}"
    );
}

// La taille de texte d'un champ est héritée de son hôte au rendu.
#[test]
fn input_font_size_is_inherited_at_render() {
    let mut session = session();
    let (field, _) = component(
        &mut session,
        "Field",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" }, "style": { "font_size": { "base": "xs" } } },
            { "parent": "$root", "kind": { "type": "Slot" } }
        ]),
        json!([]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$field", "kind": instance_of(&field) },
            { "ref": "$input", "parent": "$field", "kind": { "type": "Input", "input_type": "email" } }
        ]),
    );
    let issues = session.validate();
    assert!(
        has(&issues, IssueCode::RespInputFontSize, &applied.created["$input"]),
        "{issues:#?}"
    );
}
