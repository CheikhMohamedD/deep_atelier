//! Versions d'un projet : instantanés du document courant, lecture, restauration.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use super::bounded_text;
use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::extract::{ApiPath, Body};
use crate::schema::{NewVersion, Restore, Restored, VersionDetail, VersionList, VersionResponse};
use crate::{db, document};

const NAME_MAX: usize = 120;
const MESSAGE_MAX: usize = 2000;

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<VersionList>, ApiError> {
    let versions = db::list_versions(&state.db, user.id, id).await?;
    Ok(Json(VersionList { versions }))
}

/// Enregistre le document courant comme nouvelle version, avec un message et un nom facultatif.
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    Body(body): Body<NewVersion>,
) -> Result<(StatusCode, Json<VersionResponse>), ApiError> {
    let message = bounded_text("message", &body.message, MESSAGE_MAX)?;
    let name = match body.name.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(name) => Some(bounded_text("name", name, NAME_MAX)?),
    };
    let version = db::create_version(&state.db, user.id, id, name.as_deref(), &message).await?;
    Ok((StatusCode::CREATED, Json(VersionResponse { version })))
}

pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath((id, number)): ApiPath<(Uuid, i32)>,
) -> Result<Json<VersionDetail>, ApiError> {
    let version = db::get_version(&state.db, user.id, id, number).await?;
    Ok(Json(VersionDetail { version }))
}

/// Restaure une version : l'état courant est gardé comme version `restore`, puis le document de
/// la version (migré vers l'IR courante et contrôlé) devient le document courant.
pub async fn restore(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath((id, number)): ApiPath<(Uuid, i32)>,
    Body(body): Body<Restore>,
) -> Result<Json<Restored>, ApiError> {
    let stored = db::version_document(&state.db, user.id, id, number).await?;
    let checked = document::check_async(stored).await?;
    let (backup, current) = db::restore_version(&state.db, user.id, id, number, body.base_version, &checked).await?;
    Ok(Json(Restored {
        backup,
        document: current,
    }))
}
