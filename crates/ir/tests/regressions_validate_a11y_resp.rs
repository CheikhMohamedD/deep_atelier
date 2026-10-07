//! Régressions de la revue (a) : validation accessibilité et responsive.

mod common;

use common::*;
use ir::*;
use serde_json::json;

fn insert(session: &mut Session, nodes: serde_json::Value) -> Applied {
    let root = home_root(session);
    run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": nodes }]),
    )
}

fn has(issues: &[Issue], code: IssueCode, node: &NodeId) -> bool {
    issues.iter().any(|i| i.code == code && i.node.as_ref() == Some(node))
}

fn count(issues: &[Issue], code: IssueCode) -> usize {
    issues.iter().filter(|i| i.code == code).count()
}

/// Applique les corrections proposées pour un code sur un nœud.
fn apply_fix(session: &mut Session, code: IssueCode, node: &NodeId) {
    let issues = session.validate();
    let issue = issues
        .iter()
        .find(|i| i.code == code && i.node.as_ref() == Some(node))
        .unwrap_or_else(|| panic!("{code:?} on {node}"));
    run(session, issue.fix.clone().expect("a fix is proposed"));
}

/// Valeur effective d'une dimension à un breakpoint (`auto` si la propriété est absente).
fn effective(value: &Option<Responsive<Size>>, bp: Breakpoint) -> Size {
    value.as_ref().map_or(Size::Auto, |v| *v.resolve(bp))
}

/// Dimensions effectives (largeur, hauteur et leurs minimums) au-dessus du mobile.
fn desktop_sizes(style: &Style) -> Vec<Size> {
    [Breakpoint::Md, Breakpoint::Lg, Breakpoint::Xl]
        .into_iter()
        .flat_map(|bp| {
            [
                effective(&style.width, bp),
                effective(&style.min_width, bp),
                effective(&style.height, bp),
                effective(&style.min_height, bp),
            ]
        })
        .collect()
}

/// Crée un composant à partir d'un sous-arbre inséré sur la page ; renvoie son id.
fn component(session: &mut Session, name: &str, nodes: serde_json::Value, props: serde_json::Value) -> ComponentId {
    let applied = insert(session, nodes);
    let props: Vec<serde_json::Value> = props
        .as_array()
        .map(|list| {
            list.iter()
                .map(|p| json!({ "name": p["name"], "node": applied.created[p["node"].as_str().unwrap()].to_string(), "field": p["field"] }))
                .collect()
        })
        .unwrap_or_default();
    let applied = run_json(
        session,
        json!([{ "op": "create_component", "node": applied.created["$root"].to_string(), "name": name, "props": props }]),
    );
    run(
        session,
        vec![Command::DeleteNodes {
            nodes: vec![r(&applied.inserted[0])],
        }],
    );
    applied.components[0].clone()
}

// #27 : la correction de cible tactile traite aussi une largeur trop étroite.
#[test]
fn touch_target_fix_resolves_a_too_narrow_width() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$narrow", "kind": { "type": "Button", "label": "OK" }, "style": { "width": { "base": "8" } } },
            { "ref": "$icon", "kind": { "type": "Button", "label": "X" }, "style": { "width": { "base": "8" }, "height": { "base": "8" } } },
            { "ref": "$link", "kind": { "type": "Link", "label": "Lien", "href": { "kind": "External", "url": "https://example.com" } },
              "style": { "width": { "base": "20px" } } }
        ]),
    );
    for reference in ["$narrow", "$icon", "$link"] {
        let node = applied.created[reference].clone();
        let before = session.document().nodes[&node].style.clone();
        apply_fix(&mut session, IssueCode::RespTouchTarget, &node);
        let issues = session.validate();
        assert!(
            !has(&issues, IssueCode::RespTouchTarget, &node),
            "{reference}: warning remains after its fix"
        );
        let after = &session.document().nodes[&node].style;
        assert_eq!(
            desktop_sizes(after),
            desktop_sizes(&before),
            "{reference}: desktop layout preserved"
        );
    }
    let doc = session.document();
    let narrow = &doc.nodes[&applied.created["$narrow"]].style;
    assert_eq!(effective(&narrow.width, Breakpoint::Base), Size::Auto);
    assert_eq!(effective(&narrow.min_width, Breakpoint::Base), Size::Space(Space::S11));
}

// #31 : la correction de `min_width` (et celle de cible tactile) ne touche que le mobile.
#[test]
fn responsive_fixes_keep_larger_breakpoints() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$min", "kind": { "type": "Box" }, "style": { "min_width": { "base": "500px", "lg": "800px" } } },
            { "ref": "$only", "kind": { "type": "Box" }, "style": { "min_width": { "base": "500px" } } },
            { "ref": "$tall", "kind": { "type": "Button", "label": "OK" }, "style": { "height": { "base": "8", "lg": "16" } } },
            { "ref": "$bounded", "kind": { "type": "Button", "label": "OK" },
              "style": { "height": { "base": "8" }, "min_height": { "base": "4", "xl": "6" } } }
        ]),
    );
    for (reference, code) in [
        ("$min", IssueCode::RespFixedWidthOverflow),
        ("$only", IssueCode::RespFixedWidthOverflow),
        ("$tall", IssueCode::RespTouchTarget),
        ("$bounded", IssueCode::RespTouchTarget),
    ] {
        let node = applied.created[reference].clone();
        let before = session.document().nodes[&node].style.clone();
        apply_fix(&mut session, code, &node);
        assert!(
            !has(&session.validate(), code, &node),
            "{reference}: {code:?} remains after its fix"
        );
        let after = &session.document().nodes[&node].style;
        assert_eq!(
            desktop_sizes(after),
            desktop_sizes(&before),
            "{reference}: desktop layout preserved"
        );
    }
    let doc = session.document();
    let min = doc.nodes[&applied.created["$min"]].style.min_width.as_ref().unwrap();
    assert_eq!(min.lg, Some(Size::Px(800)));
    let tall = doc.nodes[&applied.created["$tall"]].style.height.as_ref().unwrap();
    assert_eq!(tall.lg, Some(Size::Space(Space::S16)));
}

// #30 : l'alt d'une image nomme le bouton ou le lien qui la contient.
#[test]
fn image_alt_names_its_button_or_link() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$search", "kind": { "type": "Button" } },
            { "parent": "$search", "kind": { "type": "Image", "alt": "Rechercher",
              "source": { "kind": "Url", "url": "https://example.com/loupe.png" }, "intrinsic": { "width": 24, "height": 24 } } },
            { "ref": "$home", "kind": { "type": "Link", "href": { "kind": "External", "url": "https://example.com" } } },
            { "parent": "$home", "kind": { "type": "Image", "alt": "Logo",
              "source": { "kind": "Url", "url": "https://example.com/logo.png" }, "intrinsic": { "width": 120, "height": 40 } } },
            { "ref": "$deco", "kind": { "type": "Button" } },
            { "parent": "$deco", "kind": { "type": "Image", "alt": "Décor",
              "source": { "kind": "Url", "url": "https://example.com/d.png" }, "intrinsic": { "width": 24, "height": 24 } },
              "meta": { "a11y_hidden": true } }
        ]),
    );
    let issues = session.validate();
    assert!(!has(
        &issues,
        IssueCode::A11yAccessibleName,
        &applied.created["$search"]
    ));
    assert!(!has(&issues, IssueCode::A11yAccessibleName, &applied.created["$home"]));
    assert!(
        has(&issues, IssueCode::A11yAccessibleName, &applied.created["$deco"]),
        "a hidden image does not name its button"
    );
}

// #32 : `a11y.hidden` sur un conteneur masque tout son sous-arbre.
#[test]
fn hidden_ancestors_hide_their_subtree() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$wrapper", "kind": { "type": "Box" }, "meta": { "a11y_hidden": true } },
            { "ref": "$img", "parent": "$wrapper", "kind": { "type": "Image" } },
            { "ref": "$icon", "parent": "$wrapper", "kind": { "type": "Icon", "name": "x" }, "meta": { "a11y_hidden": false } },
            { "ref": "$button", "kind": { "type": "Button" } },
            { "ref": "$inner", "parent": "$button", "kind": { "type": "Box" }, "meta": { "a11y_hidden": true } },
            { "parent": "$inner", "kind": { "type": "Text", "content": [{ "text": "Menu" }] } },
            { "ref": "$visible", "kind": { "type": "Image" } }
        ]),
    );
    let issues = session.validate();
    assert!(!has(&issues, IssueCode::A11yImgAlt, &applied.created["$img"]));
    assert!(!has(&issues, IssueCode::A11yAccessibleName, &applied.created["$icon"]));
    assert!(
        has(&issues, IssueCode::A11yAccessibleName, &applied.created["$button"]),
        "text inside a hidden wrapper does not name the button"
    );
    assert!(has(&issues, IssueCode::A11yImgAlt, &applied.created["$visible"]));
}

// #33 : une instance ne nomme un bouton que par son contenu rendu.
#[test]
fn instances_name_a_button_only_through_their_content() {
    let mut session = session();
    let empty = component(
        &mut session,
        "Empty",
        json!([{ "ref": "$root", "kind": { "type": "Stack" } }]),
        json!([]),
    );
    let glyph = component(
        &mut session,
        "CloseGlyph",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "parent": "$root", "kind": { "type": "Icon", "name": "x" } }
        ]),
        json!([]),
    );
    let titled = component(
        &mut session,
        "Titled",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "parent": "$root", "kind": { "type": "Text", "content": [{ "text": "Menu" }] } }
        ]),
        json!([]),
    );
    let shell = component(
        &mut session,
        "Shell",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "parent": "$root", "kind": { "type": "Slot" } }
        ]),
        json!([]),
    );
    let tag = component(
        &mut session,
        "Tag",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$text", "parent": "$root", "kind": { "type": "Text", "content": [] } }
        ]),
        json!([{ "name": "label", "node": "$text", "field": "Text" }]),
    );
    let instance = |component: &ComponentId| json!({ "type": "ComponentInstance", "component": component.to_string() });
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$empty", "kind": { "type": "Button" } },
            { "parent": "$empty", "kind": instance(&empty) },
            { "ref": "$glyph", "kind": { "type": "Button" } },
            { "parent": "$glyph", "kind": instance(&glyph) },
            { "ref": "$titled", "kind": { "type": "Button" } },
            { "parent": "$titled", "kind": instance(&titled) },
            { "ref": "$hidden", "kind": { "type": "Button" } },
            { "parent": "$hidden", "kind": instance(&titled), "meta": { "a11y_hidden": true } },
            { "ref": "$filled", "kind": { "type": "Button" } },
            { "ref": "$shell", "parent": "$filled", "kind": instance(&shell) },
            { "parent": "$shell", "kind": { "type": "Text", "content": [{ "text": "Ouvrir" }] } },
            { "ref": "$unfilled", "kind": { "type": "Link", "href": { "kind": "External", "url": "https://example.com" } } },
            { "parent": "$unfilled", "kind": instance(&shell) },
            { "ref": "$tagged", "kind": { "type": "Button" } },
            { "ref": "$tag", "parent": "$tagged", "kind": instance(&tag) },
            { "ref": "$untagged", "kind": { "type": "Button" } },
            { "parent": "$untagged", "kind": instance(&tag) }
        ]),
    );
    run_json(
        &mut session,
        json!([{ "op": "set_props", "node": applied.created["$tag"].to_string(), "props": { "type": "ComponentInstance",
                 "overrides": [{ "prop": "label", "value": { "kind": "Text", "value": "Étiquette" } }] } }]),
    );
    let issues = session.validate();
    let unnamed = |reference: &str| has(&issues, IssueCode::A11yAccessibleName, &applied.created[reference]);
    assert!(unnamed("$empty"), "empty component");
    assert!(unnamed("$glyph"), "decorative icon only");
    assert!(!unnamed("$titled"));
    assert!(unnamed("$hidden"), "hidden instance");
    assert!(!unnamed("$filled"), "slot content names the button");
    assert!(unnamed("$unfilled"), "empty slot");
    assert!(!unnamed("$tagged"), "text from the prop override");
    assert!(unnamed("$untagged"), "empty prop default");
}

// #34 : les titres et `Main` masqués ne comptent pas.
fn heading(level: &str, text: &str) -> serde_json::Value {
    json!({ "type": "Text", "role": { "kind": "Heading", "level": level }, "content": [{ "text": text }] })
}

#[test]
fn a_hidden_h1_or_main_is_missing() {
    let mut session = session();
    insert(
        &mut session,
        json!([
            { "kind": heading("h1", "Masqué"), "meta": { "a11y_hidden": true } },
            { "kind": { "type": "Box", "role": { "kind": "Main" } }, "meta": { "a11y_hidden": true } }
        ]),
    );
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMissingH1), 1, "only H1 is hidden");
    assert_eq!(count(&issues, IssueCode::A11yMissingMain), 1, "only Main is hidden");
}

#[test]
fn hidden_headings_and_mains_are_not_duplicates() {
    let mut session = session();
    insert(
        &mut session,
        json!([
            { "kind": heading("h1", "Masqué"), "meta": { "a11y_hidden": true } },
            { "ref": "$wrapper", "kind": { "type": "Box" }, "meta": { "a11y_hidden": true } },
            { "parent": "$wrapper", "kind": heading("h1", "Masqué aussi") },
            { "kind": heading("h1", "Visible") },
            { "kind": { "type": "Box", "role": { "kind": "Main" } }, "meta": { "a11y_hidden": true } },
            { "kind": { "type": "Box", "role": { "kind": "Main" } } }
        ]),
    );
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMultipleH1), 0);
    assert_eq!(count(&issues, IssueCode::A11yMultipleMain), 0);
    assert_eq!(count(&issues, IssueCode::A11yMissingH1), 0);
    assert_eq!(count(&issues, IssueCode::A11yMissingMain), 0);
}

#[test]
fn a_hidden_heading_does_not_bridge_a_level_skip() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "kind": heading("h1", "Titre") },
            { "kind": heading("h2", "Masqué"), "meta": { "a11y_hidden": true } },
            { "ref": "$h3", "kind": heading("h3", "Saut") }
        ]),
    );
    assert!(has(
        &session.validate(),
        IssueCode::A11yHeadingSkip,
        &applied.created["$h3"]
    ));
}

// Relecture : le masquage d'une instance s'étend à l'arbre de son composant dans le rendu de la page.
fn hero(session: &mut Session) -> ComponentId {
    component(
        session,
        "Hero",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "parent": "$root", "kind": heading("h1", "Héros") },
            { "parent": "$root", "kind": { "type": "Box", "role": { "kind": "Main" } } }
        ]),
        json!([]),
    )
}

fn instance_of(component: &ComponentId) -> serde_json::Value {
    json!({ "type": "ComponentInstance", "component": component.to_string() })
}

#[test]
fn a_hidden_instance_adds_no_heading_or_main() {
    let mut session = session();
    let hero = hero(&mut session);
    insert(
        &mut session,
        json!([
            { "kind": instance_of(&hero), "meta": { "a11y_hidden": true } },
            { "kind": heading("h1", "Visible") },
            { "kind": { "type": "Box", "role": { "kind": "Main" } } }
        ]),
    );
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMultipleH1), 0, "{issues:#?}");
    assert_eq!(count(&issues, IssueCode::A11yMultipleMain), 0, "{issues:#?}");
}

#[test]
fn hiding_follows_each_instance_of_a_component() {
    let mut session = session();
    let hero = hero(&mut session);
    insert(
        &mut session,
        json!([
            { "kind": instance_of(&hero) },
            { "kind": instance_of(&hero), "meta": { "a11y_hidden": true } }
        ]),
    );
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMultipleH1), 0, "{issues:#?}");
    assert_eq!(count(&issues, IssueCode::A11yMultipleMain), 0, "{issues:#?}");
    assert_eq!(count(&issues, IssueCode::A11yMissingH1), 0, "{issues:#?}");
    assert_eq!(count(&issues, IssueCode::A11yMissingMain), 0, "{issues:#?}");
}

#[test]
fn an_instance_under_a_hidden_wrapper_provides_no_heading_or_main() {
    let mut session = session();
    let hero = hero(&mut session);
    insert(
        &mut session,
        json!([
            { "ref": "$wrapper", "kind": { "type": "Box" }, "meta": { "a11y_hidden": true } },
            { "parent": "$wrapper", "kind": instance_of(&hero) }
        ]),
    );
    let issues = session.validate();
    assert_eq!(count(&issues, IssueCode::A11yMissingH1), 1, "{issues:#?}");
    assert_eq!(count(&issues, IssueCode::A11yMissingMain), 1, "{issues:#?}");
}

// Relecture : le contenu d'un slot placé sous un conteneur masqué du composant est masqué.
#[test]
fn slot_content_under_a_hidden_wrapper_is_hidden() {
    let mut session = session();
    let decor = component(
        &mut session,
        "Decor",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" } },
            { "ref": "$wrapper", "parent": "$root", "kind": { "type": "Box" }, "meta": { "a11y_hidden": true } },
            { "parent": "$wrapper", "kind": { "type": "Slot" } }
        ]),
        json!([]),
    );
    let hero = hero(&mut session);
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$decor", "kind": instance_of(&decor) },
            { "ref": "$img", "parent": "$decor", "kind": { "type": "Image" } },
            { "parent": "$decor", "kind": heading("h1", "Masqué") },
            { "parent": "$decor", "kind": instance_of(&hero) },
            { "kind": heading("h1", "Visible") },
            { "kind": { "type": "Box", "role": { "kind": "Main" } } },
            { "ref": "$visible", "kind": { "type": "Image" } }
        ]),
    );
    let issues = session.validate();
    assert!(
        !has(&issues, IssueCode::A11yImgAlt, &applied.created["$img"]),
        "{issues:#?}"
    );
    assert!(has(&issues, IssueCode::A11yImgAlt, &applied.created["$visible"]));
    assert_eq!(count(&issues, IssueCode::A11yMultipleH1), 0, "{issues:#?}");
    assert_eq!(count(&issues, IssueCode::A11yMultipleMain), 0, "{issues:#?}");
}

#[test]
fn a_page_under_a_hidden_page_slot_is_hidden() {
    let mut session = session();
    let layout = run_json(&mut session, json!([{ "op": "create_layout", "name": "site" }])).layouts[0].clone();
    let doc = session.document();
    let slot = doc.nodes[&doc.layout(&layout).unwrap().root].children[0].clone();
    let page = doc.pages[0].id.clone();
    run_json(
        &mut session,
        json!([
            { "op": "wrap_nodes", "nodes": [slot.to_string()], "container": { "kind": "Box", "meta": { "a11y_hidden": true } } },
            { "op": "update_page", "page": page.to_string(), "layout": layout.to_string() }
        ]),
    );
    let applied = insert(
        &mut session,
        json!([
            { "kind": heading("h1", "Masqué") },
            { "ref": "$img", "kind": { "type": "Image" } }
        ]),
    );
    let issues = session.validate();
    assert!(
        !has(&issues, IssueCode::A11yImgAlt, &applied.created["$img"]),
        "{issues:#?}"
    );
    assert_eq!(count(&issues, IssueCode::A11yMissingH1), 1, "{issues:#?}");
}

// Relecture : un minimum fixe d'au moins 44 px suffit ; un minimum relatif au contenu ou au
// parent ne garantit pas la cible et passe à 44 px.
#[test]
fn touch_target_minimums_are_judged_by_their_size() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$floored", "kind": { "type": "Button", "label": "OK" },
              "style": { "width": { "base": "8" }, "min_width": { "base": "12" }, "height": { "base": "12" } } },
            { "ref": "$screen", "kind": { "type": "Button", "label": "OK" },
              "style": { "height": { "base": "8" }, "min_height": { "base": "screen" } } },
            { "ref": "$fit", "kind": { "type": "Button", "label": "OK" },
              "style": { "height": { "base": "8" }, "min_height": { "base": "fit" } } },
            { "ref": "$full", "kind": { "type": "Button", "label": "OK" },
              "style": { "width": { "base": "8" }, "min_width": { "base": "full", "lg": "1/2" } } }
        ]),
    );
    let issues = session.validate();
    for reference in ["$floored", "$screen"] {
        assert!(
            !has(&issues, IssueCode::RespTouchTarget, &applied.created[reference]),
            "{reference}: its minimum already makes a 44px target"
        );
    }
    for reference in ["$fit", "$full"] {
        let node = applied.created[reference].clone();
        let before = session.document().nodes[&node].style.clone();
        apply_fix(&mut session, IssueCode::RespTouchTarget, &node);
        assert!(!has(&session.validate(), IssueCode::RespTouchTarget, &node));
        let after = &session.document().nodes[&node].style;
        assert_eq!(
            desktop_sizes(after),
            desktop_sizes(&before),
            "{reference}: desktop layout preserved"
        );
    }
    let doc = session.document();
    let fit = &doc.nodes[&applied.created["$fit"]].style;
    assert_eq!(effective(&fit.height, Breakpoint::Base), Size::Auto);
    assert_eq!(effective(&fit.min_height, Breakpoint::Base), Size::Space(Space::S11));
    let full = &doc.nodes[&applied.created["$full"]].style;
    assert_eq!(effective(&full.width, Breakpoint::Base), Size::Auto);
    assert_eq!(effective(&full.min_width, Breakpoint::Base), Size::Space(Space::S11));
}
