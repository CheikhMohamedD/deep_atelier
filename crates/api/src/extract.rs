//! Extracteurs dont les refus suivent le format d'erreur de l'API.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, FromRequestParts, Path, Request};
use axum::http::StatusCode;
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// Corps JSON ; un corps invalide donne `INVALID_REQUEST`, un corps trop gros `PAYLOAD_TOO_LARGE`.
pub struct Body<T>(pub T);

impl<S, T> FromRequest<S> for Body<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => Err(ApiError::PayloadTooLarge),
            Err(rejection) => Err(ApiError::InvalidRequest(json_rejection_message(&rejection))),
        }
    }
}

fn json_rejection_message(rejection: &JsonRejection) -> String {
    match rejection {
        JsonRejection::MissingJsonContentType(_) => "expected a JSON body (Content-Type: application/json)".to_owned(),
        other => other.body_text(),
    }
}

/// Paramètres de chemin ; un paramètre mal formé (identifiant invalide) désigne une ressource
/// absente.
pub struct ApiPath<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiPath<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(value)| Self(value))
            .map_err(|_| ApiError::NotFound)
    }
}
