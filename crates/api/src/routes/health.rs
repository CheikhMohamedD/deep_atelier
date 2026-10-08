//! Santé du service : la base répond.

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::AppState;
use crate::error::ApiError;

pub async fn health(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    sqlx::query("select 1").execute(&state.db).await.map_err(|error| {
        tracing::warn!(%error, "database health check failed");
        ApiError::Unavailable("database unavailable".to_owned())
    })?;
    Ok(Json(json!({ "status": "ok" })))
}
