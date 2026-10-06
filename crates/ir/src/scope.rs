//! Origine et périmètre d'une transaction.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::NodeId;
use crate::node::NodeSource;
use crate::style::responsive::Breakpoint;

/// Auteur d'une transaction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    User,
    Ai { run_id: String },
    Code,
    System,
}

impl Origin {
    /// Provenance enregistrée sur les nœuds créés.
    pub fn node_source(&self) -> NodeSource {
        match self {
            Origin::Ai { .. } => NodeSource::Ai,
            Origin::Code => NodeSource::Code,
            Origin::User | Origin::System => NodeSource::Visual,
        }
    }

    pub fn is_ai(&self) -> bool {
        matches!(self, Origin::Ai { .. })
    }
}

/// Périmètre autorisé d'une transaction (prompt sur sélection, sur breakpoint, mode Contenu).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Scope {
    /// Mode Contenu : textes, images, liens seulement.
    #[serde(default)]
    pub content_only: bool,
    /// Prompt sur breakpoint : valeurs de ce breakpoint seulement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub breakpoint: Option<Breakpoint>,
    /// Prompt sur sélection : nœuds cibles limités à ces sous-arbres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub subtree: Option<Vec<NodeId>>,
}

impl Scope {
    /// Aucun restriction.
    pub fn full() -> Self {
        Self::default()
    }

    pub fn is_full(&self) -> bool {
        !self.content_only && self.breakpoint.is_none() && self.subtree.is_none()
    }
}
