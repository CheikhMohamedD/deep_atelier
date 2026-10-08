//! Document courant d'un projet : lecture, sauvegarde automatique.

use axum::Json;
use axum::extract::State;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::db::{self, DocumentState, Saved};
use crate::document;
use crate::error::ApiError;
use crate::extract::{ApiPath, Body};
use crate::schema::SaveDocument;

pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<DocumentState>, ApiError> {
    Ok(Json(db::get_document(&state.db, user.id, id).await?))
}

/// Enregistre le document s'il est intègre et si personne ne l'a modifié depuis `base_version`.
pub async fn save(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    Body(body): Body<SaveDocument>,
) -> Result<Json<Saved>, ApiError> {
    let checked = document::check_async(body.document).await?;
    let saved = db::save_document(&state.db, user.id, id, body.base_version, &checked).await?;
    Ok(Json(saved))
}
