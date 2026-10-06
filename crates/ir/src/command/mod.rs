//! Commandes de mutation : définition, spécifications d'entrée, abaissement en ops.

#[allow(clippy::module_inception)]
pub mod command;
pub mod lower;
pub mod neutral;
pub mod spec;

pub use command::{Command, CommandError};
pub use lower::{LowerOutput, Lowering};
pub use spec::*;
