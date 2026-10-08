//! IR de Deep Atelier : représentation intermédiaire typée et indépendante de la plateforme.
//!
//! - [`document`], [`node`], [`tokens`], [`style`] : le schéma (aucun concept DOM/CSS).
//! - [`command`] : l'API publique de mutation (UI, parser, LLM), abaissée en [`op::Op`] inversibles.
//! - [`history`] : sessions, transactions, gestes, brouillons IA, undo/redo.
//! - [`render`] : rendu des pages (instances développées, slots remplis), hôtes de rendu.
//! - [`validate`] : schéma, accessibilité, contraste, responsive, qualité.
//! - [`migrate`] : chargement des documents sérialisés et migrations de version.

mod error;
mod macros;

pub mod command;
pub mod document;
pub mod history;
pub mod id;
pub mod migrate;
pub mod node;
pub mod op;
pub mod query;
pub mod render;
pub mod scope;
pub mod style;
pub mod tokens;
pub mod validate;

pub use command::{Command, CommandError};
pub use document::*;
pub use error::ValueError;
pub use history::{Applied, ChangeUnit, ChangeUnitId, DraftAccept, Session, Transaction};
pub use id::*;
pub use node::*;
pub use op::{ChangeSet, Op, OpError};
pub use scope::{Origin, Scope};
pub use style::*;
pub use tokens::*;
pub use validate::{Issue, IssueCode, Severity, validate};
