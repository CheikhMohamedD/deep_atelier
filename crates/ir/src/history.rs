//! Session d'édition : transactions atomiques, undo/redo illimité, gestes, brouillons IA.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::command::{Command, CommandError, LowerOutput, Lowering};
use crate::document::Document;
use crate::id::{ComponentId, IdGen, LayoutId, NodeId, PageId};
use crate::op::{ChangeSet, Op};
use crate::scope::{Origin, Scope};
use crate::validate::{Issue, IssueKey, validate};

/// Lot de commandes appliqué atomiquement (une entrée d'undo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct Transaction {
    pub label: String,
    pub origin: Origin,
    #[serde(default)]
    pub scope: Scope,
    pub commands: Vec<Command>,
}

impl Transaction {
    /// Transaction utilisateur sans restriction de périmètre.
    pub fn user(label: impl Into<String>, commands: Vec<Command>) -> Self {
        Self {
            label: label.into(),
            origin: Origin::User,
            scope: Scope::full(),
            commands,
        }
    }
}

/// Résultat d'une transaction appliquée.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct Applied {
    pub ops: Vec<Op>,
    /// Inverses, dans l'ordre d'application des ops (à rejouer à rebours).
    pub inverse: Vec<Op>,
    /// `$ref` → id créé.
    pub created: BTreeMap<String, NodeId>,
    /// Racines des sous-arbres insérés.
    pub inserted: Vec<NodeId>,
    pub pages: Vec<PageId>,
    pub layouts: Vec<LayoutId>,
    pub components: Vec<ComponentId>,
    pub changes: ChangeSet,
    /// Problèmes du document après la transaction.
    pub issues: Vec<Issue>,
}

/// Identifiant d'une unité de revue d'un brouillon IA (une commande du run).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct ChangeUnitId(pub u32);

/// Acceptation d'un brouillon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind", content = "units")]
pub enum DraftAccept {
    All,
    Units(Vec<ChangeUnitId>),
}

/// Unité de revue exposée à l'UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ChangeUnit {
    pub id: ChangeUnitId,
    /// Nom de la commande (`insert_nodes`).
    pub label: String,
    pub changes: ChangeSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    label: String,
    origin: Origin,
    ops: Vec<Op>,
    inverse: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq)]
struct DraftUnit {
    id: ChangeUnitId,
    label: String,
    ops: Vec<Op>,
    inverse: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq)]
struct Draft {
    run_id: String,
    label: Option<String>,
    refs: BTreeMap<String, NodeId>,
    units: Vec<DraftUnit>,
    base_errors: BTreeSet<IssueKey>,
}

/// Applique des ops dans l'ordre ; en cas d'échec, annule celles déjà appliquées.
/// Retourne les inverses (dans l'ordre d'application).
fn apply_all(doc: &mut Document, ops: &[Op], clamp: bool) -> Result<(Vec<Op>, Vec<Op>), CommandError> {
    let mut applied = Vec::with_capacity(ops.len());
    let mut inverse = Vec::with_capacity(ops.len());
    for op in ops {
        let mut op = op.clone();
        if clamp {
            op.clamp_indices(doc);
        }
        match op.apply(doc) {
            Ok(inv) => {
                applied.push(op);
                inverse.push(inv);
            }
            Err(error) => {
                undo_all(doc, &inverse);
                return Err(error.into());
            }
        }
    }
    Ok((applied, inverse))
}

/// Rejoue des inverses à rebours (sans échec possible sur un état cohérent).
fn undo_all(doc: &mut Document, inverse: &[Op]) {
    for op in inverse.iter().rev() {
        // Inverses d'ops appliquées avec succès sur cet état : leur application réussit.
        let _ = op.apply(doc);
    }
}

fn error_keys(issues: &[Issue]) -> BTreeSet<IssueKey> {
    issues.iter().filter(|i| i.is_error()).map(Issue::key).collect()
}

/// Session d'édition d'un document.
#[derive(Debug, Clone)]
pub struct Session {
    doc: Document,
    ids: IdGen,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    gesture: Option<Entry>,
    draft: Option<Draft>,
}

impl Session {
    /// `seed` initialise le générateur d'ids (aléa fourni par l'hôte).
    pub fn new(doc: Document, seed: u64) -> Self {
        Self {
            doc,
            ids: IdGen::from_seed(seed),
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
            draft: None,
        }
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn into_document(self) -> Document {
        self.doc
    }

    pub fn can_undo(&self) -> bool {
        self.draft.is_none() && (!self.undo.is_empty() || self.gesture.as_ref().is_some_and(|g| !g.ops.is_empty()))
    }

    pub fn can_redo(&self) -> bool {
        self.draft.is_none() && !self.redo.is_empty()
    }

    /// Libellés de la pile d'undo (le plus récent en dernier).
    pub fn undo_labels(&self) -> Vec<&str> {
        self.undo.iter().map(|e| e.label.as_str()).collect()
    }

    /// Origine de la dernière entrée d'undo.
    pub fn last_origin(&self) -> Option<&Origin> {
        self.undo.last().map(|e| &e.origin)
    }

    /// Applique une transaction de façon atomique : en cas d'erreur, rien n'est appliqué.
    pub fn apply(&mut self, tx: Transaction) -> Result<Applied, CommandError> {
        let in_draft = match (&self.draft, &tx.origin) {
            (Some(draft), Origin::Ai { run_id }) if *run_id == draft.run_id => true,
            (Some(_), _) => return Err(CommandError::DraftInProgress),
            (None, _) => false,
        };
        let mut refs = match &self.draft {
            Some(draft) if in_draft => draft.refs.clone(),
            _ => BTreeMap::new(),
        };
        let mut output = LowerOutput::default();
        let mut ops = Vec::new();
        let mut inverse = Vec::new();
        let mut units = Vec::new();
        for (index, command) in tx.commands.iter().enumerate() {
            let mut lowering = Lowering::new(&mut self.doc, &mut self.ids, &mut refs, &tx.scope, &tx.origin);
            if let Err(error) = lowering.lower(command) {
                lowering.rollback();
                undo_all(&mut self.doc, &inverse);
                return Err(CommandError::InCommand {
                    index,
                    command: command.name(),
                    source: Box::new(error),
                });
            }
            let command_ops = std::mem::take(&mut lowering.ops);
            let command_inverse = std::mem::take(&mut lowering.inverse);
            let out = std::mem::take(&mut lowering.out);
            output.created.extend(out.created);
            output.inserted.extend(out.inserted);
            output.pages.extend(out.pages);
            output.layouts.extend(out.layouts);
            output.components.extend(out.components);
            units.push((command.name().to_owned(), command_ops.clone(), command_inverse.clone()));
            ops.extend(command_ops);
            inverse.extend(command_inverse);
        }

        let changes = ChangeSet::from_ops(ops.iter().chain(inverse.iter()));
        let issues = validate(&self.doc);
        if in_draft && let Some(draft) = &mut self.draft {
            draft.refs = refs;
            if draft.label.is_none() {
                draft.label = Some(tx.label.clone());
            }
            for (label, unit_ops, unit_inverse) in units {
                if unit_ops.is_empty() {
                    continue;
                }
                let id = ChangeUnitId(draft.units.len() as u32);
                draft.units.push(DraftUnit {
                    id,
                    label,
                    ops: unit_ops,
                    inverse: unit_inverse,
                });
            }
        } else if !ops.is_empty() {
            match &mut self.gesture {
                Some(gesture) => {
                    gesture.ops.extend(ops.iter().cloned());
                    gesture.inverse.extend(inverse.iter().cloned());
                }
                None => {
                    self.undo.push(Entry {
                        label: tx.label.clone(),
                        origin: tx.origin.clone(),
                        ops: ops.clone(),
                        inverse: inverse.clone(),
                    });
                }
            }
            self.redo.clear();
        }
        Ok(Applied {
            ops,
            inverse,
            created: output.created,
            inserted: output.inserted,
            pages: output.pages,
            layouts: output.layouts,
            components: output.components,
            changes,
            issues,
        })
    }

    /// Annule la dernière entrée. `Ok(None)` si rien à annuler.
    pub fn undo(&mut self) -> Result<Option<ChangeSet>, CommandError> {
        if self.draft.is_some() {
            return Err(CommandError::DraftInProgress);
        }
        self.end_gesture();
        let Some(entry) = self.undo.pop() else { return Ok(None) };
        let reversed: Vec<Op> = entry.inverse.iter().rev().cloned().collect();
        if let Err(error) = apply_all(&mut self.doc, &reversed, false) {
            self.undo.push(entry);
            return Err(error);
        }
        let changes = ChangeSet::from_ops(entry.ops.iter().chain(entry.inverse.iter()));
        self.redo.push(entry);
        Ok(Some(changes))
    }

    /// Rétablit la dernière entrée annulée. `Ok(None)` si rien à rétablir.
    pub fn redo(&mut self) -> Result<Option<ChangeSet>, CommandError> {
        if self.draft.is_some() {
            return Err(CommandError::DraftInProgress);
        }
        self.end_gesture();
        let Some(entry) = self.redo.pop() else { return Ok(None) };
        if let Err(error) = apply_all(&mut self.doc, &entry.ops, false) {
            self.redo.push(entry);
            return Err(error);
        }
        let changes = ChangeSet::from_ops(entry.ops.iter().chain(entry.inverse.iter()));
        self.undo.push(entry);
        Ok(Some(changes))
    }

    /// Début d'un geste continu (slider, glisser) : les transactions suivantes forment une seule
    /// entrée d'undo jusqu'à `end_gesture`.
    pub fn begin_gesture(&mut self, label: &str) {
        self.end_gesture();
        self.gesture = Some(Entry {
            label: label.to_owned(),
            origin: Origin::User,
            ops: Vec::new(),
            inverse: Vec::new(),
        });
    }

    pub fn end_gesture(&mut self) {
        if let Some(gesture) = self.gesture.take()
            && !gesture.ops.is_empty()
        {
            self.undo.push(gesture);
            self.redo.clear();
        }
    }

    /// Ouvre un brouillon IA : les transactions du run s'y accumulent, hors historique.
    pub fn begin_draft(&mut self, run_id: &str) -> Result<(), CommandError> {
        if self.draft.is_some() {
            return Err(CommandError::DraftInProgress);
        }
        self.end_gesture();
        let base_errors = error_keys(&validate(&self.doc));
        self.draft = Some(Draft {
            run_id: run_id.to_owned(),
            label: None,
            refs: BTreeMap::new(),
            units: Vec::new(),
            base_errors,
        });
        Ok(())
    }

    pub fn has_draft(&self) -> bool {
        self.draft.is_some()
    }

    /// Unités de revue du brouillon courant.
    pub fn draft_units(&self) -> Vec<ChangeUnit> {
        self.draft
            .iter()
            .flat_map(|d| d.units.iter())
            .map(|u| ChangeUnit {
                id: u.id,
                label: u.label.clone(),
                changes: ChangeSet::from_ops(u.ops.iter().chain(u.inverse.iter())),
            })
            .collect()
    }

    /// Erreurs introduites par rapport au document d'avant le brouillon.
    fn new_errors(&self, base: &BTreeSet<IssueKey>) -> Vec<Issue> {
        validate(&self.doc)
            .into_iter()
            .filter(|i| i.is_error() && !base.contains(&i.key()))
            .collect()
    }

    /// Valide le brouillon en une seule entrée d'undo. Refusé (brouillon conservé) si le
    /// résultat introduit des erreurs bloquantes.
    pub fn commit_draft(&mut self, accept: DraftAccept) -> Result<ChangeSet, CommandError> {
        let draft = self.draft.take().ok_or(CommandError::NoDraft)?;
        let all_ops: Vec<Op> = draft.units.iter().flat_map(|u| u.ops.iter().cloned()).collect();
        let all_inverse: Vec<Op> = draft.units.iter().flat_map(|u| u.inverse.iter().cloned()).collect();
        let (ops, inverse) = match &accept {
            DraftAccept::All => (all_ops.clone(), all_inverse.clone()),
            DraftAccept::Units(selected) => {
                let selected: BTreeSet<ChangeUnitId> = selected.iter().copied().collect();
                undo_all(&mut self.doc, &all_inverse);
                let wanted: Vec<Op> = draft
                    .units
                    .iter()
                    .filter(|u| selected.contains(&u.id))
                    .flat_map(|u| u.ops.iter().cloned())
                    .collect();
                match apply_all(&mut self.doc, &wanted, true) {
                    Ok(result) => result,
                    Err(error) => {
                        // Rétablit le brouillon complet.
                        let _ = apply_all(&mut self.doc, &all_ops, false);
                        self.draft = Some(draft);
                        return Err(CommandError::UnitDependency(error.to_string()));
                    }
                }
            }
        };
        let errors = self.new_errors(&draft.base_errors);
        if !errors.is_empty() {
            if matches!(accept, DraftAccept::Units(_)) {
                undo_all(&mut self.doc, &inverse);
                let _ = apply_all(&mut self.doc, &all_ops, false);
            }
            self.draft = Some(draft);
            return Err(CommandError::ValidationFailed(errors));
        }
        let changes = ChangeSet::from_ops(all_ops.iter().chain(all_inverse.iter()));
        if !ops.is_empty() {
            self.undo.push(Entry {
                label: draft
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("AI run {}", draft.run_id)),
                origin: Origin::Ai {
                    run_id: draft.run_id.clone(),
                },
                ops,
                inverse,
            });
            self.redo.clear();
        }
        Ok(changes)
    }

    /// Abandonne le brouillon : le document revient à son état d'avant le run.
    pub fn discard_draft(&mut self) -> ChangeSet {
        let Some(draft) = self.draft.take() else {
            return ChangeSet::default();
        };
        let ops: Vec<Op> = draft.units.iter().flat_map(|u| u.ops.iter().cloned()).collect();
        let inverse: Vec<Op> = draft.units.iter().flat_map(|u| u.inverse.iter().cloned()).collect();
        undo_all(&mut self.doc, &inverse);
        ChangeSet::from_ops(ops.iter().chain(inverse.iter()))
    }

    /// Problèmes du document courant.
    pub fn validate(&self) -> Vec<Issue> {
        validate(&self.doc)
    }
}
