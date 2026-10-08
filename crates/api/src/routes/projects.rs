//! Projets de l'utilisateur.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::bounded_text;
use crate::AppState;
use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::extract::{ApiPath, Body};
use crate::{db, document};

const NAME_MAX: usize = 120;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewProject {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectPatch {
    name: String,
}

pub async fn list(State(state): State<AppState>, user: AuthUser) -> Result<Json<Value>, ApiError> {
    let projects = db::list_projects(&state.db, user.id).await?;
    Ok(Json(json!({ "projects": projects })))
}

/// Crée un projet avec un document neuf (une page d'accueil vide).
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Body(body): Body<NewProject>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let name = bounded_text("name", &body.name, NAME_MAX)?;
    let blank = document::blank(&name)?;
    let (project, document) = db::create_project(&state.db, user.id, &name, &blank).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "project": project, "document": document })),
    ))
}

pub async fn get(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let project = db::get_project(&state.db, user.id, id).await?;
    Ok(Json(json!({ "project": project })))
}

/// Renomme le projet (le nom du site, dans le document, se change par une commande de l'IR).
pub async fn rename(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
    Body(body): Body<ProjectPatch>,
) -> Result<Json<Value>, ApiError> {
    let name = bounded_text("name", &body.name, NAME_MAX)?;
    let project = db::rename_project(&state.db, user.id, id, &name).await?;
    Ok(Json(json!({ "project": project })))
}

pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    db::delete_project(&state.db, user.id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
