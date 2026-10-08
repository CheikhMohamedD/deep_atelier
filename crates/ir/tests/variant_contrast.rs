//! Contraste avec les variantes : le fond, la couleur et la taille du texte d'un nœud de
//! composant suivent les surcharges de la variante choisie par chaque instance (défaut constaté
//! par Lighthouse sur la landing de démonstration de l'étape b).

mod common;

use common::*;
use ir::*;
use serde_json::json;

fn has(issues: &[Issue], code: IssueCode, node: &NodeId) -> bool {
    issues.iter().any(|i| i.code == code && i.node.as_ref() == Some(node))
}

#[test]
fn contrast_follows_the_variant_chosen_by_each_instance() {
    let mut session = session();
    let root = home_root(&session);
    let applied = run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$root", "kind": { "type": "Box" }, "style": { "background": { "base": { "kind": "Color", "color": "card" } } } },
            { "ref": "$text", "parent": "$root", "kind": { "type": "Text", "content": [{ "text": "Description" }] },
              "style": { "text_color": { "base": "muted-foreground" } } }
        ] }]),
    );
    let text = applied.created["$text"].clone();
    let card = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": applied.created["$root"].to_string(), "name": "Card", "props": [] }]),
    );
    let component = card.components[0].clone();
    let card_root = session.document().component(&component).unwrap().root.clone();
    run_json(
        &mut session,
        json!([{ "op": "update_component", "component": component.to_string(), "variants": [{
            "name": "tone", "default": "standard",
            "options": [
                { "name": "standard" },
                { "name": "featured", "overrides": [{ "node": card_root.to_string(),
                  "style": { "background": { "base": { "kind": "Color", "color": "primary" } } } }] }
            ]
        }] }]),
    );
    // L'instance créée par l'extraction garde la variante par défaut : lisible.
    assert!(!has(&session.validate(), IssueCode::A11yContrast, &text));

    // Une instance « featured » met le texte gris sur le fond foncé.
    run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "kind": { "type": "ComponentInstance", "component": component.to_string(),
                        "variants": [{ "axis": "tone", "option": "featured" }] } }
        ] }]),
    );
    let issues = session.validate();
    assert!(has(&issues, IssueCode::A11yContrast, &text), "{issues:#?}");

    // La variante qui éclaircit aussi le texte corrige le problème.
    run_json(
        &mut session,
        json!([{ "op": "update_component", "component": component.to_string(), "variants": [{
            "name": "tone", "default": "standard",
            "options": [
                { "name": "standard" },
                { "name": "featured", "overrides": [
                    { "node": card_root.to_string(), "style": { "background": { "base": { "kind": "Color", "color": "primary" } } } },
                    { "node": text.to_string(), "style": { "text_color": { "base": "primary-foreground" } } }
                ] }
            ]
        }] }]),
    );
    let issues = session.validate();
    assert!(!has(&issues, IssueCode::A11yContrast, &text), "{issues:#?}");
}
