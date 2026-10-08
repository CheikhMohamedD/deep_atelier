//! Compilateur web de Deep Atelier : IR → projet Next.js 16 (App Router, TypeScript strict,
//! Tailwind CSS v4), sans dépendance à Deep Atelier au runtime.
//!
//! - [`compile`] : projet complet (pages, layouts, composants, thème, configuration).
//! - Le code est mis en forme comme Prettier 3 le ferait (imprimeur de [`doc`]).
//! - En mode [`Mode::Edit`], chaque élément porte `data-atl-id` (lien code ⇄ canvas) ; chaque
//!   fichier donne la plage de chaque nœud ([`File::source_map`]).

pub mod classes;
pub mod demo;
pub mod doc;
pub mod elements;
pub mod jsx;
pub mod module;
pub mod names;
pub mod plan;
pub mod project;
pub mod theme;

use ir::{AssetId, Document, NodeId};

/// Usage du code produit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Code exporté : aucune trace de Deep Atelier.
    Export,
    /// Code affiché dans l'éditeur : `data-atl-id` sur chaque élément.
    Edit,
}

/// Plage d'un nœud dans un fichier (octets, fin exclue).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRange {
    pub node: NodeId,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContents {
    Text(String),
    /// Fichier d'un asset, copié depuis le stockage par l'export.
    Asset(AssetId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// Chemin relatif à la racine du projet.
    pub path: String,
    pub contents: FileContents,
    pub source_map: Vec<SourceRange>,
}

impl File {
    pub fn text(&self) -> Option<&str> {
        match &self.contents {
            FileContents::Text(text) => Some(text),
            FileContents::Asset(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub files: Vec<File>,
}

impl Project {
    pub fn file(&self, path: &str) -> Option<&File> {
        self.files.iter().find(|f| f.path == path)
    }
}

/// Compile un document en projet Next.js.
pub fn compile(doc: &Document, mode: Mode) -> Project {
    project::compile(doc, mode)
}
