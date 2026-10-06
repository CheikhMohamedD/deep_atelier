//! Taille réaliste : un document de plusieurs centaines de nœuds reste rapide à modifier et valider.

mod common;

use common::*;
use serde_json::json;
use std::time::Instant;

#[test]
fn a_large_page_stays_responsive() {
    let mut session = session();
    let root = home_root(&session);
    // 40 sections × 12 nœuds ≈ 500 nœuds.
    for i in 0..40 {
        run_json(
            &mut session,
            json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
                { "ref": format!("$s{i}"), "kind": { "type": "Box", "role": { "kind": "Section" } }, "style": { "padding": { "base": "6", "md": "12" } } },
                { "parent": format!("$s{i}"), "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h2" }, "content": [{ "text": "Titre" }] } },
                { "ref": format!("$g{i}"), "parent": format!("$s{i}"), "kind": { "type": "Grid", "role": { "kind": "List" } }, "style": { "columns": { "base": "1", "md": "3" } } },
                { "ref": format!("$c{i}a"), "parent": format!("$g{i}"), "kind": { "type": "Stack", "role": { "kind": "ListItem" } } },
                { "parent": format!("$c{i}a"), "kind": { "type": "Text", "content": [{ "text": "Carte" }] } },
                { "parent": format!("$c{i}a"), "kind": { "type": "Button", "label": "Voir" } },
                { "ref": format!("$c{i}b"), "parent": format!("$g{i}"), "kind": { "type": "Stack", "role": { "kind": "ListItem" } } },
                { "parent": format!("$c{i}b"), "kind": { "type": "Text", "content": [{ "text": "Carte" }] } },
                { "parent": format!("$c{i}b"), "kind": { "type": "Button", "label": "Voir" } },
                { "ref": format!("$c{i}c"), "parent": format!("$g{i}"), "kind": { "type": "Stack", "role": { "kind": "ListItem" } } },
                { "parent": format!("$c{i}c"), "kind": { "type": "Text", "content": [{ "text": "Carte" }] } },
                { "parent": format!("$c{i}c"), "kind": { "type": "Button", "label": "Voir" } }
            ]}]),
        );
    }
    assert!(session.document().nodes.len() > 480);
    let start = Instant::now();
    let issues = session.validate();
    let elapsed = start.elapsed();
    assert!(
        issues.iter().all(|i| !i.is_error()),
        "{:#?}",
        issues.iter().filter(|i| i.is_error()).take(3).collect::<Vec<_>>()
    );
    // Garde-fou large (build debug, machine de CI partagée).
    assert!(elapsed.as_millis() < 2_000, "validation took {elapsed:?}");
}
