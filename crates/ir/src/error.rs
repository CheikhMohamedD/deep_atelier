//! Erreurs de valeur (analyse des formes textuelles).

use std::fmt;

/// Valeur textuelle invalide pour un type de l'IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueError {
    /// Nom du type attendu.
    pub expected: &'static str,
    /// Texte refusé.
    pub found: String,
}

impl ValueError {
    pub fn new(expected: &'static str, found: impl Into<String>) -> Self {
        Self {
            expected,
            found: found.into(),
        }
    }
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {} value: {:?}", self.expected, self.found)
    }
}

impl std::error::Error for ValueError {}

/// Schéma JSON d'une chaîne énumérée.
pub(crate) fn enum_schema<I, S>(values: I) -> schemars::Schema
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let values: Vec<String> = values.into_iter().map(Into::into).collect();
    schemars::json_schema!({ "type": "string", "enum": values })
}

/// Schéma JSON d'une chaîne contrainte par une expression régulière.
pub(crate) fn pattern_schema(pattern: &str) -> schemars::Schema {
    schemars::json_schema!({ "type": "string", "pattern": pattern })
}
