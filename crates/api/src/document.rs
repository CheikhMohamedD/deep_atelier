//! Contrôle d'un document reçu : migration vers la version courante de l'IR, puis intégrité
//! (identifiants, arbre, références). Les autres problèmes (accessibilité, responsive, qualité)
//! n'empêchent pas l'enregistrement : un travail en cours n'est jamais perdu, l'éditeur les
//! affiche.

use ir::{Document, IdGen, Issue};
use serde_json::Value;
use uuid::Uuid;

use crate::error::ApiError;

/// Document contrôlé, prêt à être enregistré sous sa forme normalisée.
#[derive(Debug)]
pub struct Checked {
    pub ir_version: i32,
    pub json: Value,
}

/// Migre et contrôle un document ; travail de calcul, à lancer hors du runtime (`spawn_blocking`).
pub fn check(value: Value) -> Result<Checked, ApiError> {
    let document = ir::migrate::migrate(value).map_err(|error| ApiError::InvalidDocument {
        message: error.to_string(),
        issues: Vec::new(),
    })?;
    let issues: Vec<Issue> = ir::validate(&document)
        .into_iter()
        .filter(|issue| issue.code.is_integrity())
        .collect();
    if !issues.is_empty() {
        return Err(ApiError::InvalidDocument {
            message: format!("the document has {} integrity issue(s)", issues.len()),
            issues,
        });
    }
    normalized(&document)
}

/// [`check`] sur le pool de threads bloquants.
pub async fn check_async(value: Value) -> Result<Checked, ApiError> {
    tokio::task::spawn_blocking(move || check(value))
        .await
        .map_err(ApiError::internal)?
}

/// Document neuf d'un projet : une page d'accueil vide, tokens par défaut.
pub fn blank(name: &str) -> Result<Checked, ApiError> {
    let seed = Uuid::new_v4().as_u64_pair().0;
    normalized(&Document::new(name, &mut IdGen::from_seed(seed)))
}

fn normalized(document: &Document) -> Result<Checked, ApiError> {
    let ir_version = i32::try_from(document.version).map_err(ApiError::internal)?;
    Ok(Checked {
        ir_version,
        json: serde_json::to_value(document)?,
    })
}

#[cfg(test)]
mod tests {
    use ir::{IR_VERSION, IssueCode};
    use serde_json::json;

    use super::*;

    #[test]
    fn a_blank_document_passes_the_check() {
        let blank = blank("Mon site").expect("blank document");
        assert_eq!(blank.ir_version, i32::try_from(IR_VERSION).expect("version"));
        assert_eq!(blank.json["name"], "Mon site");
        let checked = check(blank.json.clone()).expect("valid document");
        assert_eq!(checked.json, blank.json);
    }

    #[test]
    fn documents_with_only_quality_issues_are_accepted() {
        let mut value = blank("Site").expect("blank").json;
        // Un document neuf n'a ni H1 ni `main` : avertissements d'accessibilité, pas d'intégrité.
        value["settings"]["lang"] = json!("");
        let checked = check(value).expect("accepted despite issues");
        assert_eq!(checked.json["settings"]["lang"], "");
    }

    #[test]
    fn broken_documents_are_refused_with_their_integrity_issues() {
        let mut value = blank("Site").expect("blank").json;
        let root = value["pages"][0]["root"].as_str().expect("root id").to_owned();
        value["nodes"][&root]["children"] = json!(["n_zzzzzzzzzz"]);
        match check(value) {
            Err(ApiError::InvalidDocument { issues, .. }) => {
                assert!(issues.iter().all(|i| i.code.is_integrity()));
                assert!(issues.iter().any(|i| i.code == IssueCode::TreeInconsistent));
            }
            other => panic!("expected an invalid document, got {other:?}"),
        }
    }

    #[test]
    fn unreadable_documents_are_refused() {
        for value in [json!([]), json!({ "version": 99 }), json!({ "version": 1, "name": 3 })] {
            assert!(matches!(
                check(value),
                Err(ApiError::InvalidDocument { issues, .. }) if issues.is_empty()
            ));
        }
    }
}
