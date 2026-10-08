//! Corps des requêtes et des réponses de l'API, exportés en TypeScript (`cargo xtask codegen`)
//! pour l'éditeur : le contrat se vérifie à la compilation des deux côtés.

use ir::Issue;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::db::{DocumentState, Project, Version, VersionWithDocument};

/// `GET /health`.
#[derive(Debug, Clone, Serialize, TS)]
pub struct Health {
    pub status: String,
}

/// `POST /projects`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct NewProject {
    pub name: String,
}

/// `PATCH /projects/{id}`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectPatch {
    pub name: String,
}

/// `PUT /projects/{id}/document` : la sauvegarde remplace la version `base_version`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SaveDocument {
    pub base_version: i64,
    #[ts(as = "ir::Document")]
    pub document: Value,
}

/// `POST /projects/{id}/versions`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct NewVersion {
    pub message: String,
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
}

/// `POST /projects/{id}/versions/{number}/restore`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Restore {
    pub base_version: i64,
}

/// `GET /projects`.
#[derive(Debug, Clone, Serialize, TS)]
pub struct ProjectList {
    pub projects: Vec<Project>,
}

/// `GET`, `PATCH /projects/{id}`.
#[derive(Debug, Clone, Serialize, TS)]
pub struct ProjectResponse {
    pub project: Project,
}

/// `POST /projects` : le projet et son document neuf.
#[derive(Debug, Clone, Serialize, TS)]
pub struct CreatedProject {
    pub project: Project,
    pub document: DocumentState,
}

/// `GET /projects/{id}/versions`.
#[derive(Debug, Clone, Serialize, TS)]
pub struct VersionList {
    pub versions: Vec<Version>,
}

/// `POST /projects/{id}/versions`.
#[derive(Debug, Clone, Serialize, TS)]
pub struct VersionResponse {
    pub version: Version,
}

/// `GET /projects/{id}/versions/{number}`.
#[derive(Debug, Clone, Serialize, TS)]
pub struct VersionDetail {
    pub version: VersionWithDocument,
}

/// `POST /projects/{id}/versions/{number}/restore` : l'état gardé avant la restauration, et le
/// nouveau document courant.
#[derive(Debug, Clone, Serialize, TS)]
pub struct Restored {
    pub backup: Version,
    pub document: DocumentState,
}

/// Corps de toute réponse d'erreur.
#[derive(Debug, Clone, Serialize, TS)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct ErrorDetail {
    /// Code stable (`UNAUTHORIZED`, `NOT_FOUND`, `VERSION_CONFLICT`, `INVALID_REQUEST`,
    /// `PAYLOAD_TOO_LARGE`, `INVALID_DOCUMENT`, `UNAVAILABLE`, `INTERNAL`).
    pub code: String,
    /// Message en anglais ; l'éditeur affiche la traduction du code.
    pub message: String,
    /// `VERSION_CONFLICT` : version courante du document.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub current_version: Option<i64>,
    /// `INVALID_DOCUMENT` : problèmes d'intégrité.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub issues: Option<Vec<Issue>>,
}
