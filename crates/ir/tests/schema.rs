//! Formes sérialisées : aller-retour JSON, JSON Schema des outils LLM, types TypeScript.

mod common;

use std::collections::BTreeSet;

use common::*;
use ir::command::Command;
use ir::*;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

fn populated() -> Session {
    let mut session = session();
    let root = home_root(&session);
    run_json(
        &mut session,
        json!([
            { "op": "create_layout", "name": "site" },
            { "op": "insert_nodes", "parent": root.to_string(), "nodes": [
                { "ref": "$main", "kind": { "type": "Box", "role": { "kind": "Main" } } },
                { "ref": "$hero", "parent": "$main", "kind": { "type": "Stack", "role": { "kind": "Section" } },
                  "style": { "padding": { "base": "6", "md": "12" }, "gap": { "base": "4", "lg": "8" },
                             "background": { "base": { "kind": "Gradient", "direction": "to-br", "from": "primary", "to": "accent" } } },
                  "hover": { "shadow": { "base": "lg" } }, "visibility": { "base": true, "2xl": false } },
                { "parent": "$hero", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" },
                  "content": [{ "text": "Bonjour", "strong": true }, { "text": "\nmonde", "color": "primary/80" }] } },
                { "parent": "$hero", "kind": { "type": "Link", "href": { "kind": "External", "url": "https://example.com" }, "label": "Lien" } },
                { "parent": "$hero", "kind": { "type": "Image", "source": { "kind": "Placeholder", "label": "Hero", "ratio": "video" }, "alt": "Hero" } }
            ]}
        ]),
    );
    session
}

#[test]
fn documents_round_trip_through_json() {
    let session = populated();
    let doc = session.document();
    let text = serde_json::to_string(doc).unwrap();
    let back: Document = serde_json::from_str(&text).unwrap();
    assert_eq!(&back, doc);
    assert_eq!(ir::migrate::load(&text).unwrap(), *doc);
    // Forme compacte : valeurs par défaut omises, breakpoints en chaînes.
    assert!(text.contains(r#""padding_top":{"base":"6","md":"12"}"#), "{text}");
    assert!(text.contains(r#""2xl":false"#));
}

#[test]
fn commands_and_ops_round_trip() {
    let commands: Vec<Command> = json(json!([
        { "op": "set_style", "node": "$hero", "state": "hover", "style": { "padding_x": { "base": "4", "md": null }, "gap": null } },
        { "op": "update_page", "page": "p_0123456789", "layout": null },
        { "op": "update_settings", "favicon": null, "lang": "fr-SN" },
        { "op": "set_token", "edit": { "kind": "Font", "name": "display", "value": { "kind": "Google", "family": "Inter", "weights": [400, 700] } } }
    ]));
    let back: Vec<Command> = json(serde_json::to_value(&commands).unwrap());
    assert_eq!(back, commands);
    // `null` (effacer) et champ absent (inchangé) restent distincts.
    let Command::UpdatePage { layout, name, .. } = &commands[1] else {
        panic!()
    };
    assert_eq!((layout, name), (&Some(None), &None));

    // Des ops sérialisées puis rejouées sur le même document de départ donnent le même résultat.
    let mut replay = Document::new("Test", &mut IdGen::from_seed(11));
    let mut other = Session::new(replay.clone(), 99);
    let root = home_root(&other);
    let applied = run_json(
        &mut other,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [{ "kind": { "type": "Box" } }] }]),
    );
    let ops: Vec<Op> = json(serde_json::to_value(&applied.ops).unwrap());
    for op in &ops {
        op.apply(&mut replay).unwrap();
    }
    assert_eq!(&replay, other.document(), "serialized ops replay identically");
}

fn refs(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(target)) = map.get("$ref") {
                out.insert(target.trim_start_matches("#/$defs/").to_owned());
            }
            map.values().for_each(|v| refs(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| refs(v, out)),
        _ => {}
    }
}

#[test]
fn tool_schemas_are_not_recursive() {
    // Le mode `strict` des outils refuse les schémas récursifs : aucune définition ne doit
    // pouvoir s'atteindre elle-même.
    let schema = serde_json::to_value(schemars::schema_for!(Command)).unwrap();
    let defs = schema["$defs"].as_object().expect("definitions");
    for (name, definition) in defs {
        let mut seen = BTreeSet::new();
        let mut stack = vec![definition.clone()];
        while let Some(current) = stack.pop() {
            let mut found = BTreeSet::new();
            refs(&current, &mut found);
            for target in found {
                assert_ne!(&target, name, "`{name}` is recursive");
                if seen.insert(target.clone()) {
                    stack.push(defs[&target].clone());
                }
            }
        }
    }
    // Les valeurs de style sont des énumérations de chaînes compactes.
    assert!(defs["Space"]["enum"].as_array().unwrap().contains(&json!("0.5")));
    // Patchs responsive nommés d'après leur type, avec le breakpoint `2xl`.
    assert!(defs["ResponsivePatch_Space"]["properties"].get("2xl").is_some());
    let numbered = |k: &String| {
        k.trim_start_matches("ResponsivePatch")
            .starts_with(|c: char| c.is_ascii_digit())
    };
    assert!(!defs.keys().any(numbered), "{:?}", defs.keys());
}

#[test]
fn typescript_declarations_are_generated() {
    let cfg = Config::default();
    let node = Node::decl(&cfg);
    assert!(node.contains("type Node"), "{node}");
    assert!(node.contains("parent: NodeId | null"), "{node}");
    let space = Space::decl(&cfg);
    assert!(space.contains("\"0.5\""), "{space}");
    let responsive = <Responsive<Space>>::decl(&cfg);
    assert!(responsive.contains("\"2xl\"?"), "{responsive}");
    let command = Command::decl(&cfg);
    assert!(command.contains("\"op\": \"insert_nodes\""), "{command}");
    let applied = Applied::decl(&cfg);
    assert!(applied.contains("issues: Array<Issue>"), "{applied}");
}
