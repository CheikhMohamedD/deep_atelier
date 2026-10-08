//! Snapshots du code produit pour la landing de démonstration et le document « tout-en-un ».
//! Toute évolution de la sortie du compilateur apparaît en revue dans `tests/snapshots/`.

use compiler_web::{Mode, Project, compile, demo};

fn snapshot_all(prefix: &str, project: &Project) {
    for file in &project.files {
        let Some(text) = file.text() else { continue };
        // Les SVG de remplissage et la configuration figée du gabarit n'apportent rien en revue.
        if file.path.ends_with(".svg")
            || matches!(
                file.path.as_str(),
                ".gitignore" | "AGENTS.md" | "eslint.config.mjs" | "tsconfig.json" | "pnpm-workspace.yaml"
            )
        {
            continue;
        }
        let name = format!("{prefix}__{}", file.path.replace(['/', '.', '(', ')', '[', ']'], "_"));
        insta::assert_snapshot!(name, text);
    }
}

#[test]
fn landing_snapshots() {
    snapshot_all("landing", &compile(&demo::landing(), Mode::Export));
}

#[test]
fn kitchen_sink_snapshots() {
    snapshot_all("kitchen", &compile(&demo::kitchen_sink(), Mode::Export));
}

#[test]
fn compilation_is_deterministic() {
    let doc = demo::kitchen_sink();
    assert_eq!(compile(&doc, Mode::Export), compile(&doc, Mode::Export));
    assert_eq!(compile(&doc, Mode::Edit), compile(&doc, Mode::Edit));
}
