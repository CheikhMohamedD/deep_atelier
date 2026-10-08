//! Authentification : jeton d'accès Supabase (`Authorization: Bearer …`) vérifié avec les clés
//! publiques du projet (JWKS : ES256 ou RS256). Les clés sont mises en cache, rechargées après
//! dix minutes ou quand un jeton cite une clé inconnue (rotation), au plus une fois par période
//! de refroidissement : une panne de Supabase ne bloque pas chaque requête.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use jsonwebtoken::jwk::{AlgorithmParameters, EllipticCurve, Jwk, JwkSet, KeyAlgorithm, PublicKeyUse};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::AppState;
use crate::error::ApiError;

/// Audience et rôle des jetons d'un utilisateur connecté.
const AUTHENTICATED: &str = "authenticated";
/// Âge maximal du cache de clés (Supabase met lui-même le JWKS en cache dix minutes).
const KEYS_MAX_AGE: Duration = Duration::from_secs(600);
/// Délai minimal entre deux tentatives de rechargement.
const REFRESH_COOLDOWN: Duration = Duration::from_secs(30);
/// Tolérance d'horloge sur l'expiration, en secondes.
const LEEWAY_SECONDS: u64 = 30;

/// Utilisateur authentifié par son jeton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("missing bearer token")]
    MissingToken,
    #[error("invalid token: {0}")]
    InvalidToken(String),
    #[error("unknown signing key `{0}`")]
    UnknownKey(String),
    #[error("the token does not belong to a signed-in user")]
    NotAUser,
    #[error("signing keys unavailable: {0}")]
    KeysUnavailable(String),
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::KeysUnavailable(detail) => {
                tracing::warn!(%detail, "signing keys unavailable");
                ApiError::Unavailable("authentication keys are unavailable".to_owned())
            }
            other => ApiError::Unauthorized(other.to_string()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    role: String,
    #[serde(default)]
    email: Option<String>,
}

enum KeySource {
    Remote { url: String, http: reqwest::Client },
    Fixed,
}

#[derive(Default)]
struct KeyCache {
    keys: HashMap<String, (Algorithm, DecodingKey)>,
    /// Dernier chargement réussi.
    fetched_at: Option<Instant>,
    /// Dernière tentative, réussie ou non.
    attempted_at: Option<Instant>,
}

impl KeyCache {
    fn fresh_key(&self, kid: &str, fixed: bool) -> Option<(Algorithm, DecodingKey)> {
        let fresh = fixed || self.fetched_at.is_some_and(|at| at.elapsed() < KEYS_MAX_AGE);
        self.keys.get(kid).filter(|_| fresh).cloned()
    }
}

/// Vérificateur des jetons d'accès.
pub struct JwtVerifier {
    issuer: String,
    source: KeySource,
    cache: RwLock<KeyCache>,
}

impl JwtVerifier {
    /// Clés lues dans le JWKS du projet Supabase.
    pub fn remote(issuer: impl Into<String>, jwks_url: impl Into<String>, http: reqwest::Client) -> Self {
        Self {
            issuer: issuer.into(),
            source: KeySource::Remote {
                url: jwks_url.into(),
                http,
            },
            cache: RwLock::new(KeyCache::default()),
        }
    }

    /// Clés fixes, jamais rechargées (tests).
    pub fn fixed(issuer: impl Into<String>, jwks: &JwkSet) -> Self {
        let now = Instant::now();
        Self {
            issuer: issuer.into(),
            source: KeySource::Fixed,
            cache: RwLock::new(KeyCache {
                keys: decoding_keys(jwks),
                fetched_at: Some(now),
                attempted_at: Some(now),
            }),
        }
    }

    /// Vérifie la signature, l'émetteur, l'audience et l'expiration d'un jeton d'accès.
    pub async fn verify(&self, token: &str) -> Result<AuthUser, AuthError> {
        let header = decode_header(token).map_err(|e| AuthError::InvalidToken(e.to_string()))?;
        if !matches!(header.alg, Algorithm::ES256 | Algorithm::RS256) {
            return Err(AuthError::InvalidToken(format!(
                "algorithm {:?} is not accepted",
                header.alg
            )));
        }
        let kid = header
            .kid
            .ok_or_else(|| AuthError::InvalidToken("missing key id".to_owned()))?;
        let (algorithm, key) = self.key(&kid).await?;
        if algorithm != header.alg {
            return Err(AuthError::InvalidToken(
                "algorithm does not match the signing key".to_owned(),
            ));
        }
        let mut validation = Validation::new(algorithm);
        validation.set_audience(&[AUTHENTICATED]);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["exp", "aud", "iss", "sub"]);
        validation.leeway = LEEWAY_SECONDS;
        let claims = decode::<Claims>(token, &key, &validation)
            .map_err(|e| AuthError::InvalidToken(e.to_string()))?
            .claims;
        if claims.role != AUTHENTICATED {
            return Err(AuthError::NotAUser);
        }
        let id = Uuid::parse_str(&claims.sub).map_err(|_| AuthError::NotAUser)?;
        Ok(AuthUser {
            id,
            email: claims.email,
        })
    }

    async fn key(&self, kid: &str) -> Result<(Algorithm, DecodingKey), AuthError> {
        let fixed = matches!(self.source, KeySource::Fixed);
        if let Some(key) = self.cache.read().await.fresh_key(kid, fixed) {
            return Ok(key);
        }
        let KeySource::Remote { url, http } = &self.source else {
            return Err(AuthError::UnknownKey(kid.to_owned()));
        };
        let mut cache = self.cache.write().await;
        // Une autre requête a pu recharger les clés pendant l'attente du verrou.
        if let Some(key) = cache.fresh_key(kid, false) {
            return Ok(key);
        }
        let cooling = cache.attempted_at.is_some_and(|at| at.elapsed() < REFRESH_COOLDOWN);
        if !cooling {
            cache.attempted_at = Some(Instant::now());
            match fetch_jwks(http, url).await {
                Ok(jwks) => {
                    cache.keys = decoding_keys(&jwks);
                    cache.fetched_at = cache.attempted_at;
                }
                // Clé déjà connue : mieux vaut un cache périmé qu'un refus de tous les jetons.
                Err(error) if cache.keys.contains_key(kid) => {
                    tracing::warn!(%error, "could not refresh the signing keys, using cached keys");
                }
                Err(error) => return Err(error),
            }
        } else if cache.fetched_at.is_none() {
            return Err(AuthError::KeysUnavailable("signing keys were never loaded".to_owned()));
        }
        cache
            .keys
            .get(kid)
            .cloned()
            .ok_or_else(|| AuthError::UnknownKey(kid.to_owned()))
    }
}

async fn fetch_jwks(http: &reqwest::Client, url: &str) -> Result<JwkSet, AuthError> {
    let unavailable = |e: reqwest::Error| AuthError::KeysUnavailable(e.to_string());
    http.get(url)
        .send()
        .await
        .map_err(unavailable)?
        .error_for_status()
        .map_err(unavailable)?
        .json::<JwkSet>()
        .await
        .map_err(unavailable)
}

/// Clés de signature utilisables d'un JWKS, par identifiant.
fn decoding_keys(jwks: &JwkSet) -> HashMap<String, (Algorithm, DecodingKey)> {
    jwks.keys
        .iter()
        .filter(|jwk| matches!(jwk.common.public_key_use, None | Some(PublicKeyUse::Signature)))
        .filter_map(|jwk| {
            let kid = jwk.common.key_id.clone()?;
            let algorithm = key_algorithm(jwk)?;
            match DecodingKey::from_jwk(jwk) {
                Ok(key) => Some((kid, (algorithm, key))),
                Err(error) => {
                    tracing::warn!(%kid, %error, "ignoring an unusable signing key");
                    None
                }
            }
        })
        .collect()
}

/// Algorithme d'une clé : celui qu'elle annonce, sinon déduit de son type (EC P-256 → ES256,
/// RSA → RS256). Les autres algorithmes ne sont pas acceptés.
fn key_algorithm(jwk: &Jwk) -> Option<Algorithm> {
    match jwk.common.key_algorithm {
        Some(KeyAlgorithm::ES256) => Some(Algorithm::ES256),
        Some(KeyAlgorithm::RS256) => Some(Algorithm::RS256),
        Some(_) => None,
        None => match &jwk.algorithm {
            AlgorithmParameters::EllipticCurve(ec) if ec.curve == EllipticCurve::P256 => Some(Algorithm::ES256),
            AlgorithmParameters::RSA(_) => Some(Algorithm::RS256),
            _ => None,
        },
    }
}

/// Jeton d'un en-tête `Authorization: Bearer <jeton>`.
fn bearer_token(parts: &Parts) -> Option<&str> {
    let value = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts).ok_or(AuthError::MissingToken)?;
        Ok(state.verifier.verify(token).await?)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::routing::get;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use p256::ecdsa::SigningKey;
    use p256::pkcs8::EncodePrivateKey;
    use serde_json::{Value, json};

    use super::*;

    pub(crate) const ISSUER: &str = "http://127.0.0.1:54321/auth/v1";

    /// Clé de signature ES256 de test et son JWKS.
    pub(crate) struct TestKey {
        pub kid: String,
        encoding: EncodingKey,
        pub jwks: JwkSet,
    }

    impl TestKey {
        pub(crate) fn new(kid: &str) -> Self {
            let signing = SigningKey::random(&mut rand_core::OsRng);
            let der = signing.to_pkcs8_der().expect("PKCS#8 encoding");
            let point = signing.verifying_key().to_encoded_point(false);
            let coordinate = |c: Option<&p256::FieldBytes>| URL_SAFE_NO_PAD.encode(c.expect("uncompressed point"));
            let jwks: JwkSet = serde_json::from_value(json!({ "keys": [{
                "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig", "kid": kid,
                "key_ops": ["verify"], "ext": true,
                "x": coordinate(point.x()), "y": coordinate(point.y())
            }] }))
            .expect("valid JWKS");
            Self {
                kid: kid.to_owned(),
                encoding: EncodingKey::from_ec_der(der.as_bytes()),
                jwks,
            }
        }

        pub(crate) fn sign(&self, claims: &Value) -> String {
            let mut header = Header::new(Algorithm::ES256);
            header.kid = Some(self.kid.clone());
            encode(&header, claims, &self.encoding).expect("signed token")
        }

        /// Jeton d'accès valide d'un utilisateur connecté.
        pub(crate) fn user_token(&self, id: Uuid) -> String {
            self.sign(&claims(id))
        }
    }

    pub(crate) fn claims(id: Uuid) -> Value {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        json!({
            "iss": ISSUER, "aud": AUTHENTICATED, "role": AUTHENTICATED, "sub": id.to_string(),
            "email": "ada@example.com", "iat": now, "exp": now + 3600
        })
    }

    #[tokio::test]
    async fn accepts_a_signed_in_user_token() {
        let key = TestKey::new("k1");
        let verifier = JwtVerifier::fixed(ISSUER, &key.jwks);
        let id = Uuid::new_v4();
        let user = verifier.verify(&key.user_token(id)).await.expect("valid token");
        assert_eq!(
            user,
            AuthUser {
                id,
                email: Some("ada@example.com".to_owned())
            }
        );
    }

    #[tokio::test]
    async fn rejects_tokens_that_do_not_match_the_project() {
        let key = TestKey::new("k1");
        let verifier = JwtVerifier::fixed(ISSUER, &key.jwks);
        let id = Uuid::new_v4();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();

        let mut expired = claims(id);
        expired["exp"] = json!(now - 120);
        let mut other_issuer = claims(id);
        other_issuer["iss"] = json!("https://other.supabase.co/auth/v1");
        let mut other_audience = claims(id);
        other_audience["aud"] = json!("anon");
        let mut anon = claims(id);
        anon["role"] = json!("anon");
        let mut not_a_user = claims(id);
        not_a_user["sub"] = json!("service");
        let mut no_expiry = claims(id);
        no_expiry.as_object_mut().expect("object").remove("exp");

        for (case, token) in [
            ("expired", key.sign(&expired)),
            ("issuer", key.sign(&other_issuer)),
            ("audience", key.sign(&other_audience)),
            ("no expiry", key.sign(&no_expiry)),
        ] {
            assert!(
                matches!(verifier.verify(&token).await, Err(AuthError::InvalidToken(_))),
                "{case}"
            );
        }
        for token in [key.sign(&anon), key.sign(&not_a_user)] {
            assert!(matches!(verifier.verify(&token).await, Err(AuthError::NotAUser)));
        }
    }

    #[tokio::test]
    async fn rejects_forged_tokens() {
        let key = TestKey::new("k1");
        let verifier = JwtVerifier::fixed(ISSUER, &key.jwks);
        let token = key.user_token(Uuid::new_v4());

        // Signature d'une autre clé sous le même identifiant.
        let impostor = TestKey::new("k1");
        assert!(matches!(
            verifier.verify(&impostor.user_token(Uuid::new_v4())).await,
            Err(AuthError::InvalidToken(_))
        ));
        // Clé inconnue.
        let stranger = TestKey::new("k2");
        assert!(matches!(
            verifier.verify(&stranger.user_token(Uuid::new_v4())).await,
            Err(AuthError::UnknownKey(kid)) if kid == "k2"
        ));
        // Charge utile modifiée.
        let mut parts: Vec<String> = token.split('.').map(str::to_owned).collect();
        let mut payload: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(&parts[1]).expect("base64")).expect("JSON");
        payload["sub"] = json!(Uuid::new_v4().to_string());
        parts[1] = URL_SAFE_NO_PAD.encode(payload.to_string());
        assert!(matches!(
            verifier.verify(&parts.join(".")).await,
            Err(AuthError::InvalidToken(_))
        ));
        // Secret partagé (HS256) : jamais accepté, même avec un identifiant de clé connu.
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some("k1".to_owned());
        let hs256 = encode(&header, &claims(Uuid::new_v4()), &EncodingKey::from_secret(b"secret")).expect("token");
        assert!(matches!(verifier.verify(&hs256).await, Err(AuthError::InvalidToken(_))));
        assert!(matches!(
            verifier.verify("not.a.token").await,
            Err(AuthError::InvalidToken(_))
        ));
    }

    /// Serveur JWKS local qui compte ses requêtes ; `None` répond 500.
    async fn jwks_server(jwks: Option<JwkSet>) -> (String, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let app = Router::new().route(
            "/jwks.json",
            get(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                let jwks = jwks.clone();
                async move {
                    match jwks {
                        Some(jwks) => Ok(axum::Json(jwks)),
                        None => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let url = format!("http://{}/jwks.json", listener.local_addr().expect("address"));
        tokio::spawn(async move { axum::serve(listener, app).await.expect("server") });
        (url, hits)
    }

    #[tokio::test]
    async fn remote_keys_are_fetched_once_and_cached() {
        let key = TestKey::new("k1");
        let (url, hits) = jwks_server(Some(key.jwks.clone())).await;
        let verifier = JwtVerifier::remote(ISSUER, url, reqwest::Client::new());
        for _ in 0..3 {
            verifier
                .verify(&key.user_token(Uuid::new_v4()))
                .await
                .expect("valid token");
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        // Une clé inconnue juste après un chargement ne relance pas de requête (refroidissement).
        let stranger = TestKey::new("k2");
        assert!(matches!(
            verifier.verify(&stranger.user_token(Uuid::new_v4())).await,
            Err(AuthError::UnknownKey(_))
        ));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unreachable_keys_make_the_service_unavailable() {
        let key = TestKey::new("k1");
        let (url, hits) = jwks_server(None).await;
        let verifier = JwtVerifier::remote(ISSUER, url, reqwest::Client::new());
        let token = key.user_token(Uuid::new_v4());
        assert!(matches!(
            verifier.verify(&token).await,
            Err(AuthError::KeysUnavailable(_))
        ));
        // Pendant le refroidissement, aucune nouvelle requête, toujours indisponible.
        assert!(matches!(
            verifier.verify(&token).await,
            Err(AuthError::KeysUnavailable(_))
        ));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(
            ApiError::from(AuthError::KeysUnavailable("down".to_owned())).status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn bearer_tokens_are_read_from_the_authorization_header() {
        let parts = |value: &str| {
            let request = axum::http::Request::builder()
                .header(AUTHORIZATION, value)
                .body(())
                .expect("request");
            request.into_parts().0
        };
        assert_eq!(bearer_token(&parts("Bearer abc.def.ghi")), Some("abc.def.ghi"));
        assert_eq!(bearer_token(&parts("bearer  abc")), Some("abc"));
        assert_eq!(bearer_token(&parts("Basic abc")), None);
        assert_eq!(bearer_token(&parts("Bearer ")), None);
    }
}
