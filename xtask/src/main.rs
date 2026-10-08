//! Tâches du dépôt.
//!
//! - `cargo xtask codegen` : écrit les types TypeScript de l'IR, du canvas et de l'API (ts-rs) dans
//!   `packages/ir-types/src/generated`, et leur index `packages/ir-types/src/index.ts`.
//! - `cargo xtask codegen --check` : échoue si les fichiers versionnés ne correspondent plus aux
//!   types Rust (CI). Ces fichiers ne s'éditent jamais à la main.
//! - `cargo xtask demo <landing|kitchen> <fichier>` : écrit un document de démonstration en JSON
//!   (projets des tests de bout en bout de l'éditeur).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ts_rs::{Config, TS};

/// En-tête de l'index généré.
const INDEX_HEADER: &str = "// Généré par `cargo xtask codegen` à partir des types Rust : ne pas modifier.\n";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["codegen"] => false,
        ["codegen", "--check"] => true,
        ["demo", name, path] => return demo(name, Path::new(path)),
        _ => {
            eprintln!("usage: cargo xtask codegen [--check] | cargo xtask demo <landing|kitchen> <file>");
            return ExitCode::FAILURE;
        }
    };
    match codegen(&workspace_root(), check) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Écrit un document de démonstration (`compiler_web::demo`) en JSON.
fn demo(name: &str, path: &Path) -> ExitCode {
    let doc = match name {
        "landing" => compiler_web::demo::landing(),
        "kitchen" => compiler_web::demo::kitchen_sink(),
        other => {
            eprintln!("unknown demo `{other}` (landing, kitchen)");
            return ExitCode::FAILURE;
        }
    };
    let json = match serde_json::to_string(&doc) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
        && let Err(e) = fs::create_dir_all(parent)
    {
        eprintln!("{}: {e}", parent.display());
        return ExitCode::FAILURE;
    }
    match fs::write(path, json) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in the workspace")
        .to_path_buf()
}

/// Fichiers générés : chemin relatif à `packages/ir-types/src` → contenu.
fn generate(scratch: &Path) -> Result<BTreeMap<PathBuf, String>, String> {
    // Les entiers 64 bits (taille d'un asset) sont des nombres dans le JSON.
    let cfg = Config::new().with_out_dir(scratch).with_large_int("number");
    let export = |result: Result<(), ts_rs::ExportError>| result.map_err(|e| format!("ts-rs: {e}"));
    export(ir::Document::export_all(&cfg))?;
    export(ir::Transaction::export_all(&cfg))?;
    export(ir::Applied::export_all(&cfg))?;
    export(ir::Command::export_all(&cfg))?;
    export(ir::Issue::export_all(&cfg))?;
    export(ir::ChangeSet::export_all(&cfg))?;
    export(ir::ChangeUnit::export_all(&cfg))?;
    export(compiler_web::canvas::CanvasPage::export_all(&cfg))?;
    // Contrat de l'API (crates/api/src/schema.rs).
    {
        use api::schema::*;
        export(Health::export_all(&cfg))?;
        export(NewProject::export_all(&cfg))?;
        export(ProjectPatch::export_all(&cfg))?;
        export(SaveDocument::export_all(&cfg))?;
        export(NewVersion::export_all(&cfg))?;
        export(Restore::export_all(&cfg))?;
        export(ProjectList::export_all(&cfg))?;
        export(ProjectResponse::export_all(&cfg))?;
        export(CreatedProject::export_all(&cfg))?;
        export(VersionList::export_all(&cfg))?;
        export(VersionResponse::export_all(&cfg))?;
        export(VersionDetail::export_all(&cfg))?;
        export(Restored::export_all(&cfg))?;
        export(ErrorBody::export_all(&cfg))?;
        // Réponse de `PUT /projects/{id}/document`.
        export(api::db::Saved::export_all(&cfg))?;
    }

    let mut files = BTreeMap::new();
    collect(scratch, scratch, &mut files)?;
    let mut index = String::from(INDEX_HEADER);
    for path in files.keys() {
        let module = path.with_extension("");
        index.push_str(&format!("export type * from \"./generated/{}\";\n", slash(&module)));
    }
    let mut out: BTreeMap<PathBuf, String> = files
        .into_iter()
        .map(|(path, text)| (Path::new("generated").join(path), text))
        .collect();
    out.insert(PathBuf::from("index.ts"), index);
    Ok(out)
}

fn slash(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn collect(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, String>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect(root, &path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "ts") {
            let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let relative = path.strip_prefix(root).map_err(|e| e.to_string())?.to_path_buf();
            files.insert(relative, text);
        }
    }
    Ok(())
}

/// Fichiers versionnés actuels (index et dossier `generated`).
fn current(src: &Path) -> Result<BTreeMap<PathBuf, String>, String> {
    let mut files = BTreeMap::new();
    let generated = src.join("generated");
    if generated.is_dir() {
        let mut inner = BTreeMap::new();
        collect(&generated, &generated, &mut inner)?;
        files.extend(inner.into_iter().map(|(p, t)| (Path::new("generated").join(p), t)));
    }
    if let Ok(index) = fs::read_to_string(src.join("index.ts")) {
        files.insert(PathBuf::from("index.ts"), index);
    }
    Ok(files)
}

fn codegen(root: &Path, check: bool) -> Result<(), String> {
    let src = root.join("packages/ir-types/src");
    let scratch = std::env::temp_dir().join(format!("deep-atelier-codegen-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let generated = generate(&scratch);
    let _ = fs::remove_dir_all(&scratch);
    let generated = generated?;
    let existing = current(&src)?;

    if check {
        let stale: Vec<String> = generated
            .iter()
            .filter(|(path, text)| existing.get(*path) != Some(*text))
            .map(|(path, _)| slash(path))
            .chain(
                existing
                    .keys()
                    .filter(|path| !generated.contains_key(*path))
                    .map(|path| slash(path)),
            )
            .collect();
        if stale.is_empty() {
            println!("packages/ir-types is up to date ({} files)", generated.len());
            return Ok(());
        }
        return Err(format!(
            "packages/ir-types is out of date, run `cargo xtask codegen`:\n  {}",
            stale.join("\n  ")
        ));
    }

    let _ = fs::remove_dir_all(src.join("generated"));
    for (path, text) in &generated {
        let target = src.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&target, text).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    println!("wrote {} files to packages/ir-types/src", generated.len());
    Ok(())
}
