//! Routes de l'API. Toutes, sauf `/health`, exigent un jeton d'utilisateur connecté.
//!
//! | Méthode | Chemin | Effet |
//! |---|---|---|
//! | GET | `/health` | base joignable |
//! | GET, POST | `/projects` | projets de l'utilisateur ; création (document vide) |
//! | GET, PATCH, DELETE | `/projects/{id}` | lecture, renommage, suppression |
//! | GET, PUT | `/projects/{id}/document` | document courant ; sauvegarde (`base_version`) |
//! | GET, POST | `/projects/{id}/versions` | versions ; nouvelle version du document courant |
//! | GET | `/projects/{id}/versions/{number}` | une version et son document |
//! | POST | `/projects/{id}/versions/{number}/restore` | restauration (`base_version`) |

mod documents;
mod health;
mod projects;
mod versions;

use axum::Router;
use axum::routing::{get, post};

use crate::AppState;
use crate::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/projects", get(projects::list).post(projects::create))
        .route(
            "/projects/{id}",
            get(projects::get).patch(projects::rename).delete(projects::delete),
        )
        .route("/projects/{id}/document", get(documents::get).put(documents::save))
        .route("/projects/{id}/versions", get(versions::list).post(versions::create))
        .route("/projects/{id}/versions/{number}", get(versions::get))
        .route("/projects/{id}/versions/{number}/restore", post(versions::restore))
        .fallback(|| async { ApiError::NotFound })
}

/// Texte de 1 à `max` caractères, espaces de bord retirés.
fn bounded_text(field: &str, raw: &str, max: usize) -> Result<String, ApiError> {
    let text = raw.trim();
    let length = text.chars().count();
    if length == 0 || length > max {
        return Err(ApiError::InvalidRequest(format!(
            "{field} must have 1 to {max} characters"
        )));
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_texts_are_trimmed_and_checked() {
        assert_eq!(bounded_text("name", "  Mon site ", 120).expect("valid"), "Mon site");
        assert!(bounded_text("name", "   ", 120).is_err());
        assert!(bounded_text("name", &"é".repeat(121), 120).is_err());
        assert!(bounded_text("name", &"é".repeat(120), 120).is_ok());
    }
}
