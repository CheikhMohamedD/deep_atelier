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
use crate::document::{Document, Owner};
use crate::id::{ComponentId, NodeId};
use crate::node::{DEFAULT_SLOT, NodeKind, PAGE_SLOT};
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
    LeafHasChildren,
    EmptyInteractive,
    NestedInteractive,
    ListItemOutsideList,
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

/// Contexte partagé : propriétaire de chaque nœud atteignable depuis une racine.
pub(crate) struct Context<'a> {
    pub doc: &'a Document,
    pub owners: BTreeMap<NodeId, Owner>,
}

impl<'a> Context<'a> {
    fn new(doc: &'a Document) -> Self {
        let mut owners = BTreeMap::new();
        for (owner, root) in doc.roots() {
            for id in doc.subtree(&root) {
                owners.entry(id).or_insert_with(|| owner.clone());
            }
        }
        Self { doc, owners }
    }

    pub fn owner(&self, id: &NodeId) -> Option<&Owner> {
        self.owners.get(id)
    }

    /// Nœuds d'une page dans l'ordre de rendu : layout (slot `page` remplacé par la page),
    /// instances développées (contenu des slots compris).
    pub fn render_order(&self, page: &crate::document::Page) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = Vec::new();
        match page.layout.as_ref().and_then(|l| self.doc.layout(l)) {
            Some(layout) => self.walk(&layout.root, Some(&page.root), None, &mut stack, &mut out),
            None => self.walk(&page.root, None, None, &mut stack, &mut out),
        }
        out
    }

    fn walk(
        &self,
        id: &NodeId,
        page_root: Option<&NodeId>,
        instance: Option<&NodeId>,
        components: &mut Vec<ComponentId>,
        out: &mut Vec<NodeId>,
    ) {
        let Some(node) = self.doc.node(id) else { return };
        if out.len() > self.doc.nodes.len() * 4 {
            return;
        }
        out.push(id.clone());
        match &node.kind {
            NodeKind::Slot(slot) if slot.name == PAGE_SLOT && page_root.is_some() => {
                if let Some(page_root) = page_root {
                    self.walk(page_root, None, None, components, out);
                }
                return;
            }
            NodeKind::Slot(slot) => {
                if let Some(instance) = instance.and_then(|i| self.doc.node(i)) {
                    for child in &instance.children {
                        let target = self.doc.node(child).and_then(|c| c.meta.slot.clone());
                        if target.as_deref().unwrap_or(DEFAULT_SLOT) == slot.name {
                            self.walk(child, page_root, None, components, out);
                        }
                    }
                }
                return;
            }
            NodeKind::ComponentInstance(props) => {
                if components.contains(&props.component) {
                    return;
                }
                if let Some(component) = self.doc.component(&props.component) {
                    components.push(props.component.clone());
                    self.walk(&component.root, page_root, Some(id), components, out);
                    components.pop();
                }
                return;
            }
            _ => {}
        }
        for child in &node.children {
            self.walk(child, page_root, instance, components, out);
        }
    }

    /// Ancres définies dans une page (page + layout).
    pub fn page_anchors(&self, page: &crate::document::Page) -> BTreeSet<String> {
        let mut roots = vec![page.root.clone()];
        if let Some(layout) = page.layout.as_ref().and_then(|l| self.doc.layout(l)) {
            roots.push(layout.root.clone());
        }
        roots
            .iter()
            .flat_map(|r| self.doc.subtree(r))
            .filter_map(|id| self.doc.node(&id).and_then(|n| n.meta.anchor.clone()))
            .collect()
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
