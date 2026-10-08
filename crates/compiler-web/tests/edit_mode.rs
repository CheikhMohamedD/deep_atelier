//! Mode édition : chaque élément porte `data-atl-id`, et la source map donne la plage exacte de
//! chaque nœud (lien code ⇄ canvas, étapes g et h). En mode export, aucune trace de l'éditeur.

use std::collections::BTreeSet;

use compiler_web::{Mode, compile, demo};
use ir::{NodeId, NodeKind, PlatformScope};

#[test]
fn edit_mode_maps_every_element_to_its_node() {
    for doc in [demo::landing(), demo::kitchen_sink()] {
        let project = compile(&doc, Mode::Edit);
        let mut mapped: BTreeSet<NodeId> = BTreeSet::new();
        for file in &project.files {
            let Some(text) = file.text() else { continue };
            for range in &file.source_map {
                let slice = &text[range.start..range.end];
                assert!(slice.starts_with('<'), "{}: {slice}", file.path);
                assert!(slice.ends_with('>'), "{}: {slice}", file.path);
                let marker = format!("data-atl-id=\"{}\"", range.node);
                assert!(slice.contains(&marker), "{}: {marker} not in {slice}", file.path);
                mapped.insert(range.node.clone());
            }
        }
        // Tout nœud qui produit un élément est retrouvé dans un fichier.
        for (owner, root) in doc.roots() {
            let _ = owner;
            for id in doc.subtree(&root) {
                let node = &doc.nodes[&id];
                let element = !matches!(node.kind, NodeKind::Slot(_) | NodeKind::RawCode(_))
                    && node.platform != PlatformScope::NativeOnly
                    && !doc
                        .ancestors(&id)
                        .iter()
                        .any(|a| doc.nodes[a].platform == PlatformScope::NativeOnly);
                // La racine générique d'une page devient un fragment ou disparaît.
                let page_root = doc.pages.iter().any(|p| {
                    p.root == id
                        || p.layout
                            .as_ref()
                            .and_then(|l| doc.layout(l))
                            .is_some_and(|l| l.root == id)
                });
                if element && !page_root {
                    assert!(mapped.contains(&id), "{id} ({}) has no range", node.kind.type_name());
                }
            }
        }
    }
}

#[test]
fn export_mode_leaves_no_trace_of_the_editor() {
    for doc in [demo::landing(), demo::kitchen_sink()] {
        let project = compile(&doc, Mode::Export);
        for file in &project.files {
            let Some(text) = file.text() else { continue };
            assert!(!text.contains("data-atl-id"), "{}", file.path);
        }
        let package = project.file("package.json").and_then(|f| f.text()).unwrap_or_default();
        assert!(!package.to_lowercase().contains("atelier\":"), "{package}");
    }
}
