//! Validation du document : schéma, accessibilité, contraste, responsive, qualité.
//!
//! Les messages sont en anglais (renvoyés au LLM) ; l'UI les traduit à partir du `code`.
//! Le débordement horizontal mesuré dans le navigateur (famille « Rendu ») n'est pas calculé ici.

mod a11y;
mod contrast;
mod responsive;
mod schema;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::command::Command;
use crate::document::{Document, Owner, Page};
use crate::id::NodeId;
use crate::render::RenderTree;
use crate::style::responsive::Breakpoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub enum Severity {
    /// Bloque l'acceptation d'une génération IA et la publication.
    Error,
    Warning,
    Info,
}

/// Codes stables des problèmes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IssueCode {
    // Schéma
    InvalidVersion,
    NoWebTarget,
    NoPage,
    DuplicateId,
    TreeInconsistent,
    OrphanNode,
    Cycle,
    RootNotContainer,
    LeafHasChildren,
    EmptyInteractive,
    NestedInteractive,
    ListItemOutsideList,
    ListChildNotItem,
    SlotOutsideComponent,
    LayoutPageSlot,
    DuplicateSlot,
    InvalidSlotTarget,
    StyleNotAllowed,
    InvalidReference,
    MissingIntrinsic,
    UnknownIcon,
    InvalidAnchor,
    DuplicateAnchor,
    InvalidRoute,
    DuplicateRoute,
    InvalidName,
    DuplicateName,
    InvalidProp,
    RecursiveComponent,
    InvalidToken,
    // Accessibilité
    A11yImgAlt,
    A11yMultipleH1,
    A11yMissingH1,
    A11yHeadingSkip,
    A11yAccessibleName,
    A11yContrast,
    A11yMultipleMain,
    A11yMissingMain,
    A11yLang,
    // Responsive
    RespFixedWidthOverflow,
    RespRowTooManyChildren,
    RespTooManyColumns,
    RespHeadingTooLarge,
    RespTouchTarget,
    RespInputFontSize,
    // Qualité
    QualityOffTokenColor,
    QualityPxValue,
    QualityEmptyContainer,
}

impl IssueCode {
    /// Problème d'intégrité : version, cible, identifiants, arbre ou références incohérents. Un
    /// tel document ne se charge, ne se rend ni ne se compile de façon sûre ; l'API refuse de
    /// l'enregistrer. Les commandes ne peuvent pas en produire.
    pub fn is_integrity(self) -> bool {
        matches!(
            self,
            Self::InvalidVersion
                | Self::NoWebTarget
                | Self::NoPage
                | Self::DuplicateId
                | Self::TreeInconsistent
                | Self::OrphanNode
                | Self::Cycle
                | Self::InvalidReference
                | Self::RecursiveComponent
        )
    }
}

/// Problème détecté.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct Issue {
    pub code: IssueCode,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub breakpoint: Option<Breakpoint>,
    pub message: String,
    /// Correction proposée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fix: Option<Vec<Command>>,
}

/// Clé d'identité d'un problème (comparaison avant / après un brouillon).
pub type IssueKey = (IssueCode, Option<NodeId>, Option<Breakpoint>, String);

impl Issue {
    pub fn new(code: IssueCode, severity: Severity, node: Option<&NodeId>, message: impl Into<String>) -> Self {
        Self {
            code,
            severity,
            node: node.cloned(),
            breakpoint: None,
            message: message.into(),
            fix: None,
        }
    }

    pub fn error(code: IssueCode, node: Option<&NodeId>, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Error, node, message)
    }

    pub fn warning(code: IssueCode, node: Option<&NodeId>, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Warning, node, message)
    }

    pub fn info(code: IssueCode, node: Option<&NodeId>, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Info, node, message)
    }

    pub fn at(mut self, bp: Breakpoint) -> Self {
        self.breakpoint = Some(bp);
        self
    }

    pub fn with_fix(mut self, fix: Vec<Command>) -> Self {
        self.fix = Some(fix);
        self
    }

    pub fn key(&self) -> IssueKey {
        (self.code, self.node.clone(), self.breakpoint, self.message.clone())
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// Valide tout le document. Résultat trié (sévérité, code, nœud, breakpoint).
pub fn validate(doc: &Document) -> Vec<Issue> {
    let ctx = Context::new(doc);
    let mut issues = Vec::new();
    schema::check(&ctx, &mut issues);
    a11y::check(&ctx, &mut issues);
    contrast::check(&ctx, &mut issues);
    responsive::check(&ctx, &mut issues);
    issues.sort_by(|a, b| {
        (a.severity, a.code, &a.node, a.breakpoint, &a.message).cmp(&(
            b.severity,
            b.code,
            &b.node,
            b.breakpoint,
            &b.message,
        ))
    });
    issues.dedup_by(|a, b| a.key() == b.key());
    issues
}

/// Contexte partagé : propriétaire de chaque nœud atteignable depuis une racine, rendu de chaque
/// page, nœuds rendus par au moins une page.
pub(crate) struct Context<'a> {
    pub doc: &'a Document,
    pub owners: BTreeMap<NodeId, Owner>,
    /// Rendu de chaque page, dans l'ordre des pages.
    pub renders: Vec<(&'a Page, RenderTree)>,
    /// Occurrences rendues de chaque nœud : (rang de la page, rang dans son rendu). Les règles
    /// qui dépendent du contexte de rendu jugent un nœud à chacun de ses rendus ; un nœud jamais
    /// rendu (composant sans instance, layout sans page) est jugé dans l'arbre où il est écrit.
    rendered: BTreeMap<NodeId, Vec<(usize, usize)>>,
    /// Ancres rendues dans chaque page (layout, page, composants développés), par rang de page.
    anchors: Vec<BTreeSet<String>>,
}

impl<'a> Context<'a> {
    fn new(doc: &'a Document) -> Self {
        let mut owners = BTreeMap::new();
        for (owner, root) in doc.roots() {
            for id in doc.subtree(&root) {
                owners.entry(id).or_insert_with(|| owner.clone());
            }
        }
        let renders: Vec<(&Page, RenderTree)> = doc.pages.iter().map(|p| (p, RenderTree::of_page(doc, p))).collect();
        let mut rendered: BTreeMap<NodeId, Vec<(usize, usize)>> = BTreeMap::new();
        for (page, (_, tree)) in renders.iter().enumerate() {
            for (index, node) in tree.nodes.iter().enumerate() {
                rendered.entry(node.id.clone()).or_default().push((page, index));
            }
        }
        let anchors = renders
            .iter()
            .map(|(_, tree)| {
                tree.nodes
                    .iter()
                    .filter_map(|n| doc.node(&n.id).and_then(|n| n.meta.anchor.clone()))
                    .collect()
            })
            .collect();
        Self {
            doc,
            owners,
            renders,
            rendered,
            anchors,
        }
    }

    pub fn owner(&self, id: &NodeId) -> Option<&Owner> {
        self.owners.get(id)
    }

    pub fn is_rendered(&self, id: &NodeId) -> bool {
        self.rendered.contains_key(id)
    }

    /// Occurrences rendues d'un nœud : page, rendu de la page, rang dans ce rendu.
    pub fn occurrences(&self, id: &NodeId) -> Vec<(&'a Page, &RenderTree, usize)> {
        self.rendered
            .get(id)
            .map(|list| {
                list.iter()
                    .map(|&(page, index)| (self.renders[page].0, &self.renders[page].1, index))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Ancres rendues dans une page (layout, page, composants développés).
    pub fn page_anchors(&self, page: &Page) -> &BTreeSet<String> {
        static NONE: BTreeSet<String> = BTreeSet::new();
        self.renders
            .iter()
            .position(|(p, _)| p.id == page.id)
            .map_or(&NONE, |rank| &self.anchors[rank])
    }
}

/// Catalogue des icônes lucide.
pub fn is_known_icon(name: &str) -> bool {
    static ICONS: OnceLock<BTreeSet<&'static str>> = OnceLock::new();
    ICONS
        .get_or_init(|| {
            include_str!("../../data/lucide-icons.txt")
                .lines()
                .filter(|line| !line.starts_with('#') && !line.is_empty())
                .collect()
        })
        .contains(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_catalogue_is_loaded() {
        assert!(is_known_icon("menu"));
        assert!(is_known_icon("arrow-right"));
        assert!(!is_known_icon("definitely-not-an-icon"));
    }
}
