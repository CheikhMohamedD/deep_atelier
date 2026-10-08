//! Erreurs de l'API et leur forme JSON : `{ "error": { "code", "message", … } }`. Les messages
//! sont en anglais ; l'éditeur les traduit à partir du `code`, comme les problèmes de l'IR.

use axum::Json;
use axum::http::header::WWW_AUTHENTICATE;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use ir::Issue;

use crate::schema::{ErrorBody, ErrorDetail};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    Unauthorized(String),
    #[error("not found")]
    NotFound,
    #[error("the document changed since version {base_version} (current version: {current_version})")]
    VersionConflict { base_version: i64, current_version: i64 },
    #[error("{0}")]
    InvalidRequest(String),
    #[error("request body too large")]
    PayloadTooLarge,
    #[error("{message}")]
    InvalidDocument { message: String, issues: Vec<Issue> },
    #[error("{0}")]
    Unavailable(String),
    #[error("internal error")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl ApiError {
    pub fn internal(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Internal(Box::new(error))
    }

    /// Code stable de l'erreur.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::NotFound => "NOT_FOUND",
            Self::VersionConflict { .. } => "VERSION_CONFLICT",
            Self::InvalidRequest(_) => "INVALID_REQUEST",
            Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::InvalidDocument { .. } => "INVALID_DOCUMENT",
            Self::Unavailable(_) => "UNAVAILABLE",
            Self::Internal(_) => "INTERNAL",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::VersionConflict { .. } => StatusCode::CONFLICT,
            Self::InvalidRequest(_) | Self::InvalidDocument { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(error)
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        Self::internal(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        match &self {
            Self::Internal(source) => tracing::error!(error = %source, "internal error"),
            Self::Unavailable(message) => tracing::warn!(%message, "service unavailable"),
            _ => {}
        }
        let body = ErrorBody {
            error: ErrorDetail {
                code: self.code().to_owned(),
                message: self.to_string(),
                current_version: match &self {
                    Self::VersionConflict { current_version, .. } => Some(*current_version),
                    _ => None,
                },
                issues: match &self {
                    Self::InvalidDocument { issues, .. } if !issues.is_empty() => Some(issues.clone()),
                    _ => None,
                },
            },
        };
        let mut response = (status, Json(body)).into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use serde_json::{Value, json};

    use super::*;

    async fn body(error: ApiError) -> (StatusCode, Option<HeaderValue>, Value) {
        let response = error.into_response();
        let status = response.status();
        let challenge = response.headers().get(WWW_AUTHENTICATE).cloned();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("body");
        (status, challenge, serde_json::from_slice(&bytes).expect("JSON body"))
    }

    #[tokio::test]
    async fn errors_have_a_stable_code_and_status() {
        let (status, challenge, json) = body(ApiError::Unauthorized("missing bearer token".to_owned())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(challenge, Some(HeaderValue::from_static("Bearer")));
        assert_eq!(
            json,
            json!({ "error": { "code": "UNAUTHORIZED", "message": "missing bearer token" } })
        );

        let (status, _, json) = body(ApiError::VersionConflict {
            base_version: 3,
            current_version: 5,
        })
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(json["error"]["code"], "VERSION_CONFLICT");
        assert_eq!(json["error"]["current_version"], 5);
    }

    #[tokio::test]
    async fn internal_errors_do_not_leak_details() {
        let (status, _, json) = body(ApiError::internal(std::io::Error::other(
            "connection string with password",
        )))
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            json,
            json!({ "error": { "code": "INTERNAL", "message": "internal error" } })
        );
    }
}
