//! API de Deep Atelier (ADR 0001 § 9 et § 16) : authentification par les jetons Supabase,
//! projets, sauvegarde automatique du document (concurrence optimiste), versions.
//!
//! L'API est le seul écrivain des tables : chaque document reçu est migré vers l'IR courante et
//! contrôlé par la crate `ir` avant d'être enregistré. Les utilisateurs n'ont, en base, qu'un
//! droit de lecture sur leurs propres lignes (RLS).

pub mod auth;
pub mod config;
pub mod db;
pub mod document;
pub mod error;
pub mod extract;
pub mod routes;
pub mod schema;

use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method, header};
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub use auth::{AuthUser, JwtVerifier};
pub use config::Config;
pub use error::ApiError;

/// Taille maximale d'une requête (un document complet).
pub const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub verifier: Arc<JwtVerifier>,
}

/// Application complète : routes, limite de taille, CORS, panique → 500, traces.
pub fn app(state: AppState, cors_origins: &[HeaderValue]) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(cors_origins.to_vec())
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .max_age(Duration::from_secs(600));
    routes::router()
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(cors)
        .layer(CatchPanicLayer::custom(panic_response))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn panic_response(_: Box<dyn Any + Send + 'static>) -> Response {
    ApiError::Internal("a request handler panicked".into()).into_response()
}
