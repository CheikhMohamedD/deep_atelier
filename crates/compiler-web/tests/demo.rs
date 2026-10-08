//! Landing de démonstration : document valide, compilation, écriture sur disque à la demande
//! (`DEEP_ATELIER_EXPORT_DIR`, utilisé par le job d'export de la CI).

use compiler_web::{FileContents, Mode, compile, demo};
use ir::{Severity, validate};

#[test]
fn the_demo_landing_is_valid() {
    let doc = demo::landing();
    let issues = validate(&doc);
    let errors: Vec<_> = issues.iter().filter(|i| i.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn the_demo_landing_compiles() {
    let doc = demo::landing();
    let project = compile(&doc, Mode::Export);
    let paths: Vec<&str> = project.files.iter().map(|f| f.path.as_str()).collect();
    for expected in [
        "app/layout.tsx",
        "app/globals.css",
        "app/(site)/layout.tsx",
        "app/(site)/page.tsx",
        "app/(site)/a-propos/page.tsx",
        "components/FeatureCard.tsx",
        "components/PricingCard.tsx",
        "package.json",
        "next.config.ts",
    ] {
        assert!(paths.contains(&expected), "{expected} missing from {paths:#?}");
    }
    if let Ok(dir) = std::env::var("DEEP_ATELIER_EXPORT_DIR") {
        let root = std::path::Path::new(&dir);
        for file in &project.files {
            let path = root.join(&file.path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
            match &file.contents {
                FileContents::Text(text) => std::fs::write(&path, text).expect("write file"),
                FileContents::Asset(_) => {}
            }
        }
    }
}

#[test]
fn the_kitchen_sink_is_valid_and_compiles() {
    let doc = demo::kitchen_sink();
    let issues = validate(&doc);
    let errors: Vec<_> = issues.iter().filter(|i| i.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    let project = compile(&doc, Mode::Export);
    if let Ok(dir) = std::env::var("DEEP_ATELIER_KITCHEN_DIR") {
        let root = std::path::Path::new(&dir);
        for file in &project.files {
            let path = root.join(&file.path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
            if let FileContents::Text(text) = &file.contents {
                std::fs::write(&path, text).expect("write file");
            }
        }
    }
}
