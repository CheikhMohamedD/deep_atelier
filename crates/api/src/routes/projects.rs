//! Projets de l'utilisateur.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use super::bounded_text;
use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::extract::{ApiPath, Body};
use crate::schema::{CreatedProject, NewProject, ProjectList, ProjectPatch, ProjectResponse};
use crate::{db, document};

const NAME_MAX: usize = 120;

pub async fn list(State(state): State<AppState>, user: AuthUser) -> Result<Json<ProjectList>, ApiError> {
    let projects = db::list_projects(&state.db, user.id).await?;
    Ok(Json(ProjectList { projects }))
}

/// Crée un projet avec un document neuf (une page d'accueil vide).
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Body(body): Body<NewProject>,
) -> Result<(StatusCode, Json<CreatedProject>), ApiError> {
    let name = bounded_text("name", &body.name, NAME_MAX)?;
    let blank = document::blank(&name)?;
    let (project, document) = db::create_project(&state.db, user.id, &name, &blank).await?;
    Ok((StatusCode::CREATED, Json(CreatedProject { project, document })))
}

pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let project = db::get_project(&state.db, user.id, id).await?;
    Ok(Json(ProjectResponse { project }))
}

/// Renomme le projet (le nom du site, dans le document, se change par une commande de l'IR).
pub async fn rename(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    Body(body): Body<ProjectPatch>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let name = bounded_text("name", &body.name, NAME_MAX)?;
    let project = db::rename_project(&state.db, user.id, id, &name).await?;
    Ok(Json(ProjectResponse { project }))
}

pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    db::delete_project(&state.db, user.id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
