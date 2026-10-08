//! Santé du service : la base répond.

use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::error::ApiError;
use crate::schema::Health;

pub async fn health(State(state): State<AppState>) -> Result<Json<Health>, ApiError> {
    sqlx::query("select 1").execute(&state.db).await.map_err(|error| {
        tracing::warn!(%error, "database health check failed");
        ApiError::Unavailable("database unavailable".to_owned())
    })?;
    Ok(Json(Health {
        status: "ok".to_owned(),
    }))
}
