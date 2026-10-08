//! Configuration lue dans l'environnement (un fichier `.env` est accepté en développement).

use std::net::SocketAddr;

use axum::http::HeaderValue;

/// Adresse d'écoute par défaut (l'éditeur tourne sur le port 3000).
const DEFAULT_BIND: &str = "127.0.0.1:3001";
const DEFAULT_CORS_ORIGINS: &str = "http://localhost:3000";
const DEFAULT_MAX_CONNECTIONS: u32 = 5;

#[derive(Debug, Clone)]
pub struct Config {
    /// Connexion Postgres avec le rôle propriétaire des tables (`postgres` sur Supabase).
    pub database_url: String,
    /// URL du projet Supabase (`https://<ref>.supabase.co`, ou `http://127.0.0.1:54321` en local).
    pub supabase_url: String,
    /// Émetteur attendu des jetons (`<supabase_url>/auth/v1` par défaut).
    pub jwt_issuer: String,
    pub bind: SocketAddr,
    /// Origines autorisées à appeler l'API depuis un navigateur (l'éditeur).
    pub cors_origins: Vec<HeaderValue>,
    pub database_max_connections: u32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing environment variable {0}")]
    Missing(&'static str),
    #[error("invalid environment variable {name}: {message}")]
    Invalid { name: &'static str, message: String },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Configuration à partir d'une fonction de lecture des variables (testable).
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let value = |name: &str| lookup(name).map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        let invalid = |name: &'static str, message: String| ConfigError::Invalid { name, message };

        let database_url = value("DATABASE_URL").ok_or(ConfigError::Missing("DATABASE_URL"))?;
        let supabase_url = value("SUPABASE_URL")
            .ok_or(ConfigError::Missing("SUPABASE_URL"))?
            .trim_end_matches('/')
            .to_owned();
        if !(supabase_url.starts_with("https://") || supabase_url.starts_with("http://")) {
            return Err(invalid("SUPABASE_URL", "expected an http(s) URL".to_owned()));
        }
        let jwt_issuer = value("SUPABASE_JWT_ISSUER").unwrap_or_else(|| format!("{supabase_url}/auth/v1"));
        let bind = value("API_BIND")
            .unwrap_or_else(|| DEFAULT_BIND.to_owned())
            .parse()
            .map_err(|e| invalid("API_BIND", format!("{e}")))?;
        let cors_origins = value("CORS_ORIGINS")
            .unwrap_or_else(|| DEFAULT_CORS_ORIGINS.to_owned())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(|origin| {
                HeaderValue::from_str(origin.trim_end_matches('/'))
                    .map_err(|_| invalid("CORS_ORIGINS", format!("`{origin}` is not a valid origin")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let database_max_connections = match value("DATABASE_MAX_CONNECTIONS") {
            Some(raw) => raw
                .parse()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| invalid("DATABASE_MAX_CONNECTIONS", "expected a positive integer".to_owned()))?,
            None => DEFAULT_MAX_CONNECTIONS,
        };
        Ok(Self {
            database_url,
            supabase_url,
            jwt_issuer,
            bind,
            cors_origins,
            database_max_connections,
        })
    }

    /// Clés publiques de signature des jetons (JWKS) du projet Supabase.
    pub fn jwks_url(&self) -> String {
        format!("{}/auth/v1/.well-known/jwks.json", self.supabase_url)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let vars: HashMap<String, String> = vars.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        Config::from_lookup(|name| vars.get(name).cloned())
    }

    const REQUIRED: [(&str, &str); 2] = [
        (
            "DATABASE_URL",
            "postgresql://postgres:postgres@127.0.0.1:54322/postgres",
        ),
        ("SUPABASE_URL", "http://127.0.0.1:54321/"),
    ];

    #[test]
    fn defaults_follow_the_supabase_url() {
        let config = config(&REQUIRED).expect("valid configuration");
        assert_eq!(config.supabase_url, "http://127.0.0.1:54321");
        assert_eq!(config.jwt_issuer, "http://127.0.0.1:54321/auth/v1");
        assert_eq!(
            config.jwks_url(),
            "http://127.0.0.1:54321/auth/v1/.well-known/jwks.json"
        );
        assert_eq!(config.bind.to_string(), DEFAULT_BIND);
        assert_eq!(
            config.cors_origins,
            vec![HeaderValue::from_static("http://localhost:3000")]
        );
        assert_eq!(config.database_max_connections, DEFAULT_MAX_CONNECTIONS);
    }

    #[test]
    fn explicit_values_are_parsed() {
        let mut vars = REQUIRED.to_vec();
        vars.extend([
            ("SUPABASE_JWT_ISSUER", "https://auth.example.com/auth/v1"),
            ("API_BIND", "0.0.0.0:8080"),
            ("CORS_ORIGINS", "http://localhost:3000, https://atelier.example.com/"),
            ("DATABASE_MAX_CONNECTIONS", "12"),
        ]);
        let config = config(&vars).expect("valid configuration");
        assert_eq!(config.jwt_issuer, "https://auth.example.com/auth/v1");
        assert_eq!(config.bind.to_string(), "0.0.0.0:8080");
        assert_eq!(
            config.cors_origins,
            vec![
                HeaderValue::from_static("http://localhost:3000"),
                HeaderValue::from_static("https://atelier.example.com"),
            ]
        );
        assert_eq!(config.database_max_connections, 12);
    }

    #[test]
    fn missing_and_invalid_values_are_reported() {
        assert_eq!(config(&[]).unwrap_err(), ConfigError::Missing("DATABASE_URL"));
        assert_eq!(
            config(&REQUIRED[..1]).unwrap_err(),
            ConfigError::Missing("SUPABASE_URL")
        );
        let mut vars = REQUIRED.to_vec();
        vars.push(("API_BIND", "localhost"));
        assert!(matches!(
            config(&vars),
            Err(ConfigError::Invalid { name: "API_BIND", .. })
        ));
        let mut vars = REQUIRED.to_vec();
        vars.push(("DATABASE_MAX_CONNECTIONS", "0"));
        assert!(matches!(
            config(&vars),
            Err(ConfigError::Invalid {
                name: "DATABASE_MAX_CONNECTIONS",
                ..
            })
        ));
        let vars = [REQUIRED[0], ("SUPABASE_URL", "127.0.0.1:54321")];
        assert!(matches!(
            config(&vars),
            Err(ConfigError::Invalid {
                name: "SUPABASE_URL",
                ..
            })
        ));
    }
}
