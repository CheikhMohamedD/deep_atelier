//! Intégrité : un document produit par les commandes n'a jamais de problème d'intégrité ; un
//! document altéré à la main (enfant inexistant, cycle, page supprimée) en a.

mod common;

use common::*;
use ir::*;
use serde_json::json;

fn integrity_codes(doc: &Document) -> Vec<IssueCode> {
    validate(doc)
        .into_iter()
        .map(|i| i.code)
        .filter(|c| c.is_integrity())
        .collect()
}

#[test]
fn documents_built_by_commands_have_no_integrity_issue() {
    let mut session = session();
    let root = home_root(&session);
    run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": [
            { "ref": "$section", "kind": { "type": "Box", "role": { "kind": "Section" } } },
            { "parent": "$section", "kind": { "type": "Image", "alt": "", "source": { "kind": "Placeholder", "label": "Photo", "ratio": "video" } } }
        ] }]),
    );
    let doc = session.document();
    // Des problèmes d'accessibilité peuvent exister (alt vide), jamais d'intégrité.
    assert!(integrity_codes(doc).is_empty(), "{:#?}", validate(doc));
}

#[test]
fn altered_documents_have_integrity_issues() {
    let session = session();
    let root = home_root(&session);

    let mut dangling = session.document().clone();
    let missing: NodeId = "n_zzzzzzzzzz".parse().expect("node id");
    dangling.nodes.get_mut(&root).expect("root").children.push(missing);
    assert_eq!(integrity_codes(&dangling), vec![IssueCode::TreeInconsistent]);

    let mut no_page = session.document().clone();
    no_page.pages.clear();
    assert!(integrity_codes(&no_page).contains(&IssueCode::NoPage));

    let mut no_web = session.document().clone();
    no_web.targets.clear();
    assert!(integrity_codes(&no_web).contains(&IssueCode::NoWebTarget));
}
