//! Tests d'intégration contre le stack Supabase local, ignorés par défaut :
//!
//! ```sh
//! supabase start
//! set -a; eval "$(supabase status -o env)"; set +a
//! cargo test -p deep-atelier-api -- --include-ignored
//! ```
//!
//! Les jetons viennent du vrai service d'authentification (inscription par e-mail et mot de passe,
//! permise sans confirmation en local) ; l'API vérifie leur signature avec le JWKS local, comme
//! en production. Les droits directs (PostgREST + RLS) sont vérifiés avec les mêmes jetons.

use std::sync::Arc;

use api::{AppState, JwtVerifier};
use axum::Router;
use axum::body::Body;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, WWW_AUTHENTICATE};
use axum::http::{HeaderValue, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

const NEEDS_STACK: &str = "stack Supabase local requis (voir l'en-tête du fichier)";

struct Stack {
    api_url: String,
    key: String,
    http: reqwest::Client,
    app: Router,
}

struct User {
    id: Uuid,
    token: String,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

async fn stack() -> Stack {
    let api_url = var("API_URL").unwrap_or_else(|| "http://127.0.0.1:54321".to_owned());
    let db_url = var("DB_URL").unwrap_or_else(|| "postgresql://postgres:postgres@127.0.0.1:54322/postgres".to_owned());
    let key = var("PUBLISHABLE_KEY")
        .or_else(|| var("ANON_KEY"))
        .unwrap_or_else(|| panic!("PUBLISHABLE_KEY absent : {NEEDS_STACK}"));
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .unwrap_or_else(|e| panic!("base locale injoignable ({e}) : {NEEDS_STACK}"));
    let http = reqwest::Client::new();
    let verifier = JwtVerifier::remote(
        format!("{api_url}/auth/v1"),
        format!("{api_url}/auth/v1/.well-known/jwks.json"),
        http.clone(),
    );
    let state = AppState {
        db,
        verifier: Arc::new(verifier),
    };
    let app = api::app(state, &[HeaderValue::from_static("http://localhost:3000")]);
    Stack {
        api_url,
        key,
        http,
        app,
    }
}

impl Stack {
    /// Nouvel utilisateur inscrit et connecté.
    async fn user(&self) -> User {
        let response: Value = self
            .http
            .post(format!("{}/auth/v1/signup", self.api_url))
            .header("apikey", &self.key)
            .json(&json!({
                "email": format!("api-{}@example.com", Uuid::new_v4().simple()),
                "password": Uuid::new_v4().to_string()
            }))
            .send()
            .await
            .expect("signup request")
            .error_for_status()
            .expect("signup accepted")
            .json()
            .await
            .expect("signup response");
        User {
            id: response["user"]["id"].as_str().expect("user id").parse().expect("uuid"),
            token: response["access_token"].as_str().expect("access token").to_owned(),
        }
    }

    async fn call(&self, method: Method, uri: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        self.call_raw(method, uri, token, body.map(|b| b.to_string())).await
    }

    async fn call_raw(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<String>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = match body {
            Some(body) => request.header(CONTENT_TYPE, "application/json").body(Body::from(body)),
            None => request.body(Body::empty()),
        }
        .expect("request");
        let response = self.app.clone().oneshot(request).await.expect("response");
        let status = response.status();
        let bytes = response.into_body().collect().await.expect("body").to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("JSON body")
        };
        (status, value)
    }

    /// Crée un projet ; renvoie son id et son document.
    async fn project(&self, user: &User, name: &str) -> (Uuid, Value) {
        let (status, body) = self
            .call(
                Method::POST,
                "/projects",
                Some(&user.token),
                Some(json!({ "name": name })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let id = body["project"]["id"].as_str().expect("id").parse().expect("uuid");
        (id, body["document"]["document"].clone())
    }

    async fn save(&self, user: &User, project: Uuid, base_version: i64, document: &Value) -> (StatusCode, Value) {
        self.call(
            Method::PUT,
            &format!("/projects/{project}/document"),
            Some(&user.token),
            Some(json!({ "base_version": base_version, "document": document })),
        )
        .await
    }

    /// Requête PostgREST directe avec le jeton de l'utilisateur (droits de la base, RLS).
    async fn rest(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = self
            .http
            .request(method, format!("{}/rest/v1/{path}", self.api_url))
            .header("apikey", &self.key);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.expect("PostgREST request");
        let status = StatusCode::from_u16(response.status().as_u16()).expect("status");
        let text = response.text().await.expect("PostgREST body");
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }
}

fn with_name(document: &Value, name: &str) -> Value {
    let mut document = document.clone();
    document["name"] = json!(name);
    document
}

#[tokio::test]
#[ignore = "stack Supabase local requis"]
async fn requests_without_a_signed_in_user_are_refused() {
    let stack = stack().await;
    let (status, body) = stack.call(Method::GET, "/health", None, None).await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "status": "ok" })));

    let request = Request::builder()
        .uri("/projects")
        .body(Body::empty())
        .expect("request");
    let response = stack.app.clone().oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.headers().get(WWW_AUTHENTICATE),
        Some(&HeaderValue::from_static("Bearer"))
    );

    let mut refused = vec!["not-a-token".to_owned(), stack.key.clone()];
    // L'ancienne clé `anon` est un JWT signé par le projet, mais pas celui d'un utilisateur.
    refused.extend(var("ANON_KEY"));
    for token in refused {
        let (status, body) = stack.call(Method::GET, "/projects", Some(&token), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert_eq!(body["error"]["code"], "UNAUTHORIZED");
    }

    let user = stack.user().await;
    let (status, body) = stack.call(Method::GET, "/nowhere", Some(&user.token), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
#[ignore = "stack Supabase local requis"]
async fn projects_belong_to_their_owner() {
    let stack = stack().await;
    let ada = stack.user().await;
    let grace = stack.user().await;

    let (status, body) = stack
        .call(
            Method::POST,
            "/projects",
            Some(&ada.token),
            Some(json!({ "name": "  Site d'Ada " })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["project"]["name"], "Site d'Ada");
    assert_eq!(body["project"]["document_version"], 1);
    assert_eq!(body["document"]["version"], 1);
    assert_eq!(body["document"]["document"]["name"], "Site d'Ada");
    assert_eq!(body["document"]["document"]["pages"].as_array().map(Vec::len), Some(1));
    let id = body["project"]["id"].as_str().expect("id").to_owned();

    let (_, mine) = stack.call(Method::GET, "/projects", Some(&ada.token), None).await;
    assert!(mine["projects"].as_array().expect("list").iter().any(|p| p["id"] == id));
    let (_, theirs) = stack.call(Method::GET, "/projects", Some(&grace.token), None).await;
    assert!(
        !theirs["projects"]
            .as_array()
            .expect("list")
            .iter()
            .any(|p| p["id"] == id)
    );

    // Le projet d'Ada n'existe pas pour Grace, quelle que soit l'opération.
    let document = stack
        .call(Method::GET, &format!("/projects/{id}/document"), Some(&ada.token), None)
        .await
        .1;
    for (method, path, body) in [
        (Method::GET, format!("/projects/{id}"), None),
        (
            Method::PATCH,
            format!("/projects/{id}"),
            Some(json!({ "name": "Volé" })),
        ),
        (Method::DELETE, format!("/projects/{id}"), None),
        (Method::GET, format!("/projects/{id}/document"), None),
        (
            Method::PUT,
            format!("/projects/{id}/document"),
            Some(json!({ "base_version": 1, "document": document["document"] })),
        ),
        (Method::GET, format!("/projects/{id}/versions"), None),
        (
            Method::POST,
            format!("/projects/{id}/versions"),
            Some(json!({ "message": "Volé" })),
        ),
        (Method::GET, format!("/projects/{id}/versions/1"), None),
        (
            Method::POST,
            format!("/projects/{id}/versions/1/restore"),
            Some(json!({ "base_version": 1 })),
        ),
    ] {
        let (status, body) = stack.call(method.clone(), &path, Some(&grace.token), body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {body}");
    }

    let (status, body) = stack
        .call(
            Method::PATCH,
            &format!("/projects/{id}"),
            Some(&ada.token),
            Some(json!({ "name": "Atelier d'Ada" })),
        )
        .await;
    assert_eq!(
        (status, &body["project"]["name"]),
        (StatusCode::OK, &json!("Atelier d'Ada"))
    );

    for body in [
        json!({ "name": "   " }),
        json!({ "name": "x".repeat(121) }),
        json!({ "name": "A", "owner_id": grace.id }),
    ] {
        let (status, error) = stack
            .call(Method::POST, "/projects", Some(&ada.token), Some(body))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
        assert_eq!(error["error"]["code"], "INVALID_REQUEST");
    }
    let (status, _) = stack
        .call(Method::GET, "/projects/not-a-uuid", Some(&ada.token), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = stack
        .call(Method::DELETE, &format!("/projects/{id}"), Some(&ada.token), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = stack
        .call(Method::GET, &format!("/projects/{id}"), Some(&ada.token), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_ne!(ada.id, grace.id);
}

#[tokio::test]
#[ignore = "stack Supabase local requis"]
async fn autosave_uses_optimistic_concurrency() {
    let stack = stack().await;
    let user = stack.user().await;
    let (id, document) = stack.project(&user, "Concurrence").await;

    let (status, saved) = stack.save(&user, id, 1, &with_name(&document, "Deuxième état")).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["version"], 2);

    // Une sauvegarde partie de la version 1 arrive trop tard.
    let (status, conflict) = stack.save(&user, id, 1, &with_name(&document, "Perdu")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(conflict["error"]["code"], "VERSION_CONFLICT");
    assert_eq!(conflict["error"]["current_version"], 2);

    let (_, current) = stack
        .call(
            Method::GET,
            &format!("/projects/{id}/document"),
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(current["version"], 2);
    assert_eq!(current["document"]["name"], "Deuxième état");
    let (_, project) = stack
        .call(Method::GET, &format!("/projects/{id}"), Some(&user.token), None)
        .await;
    assert_eq!(project["project"]["document_version"], 2);

    // Deux sauvegardes simultanées depuis la même version : une seule passe.
    let first = with_name(&document, "Onglet A");
    let second = with_name(&document, "Onglet B");
    let (a, b) = tokio::join!(stack.save(&user, id, 2, &first), stack.save(&user, id, 2, &second));
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CONFLICT]);
    let (_, current) = stack
        .call(
            Method::GET,
            &format!("/projects/{id}/document"),
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(current["version"], 3);
}

#[tokio::test]
#[ignore = "stack Supabase local requis"]
async fn damaged_documents_are_refused_but_unfinished_ones_are_saved() {
    let stack = stack().await;
    let user = stack.user().await;
    let (id, document) = stack.project(&user, "Contrôle").await;

    let mut dangling = document.clone();
    let root = dangling["pages"][0]["root"].as_str().expect("root").to_owned();
    dangling["nodes"][&root]["children"] = json!(["n_zzzzzzzzzz"]);
    let (status, error) = stack.save(&user, id, 1, &dangling).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error"]["code"], "INVALID_DOCUMENT");
    assert!(
        error["error"]["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .any(|i| i["code"] == "TREE_INCONSISTENT")
    );

    let mut too_new = document.clone();
    too_new["version"] = json!(99);
    let (status, error) = stack.save(&user, id, 1, &too_new).await;
    assert_eq!(
        (status, &error["error"]["code"]),
        (StatusCode::UNPROCESSABLE_ENTITY, &json!("INVALID_DOCUMENT"))
    );

    let (status, error) = stack
        .call(
            Method::PUT,
            &format!("/projects/{id}/document"),
            Some(&user.token),
            Some(json!({ "document": document })),
        )
        .await;
    assert_eq!(
        (status, &error["error"]["code"]),
        (StatusCode::UNPROCESSABLE_ENTITY, &json!("INVALID_REQUEST"))
    );

    let huge = format!(
        "{{\"base_version\":1,\"document\":{{\"name\":\"{}\"}}}}",
        "x".repeat(api::MAX_BODY_BYTES)
    );
    let (status, error) = stack
        .call_raw(
            Method::PUT,
            &format!("/projects/{id}/document"),
            Some(&user.token),
            Some(huge),
        )
        .await;
    assert_eq!(
        (status, &error["error"]["code"]),
        (StatusCode::PAYLOAD_TOO_LARGE, &json!("PAYLOAD_TOO_LARGE"))
    );

    // Rien n'a été enregistré.
    let (_, current) = stack
        .call(
            Method::GET,
            &format!("/projects/{id}/document"),
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(current["version"], 1);

    // Un document inachevé (langue vide : problème d'accessibilité) est enregistré.
    let mut unfinished = document.clone();
    unfinished["settings"]["lang"] = json!("");
    let (status, saved) = stack.save(&user, id, 1, &unfinished).await;
    assert_eq!((status, &saved["version"]), (StatusCode::OK, &json!(2)));
}

#[tokio::test]
#[ignore = "stack Supabase local requis"]
async fn versions_keep_and_restore_documents() {
    let stack = stack().await;
    let user = stack.user().await;
    let (id, document) = stack.project(&user, "Versions").await;
    let versions = format!("/projects/{id}/versions");

    assert_eq!(
        stack.save(&user, id, 1, &with_name(&document, "État A")).await.0,
        StatusCode::OK
    );
    let (status, first) = stack
        .call(
            Method::POST,
            &versions,
            Some(&user.token),
            Some(json!({ "message": "Première maquette", "name": " A " })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    assert_eq!(first["version"]["number"], 1);
    assert_eq!(first["version"]["name"], "A");
    assert_eq!(first["version"]["origin"], "user");
    assert_eq!(first["version"]["document_version"], 2);
    assert_eq!(first["version"]["created_by"], json!(user.id));

    assert_eq!(
        stack.save(&user, id, 2, &with_name(&document, "État B")).await.0,
        StatusCode::OK
    );
    let (status, second) = stack
        .call(
            Method::POST,
            &versions,
            Some(&user.token),
            Some(json!({ "message": "Ajustements" })),
        )
        .await;
    assert_eq!((status, &second["version"]["number"]), (StatusCode::CREATED, &json!(2)));
    assert_eq!(second["version"]["name"], Value::Null);

    let (_, list) = stack.call(Method::GET, &versions, Some(&user.token), None).await;
    let numbers: Vec<&Value> = list["versions"]
        .as_array()
        .expect("versions")
        .iter()
        .map(|v| &v["number"])
        .collect();
    assert_eq!(numbers, [&json!(2), &json!(1)]);
    assert!(list["versions"][0].get("document").is_none());

    let (_, one) = stack
        .call(Method::GET, &format!("{versions}/1"), Some(&user.token), None)
        .await;
    assert_eq!(one["version"]["document"]["name"], "État A");

    // Restauration depuis un état périmé : refusée.
    let (status, conflict) = stack
        .call(
            Method::POST,
            &format!("{versions}/1/restore"),
            Some(&user.token),
            Some(json!({ "base_version": 2 })),
        )
        .await;
    assert_eq!(
        (status, &conflict["error"]["current_version"]),
        (StatusCode::CONFLICT, &json!(3))
    );

    let (status, restored) = stack
        .call(
            Method::POST,
            &format!("{versions}/1/restore"),
            Some(&user.token),
            Some(json!({ "base_version": 3 })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["backup"]["number"], 3);
    assert_eq!(restored["backup"]["origin"], "restore");
    assert_eq!(restored["backup"]["restored_from"], 1);
    assert_eq!(restored["backup"]["message"], Value::Null);
    assert_eq!(restored["document"]["version"], 4);
    assert_eq!(restored["document"]["document"]["name"], "État A");
    // L'état d'avant la restauration reste disponible.
    let (_, backup) = stack
        .call(Method::GET, &format!("{versions}/3"), Some(&user.token), None)
        .await;
    assert_eq!(backup["version"]["document"]["name"], "État B");

    let (status, _) = stack
        .call(
            Method::POST,
            &format!("{versions}/99/restore"),
            Some(&user.token),
            Some(json!({ "base_version": 4 })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, error) = stack
        .call(
            Method::POST,
            &versions,
            Some(&user.token),
            Some(json!({ "message": " " })),
        )
        .await;
    assert_eq!(
        (status, &error["error"]["code"]),
        (StatusCode::UNPROCESSABLE_ENTITY, &json!("INVALID_REQUEST"))
    );
}

#[tokio::test]
#[ignore = "stack Supabase local requis"]
async fn the_database_lets_users_read_only_their_own_rows() {
    let stack = stack().await;
    let ada = stack.user().await;
    let grace = stack.user().await;
    let (id, _) = stack.project(&ada, "Lecture seule").await;
    let filter = format!("projects?select=id,name&id=eq.{id}");

    let (status, rows) = stack.rest(reqwest::Method::GET, &filter, Some(&ada.token), None).await;
    assert_eq!((status, rows.as_array().map(Vec::len)), (StatusCode::OK, Some(1)));
    let (status, rows) = stack
        .rest(reqwest::Method::GET, &filter, Some(&grace.token), None)
        .await;
    assert_eq!((status, rows.as_array().map(Vec::len)), (StatusCode::OK, Some(0)));
    let (status, rows) = stack
        .rest(
            reqwest::Method::GET,
            &format!("project_documents?select=version&project_id=eq.{id}"),
            Some(&ada.token),
            None,
        )
        .await;
    assert_eq!((status, rows.as_array().map(Vec::len)), (StatusCode::OK, Some(1)));

    // Aucune écriture directe, même sur ses propres lignes ; aucun accès anonyme.
    let attempts = [
        (
            reqwest::Method::POST,
            "projects".to_owned(),
            Some(json!({ "owner_id": ada.id, "name": "Direct" })),
        ),
        (
            reqwest::Method::PATCH,
            format!("projects?id=eq.{id}"),
            Some(json!({ "name": "Direct" })),
        ),
        (reqwest::Method::DELETE, format!("projects?id=eq.{id}"), None),
        (
            reqwest::Method::PATCH,
            format!("project_documents?project_id=eq.{id}"),
            Some(json!({ "version": 99 })),
        ),
    ];
    for (method, path, body) in attempts {
        let (status, error) = stack.rest(method.clone(), &path, Some(&ada.token), body).await;
        assert!(
            matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
            "{method} {path}: {status} {error}"
        );
    }
    let (status, _) = stack.rest(reqwest::Method::GET, "projects?select=id", None, None).await;
    assert!(
        matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
        "{status}"
    );

    let (_, project) = stack
        .call(Method::GET, &format!("/projects/{id}"), Some(&ada.token), None)
        .await;
    assert_eq!(project["project"]["name"], "Lecture seule");
    assert_eq!(project["project"]["document_version"], 1);
}
