//! Chargement et migration des documents sérialisés.
//!
//! Chaque version future ajoute une étape `vN → vN+1` sur le JSON brut, puis le document est
//! désérialisé dans la version courante. Seule la v1 existe pour l'instant.

use serde_json::Value;

use crate::document::{Document, IR_VERSION};

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("document has no numeric `version` field")]
    MissingVersion,
    #[error("document version {0} is newer than this engine ({IR_VERSION})")]
    TooNew(u64),
    #[error("document version {0} is not supported")]
    Unsupported(u64),
    #[error("invalid document: {0}")]
    Invalid(#[from] serde_json::Error),
}

/// Migre un document JSON vers `IR_VERSION` puis le désérialise.
pub fn migrate(mut value: Value) -> Result<Document, MigrateError> {
    let version = value
        .get("version")
        .and_then(Value::as_u64)
        .ok_or(MigrateError::MissingVersion)?;
    if version > u64::from(IR_VERSION) {
        return Err(MigrateError::TooNew(version));
    }
    if version == 0 {
        return Err(MigrateError::Unsupported(version));
    }
    // Étapes de migration successives (aucune tant que IR_VERSION vaut 1).
    if let Some(object) = value.as_object_mut() {
        object.insert("version".to_owned(), Value::from(IR_VERSION));
    }
    Ok(serde_json::from_value(value)?)
}

/// Charge un document depuis du JSON.
pub fn load(json: &str) -> Result<Document, MigrateError> {
    migrate(serde_json::from_str(json)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::IdGen;

    #[test]
    fn loads_current_version_and_rejects_others() {
        let doc = Document::new("Site", &mut IdGen::from_seed(3));
        let json = serde_json::to_string(&doc).unwrap();
        assert_eq!(load(&json).unwrap(), doc);
        let mut value: Value = serde_json::from_str(&json).unwrap();
        value["version"] = Value::from(99);
        assert!(matches!(migrate(value.clone()), Err(MigrateError::TooNew(99))));
        value["version"] = Value::from(0);
        assert!(matches!(migrate(value.clone()), Err(MigrateError::Unsupported(0))));
        value.as_object_mut().unwrap().remove("version");
        assert!(matches!(migrate(value), Err(MigrateError::MissingVersion)));
    }
}
