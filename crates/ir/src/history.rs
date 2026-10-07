//! Session d'édition : transactions atomiques, undo/redo illimité, gestes, brouillons IA.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::command::{Command, CommandError, LowerOutput, Lowering};
use crate::document::Document;
use crate::id::{ComponentId, IdGen, LayoutId, NodeId, PageId};
use crate::op::{ChangeSet, NodeField, Op, OpAccess, OpKey};
use crate::scope::{Origin, Scope};
use crate::style::responsive::Breakpoint;
use crate::validate::{Issue, IssueCode, validate};

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

/// Unité d'un brouillon : une commande, ses ops et le contexte de son abaissement (périmètre,
/// générateur d'ids et références locales d'avant la commande), qui permet de l'abaisser de
/// nouveau à l'acceptation partielle.
#[derive(Debug, Clone)]
struct DraftUnit {
    id: ChangeUnitId,
    label: String,
    command: Command,
    scope: Scope,
    ids: IdGen,
    refs: BTreeMap<String, NodeId>,
    ops: Vec<Op>,
    inverse: Vec<Op>,
}

#[derive(Debug, Clone)]
struct Draft {
    run_id: String,
    label: Option<String>,
    refs: BTreeMap<String, NodeId>,
    units: Vec<DraftUnit>,
    base_errors: ErrorCounts,
}

/// Identité d'une erreur : code, nœud, breakpoint et message privé des valeurs qu'un brouillon
/// peut changer sans créer de problème nouveau (voir [`stable_message`]).
type ErrorKey = (IssueCode, Option<NodeId>, Option<Breakpoint>, String);

/// Nombre d'erreurs d'un document par identité.
type ErrorCounts = BTreeMap<ErrorKey, usize>;

/// Mots qui, dans un message de validation, précèdent le nom (renommable) ou l'id d'une page,
/// d'un layout ou d'un composant cité entre accents graves (`page `Accueil``,
/// `layout name `site``).
const RENAMEABLE: [&str; 4] = ["page", "layout", "component", "name"];

fn is_entity_id(text: &str) -> bool {
    text.parse::<PageId>().is_ok() || text.parse::<LayoutId>().is_ok() || text.parse::<ComponentId>().is_ok()
}

/// Message réduit à ce qui distingue deux erreurs d'un même emplacement. Les noms de page, de
/// layout et de composant (renommables) et les nombres hors accents graves (ratio de contraste,
/// niveaux de titre, décomptes) sont masqués ; restent les termes qui identifient le problème :
/// token, prop, ancre, id cité, mode de contraste.
fn stable_message(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut previous_word = "";
    // Les segments impairs sont entre accents graves.
    for (index, part) in message.split('`').enumerate() {
        if index % 2 == 1 {
            out.push('`');
            if !RENAMEABLE.contains(&previous_word) || is_entity_id(part) {
                out.push_str(part);
            }
            out.push('`');
            continue;
        }
        previous_word = if part.ends_with(' ') {
            part.split_whitespace().last().unwrap_or_default()
        } else {
            ""
        };
        let mut in_number = false;
        for c in part.chars() {
            if c.is_ascii_digit() || (in_number && c == '.') {
                if !in_number {
                    out.push('#');
                }
                in_number = true;
            } else {
                in_number = false;
                out.push(c);
            }
        }
    }
    out
}

fn error_key(issue: &Issue) -> ErrorKey {
    (
        issue.code,
        issue.node.clone(),
        issue.breakpoint,
        stable_message(&issue.message),
    )
}

fn error_counts(issues: &[Issue]) -> ErrorCounts {
    let mut counts = ErrorCounts::new();
    for issue in issues.iter().filter(|i| i.is_error()) {
        *counts.entry(error_key(issue)).or_default() += 1;
    }
    counts
}

/// Erreurs introduites par rapport à `base` : chaque erreur consomme une erreur de même identité
/// d'avant le brouillon ; celles qui n'en trouvent plus sont nouvelles. Corriger une erreur et en
/// introduire une autre au même emplacement laisse donc la seconde visible.
fn introduced_errors(base: &ErrorCounts, issues: Vec<Issue>) -> Vec<Issue> {
    let mut remaining = base.clone();
    issues
        .into_iter()
        .filter(|issue| {
            if !issue.is_error() {
                return false;
            }
            match remaining.get_mut(&error_key(issue)) {
                Some(count) if *count > 0 => {
                    *count -= 1;
                    false
                }
                _ => true,
            }
        })
        .collect()
}

/// Données écrites par les unités rejetées d'un brouillon, avec la première unité qui les écrit.
#[derive(Default)]
struct RejectedWrites {
    keys: BTreeMap<OpKey, usize>,
    /// Nœuds dont au moins un champ est écrit.
    node_fields: BTreeMap<NodeId, usize>,
}

impl RejectedWrites {
    fn record(&mut self, key: OpKey, unit: usize) {
        if let OpKey::Field(node, _) = &key {
            self.node_fields.entry(node.clone()).or_insert(unit);
        }
        self.keys.entry(key).or_insert(unit);
    }

    /// Unité rejetée dont dépend une op : elle écrit une donnée que l'op remplace (la valeur
    /// enregistrée par l'op contient alors sa modification) ou crée, supprime ou modifie une
    /// donnée que l'op suppose.
    fn conflict(&self, access: &OpAccess) -> Option<usize> {
        let written = access.writes.iter().find_map(|key| {
            self.keys.get(key).copied().or_else(|| match key {
                // Un nœud inséré avec toutes ses valeurs recouvre chacun de ses champs, et
                // réciproquement.
                OpKey::Field(node, NodeField::All) => self.node_fields.get(node).copied(),
                OpKey::Field(node, _) => self.keys.get(&OpKey::Field(node.clone(), NodeField::All)).copied(),
                _ => None,
            })
        });
        written.or_else(|| access.reads.iter().find_map(|key| self.keys.get(key).copied()))
    }
}

/// Refuse une acceptation partielle dont une unité retenue dépend d'une unité rejetée plus
/// ancienne. Les ops enregistrent des valeurs entières (style d'une propriété, tokens, page…)
/// calculées sur l'état du brouillon : rejouées sans l'unité rejetée, elles réintroduiraient sa
/// modification ou viseraient une entité (nœud, page, token…) qu'elle seule crée. Ce contrôle
/// nomme l'unité rejetée en cause ; [`replay_units`] couvre ensuite ce que l'abaissement lit sans
/// l'écrire. L'ordre des enfants n'est pas une dépendance.
fn check_unit_dependencies(units: &[DraftUnit], selected: &BTreeSet<ChangeUnitId>) -> Result<(), CommandError> {
    let mut rejected = RejectedWrites::default();
    for (index, unit) in units.iter().enumerate() {
        let accesses = unit
            .ops
            .iter()
            .zip(&unit.inverse)
            .map(|(op, inverse)| op.access(inverse));
        if selected.contains(&unit.id) {
            for access in accesses {
                if let Some(culprit) = rejected.conflict(&access) {
                    let culprit = &units[culprit];
                    return Err(CommandError::UnitDependency(format!(
                        "unit {} (`{}`) builds on rejected unit {} (`{}`)",
                        unit.id.0, unit.label, culprit.id.0, culprit.label
                    )));
                }
            }
        } else {
            for key in accesses.flat_map(|access| access.writes) {
                rejected.record(key, index);
            }
        }
    }
    Ok(())
}

/// Applique des ops dans l'ordre ; en cas d'échec, annule celles déjà appliquées.
/// Retourne les inverses (dans l'ordre d'application).
fn apply_all(doc: &mut Document, ops: &[Op]) -> Result<Vec<Op>, CommandError> {
    let mut inverse = Vec::with_capacity(ops.len());
    for op in ops {
        match op.apply(doc) {
            Ok(inv) => inverse.push(inv),
            Err(error) => {
                undo_all(doc, &inverse);
                return Err(error.into());
            }
        }
    }
    Ok(inverse)
}

/// Rejoue les unités choisies, dans l'ordre, sur le document d'avant le brouillon : chaque
/// commande est abaissée de nouveau dans le contexte de son premier abaissement et doit redonner
/// les ops du brouillon, indices de position mis à part. Sinon, elle dépendait de ce qu'une unité
/// rejetée a changé et que ses ops ne nomment pas : valeurs copiées (`duplicate_node`,
/// `detach_instance`), enfants d'un nœud retiré (`unwrap_node`, `create_component`), valeur
/// neutre héritée d'un ancêtre… L'acceptation échoue alors (`UNIT_DEPENDENCY`) et le document
/// revient à son état d'avant le brouillon. Retourne les ops appliquées et leurs inverses.
fn replay_units(doc: &mut Document, origin: &Origin, units: &[&DraftUnit]) -> Result<(Vec<Op>, Vec<Op>), CommandError> {
    let mut ops = Vec::new();
    let mut inverse = Vec::new();
    for unit in units {
        let mut ids = unit.ids.clone();
        let mut refs = unit.refs.clone();
        let mut lowering = Lowering::new(doc, &mut ids, &mut refs, &unit.scope, origin);
        let failure = match lowering.lower(&unit.command) {
            Ok(())
                if lowering.ops.len() == unit.ops.len()
                    && lowering
                        .ops
                        .iter()
                        .zip(&unit.ops)
                        .all(|(a, b)| a.same_except_position(b)) =>
            {
                ops.append(&mut lowering.ops);
                inverse.append(&mut lowering.inverse);
                continue;
            }
            Ok(()) => "its changes differ without the rejected units".to_owned(),
            Err(error) => error.to_string(),
        };
        lowering.rollback();
        undo_all(doc, &inverse);
        return Err(CommandError::UnitDependency(format!(
            "unit {} (`{}`) builds on a rejected unit: {failure}",
            unit.id.0, unit.label
        )));
    }
    Ok((ops, inverse))
}

/// Rejoue des inverses à rebours (sans échec possible sur un état cohérent).
fn undo_all(doc: &mut Document, inverse: &[Op]) {
    for op in inverse.iter().rev() {
        // Inverses d'ops appliquées avec succès sur cet état : leur application réussit.
        let _ = op.apply(doc);
    }
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
        let first_unit = self.draft.as_ref().map_or(0, |d| d.units.len());
        for (index, command) in tx.commands.iter().enumerate() {
            // Contexte de l'abaissement, conservé pour rejouer la commande seule.
            let context = in_draft.then(|| (self.ids.clone(), refs.clone()));
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
            if let Some((ids, refs_before)) = context
                && !command_ops.is_empty()
            {
                units.push(DraftUnit {
                    id: ChangeUnitId((first_unit + units.len()) as u32),
                    label: command.name().to_owned(),
                    command: command.clone(),
                    scope: tx.scope.clone(),
                    ids,
                    refs: refs_before,
                    ops: command_ops.clone(),
                    inverse: command_inverse.clone(),
                });
            }
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
            draft.units.extend(units);
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
        if let Err(error) = apply_all(&mut self.doc, &reversed) {
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
        if let Err(error) = apply_all(&mut self.doc, &entry.ops) {
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
        let base_errors = error_counts(&validate(&self.doc));
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
    fn new_errors(&self, base: &ErrorCounts) -> Vec<Issue> {
        introduced_errors(base, validate(&self.doc))
    }

    /// Valide le brouillon en une seule entrée d'undo. Refusé (brouillon conservé) si le
    /// résultat introduit des erreurs bloquantes, ou si une unité acceptée dépend d'une unité
    /// rejetée (`UNIT_DEPENDENCY`).
    pub fn commit_draft(&mut self, accept: DraftAccept) -> Result<ChangeSet, CommandError> {
        let draft = self.draft.take().ok_or(CommandError::NoDraft)?;
        let all_ops: Vec<Op> = draft.units.iter().flat_map(|u| u.ops.iter().cloned()).collect();
        let all_inverse: Vec<Op> = draft.units.iter().flat_map(|u| u.inverse.iter().cloned()).collect();
        let (ops, inverse) = match &accept {
            DraftAccept::All => (all_ops.clone(), all_inverse.clone()),
            DraftAccept::Units(selected) => {
                let selected: BTreeSet<ChangeUnitId> = selected.iter().copied().collect();
                if let Err(error) = check_unit_dependencies(&draft.units, &selected) {
                    self.draft = Some(draft);
                    return Err(error);
                }
                undo_all(&mut self.doc, &all_inverse);
                let origin = Origin::Ai {
                    run_id: draft.run_id.clone(),
                };
                let chosen: Vec<&DraftUnit> = draft.units.iter().filter(|u| selected.contains(&u.id)).collect();
                match replay_units(&mut self.doc, &origin, &chosen) {
                    Ok(result) => result,
                    Err(error) => {
                        // Rétablit le brouillon complet.
                        let _ = apply_all(&mut self.doc, &all_ops);
                        self.draft = Some(draft);
                        return Err(error);
                    }
                }
            }
        };
        let errors = self.new_errors(&draft.base_errors);
        if !errors.is_empty() {
            if matches!(accept, DraftAccept::Units(_)) {
                undo_all(&mut self.doc, &inverse);
                let _ = apply_all(&mut self.doc, &all_ops);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn missing(what: &str) -> Issue {
        Issue::error(IssueCode::InvalidReference, None, format!("{what} does not exist"))
    }

    #[test]
    fn errors_are_compared_by_stable_identity() {
        let node: NodeId = "n_0000000000".parse().unwrap();
        let h1 = |page: &str| {
            Issue::error(
                IssueCode::A11yMultipleH1,
                Some(&node),
                format!("page `{page}` has more than one H1"),
            )
        };
        let contrast = |ratio: &str, threshold: &str, mode: &str| {
            Issue::error(
                IssueCode::A11yContrast,
                Some(&node),
                format!("text contrast {ratio}:1 is below {threshold}:1 ({mode} mode, from md)"),
            )
            .at(Breakpoint::Md)
        };

        // Page renommée, ratio de contraste changé : mêmes erreurs.
        let base = error_counts(&[h1("Accueil"), contrast("2.95", "4.5", "dark")]);
        assert!(introduced_errors(&base, vec![h1("Home"), contrast("3.10", "3", "dark")]).is_empty());
        // Même emplacement, autre mode de contraste : nouvelle erreur.
        let light = contrast("2.95", "4.5", "light");
        assert_eq!(introduced_errors(&base, vec![light.clone()]), vec![light]);

        // Même emplacement, autre référence manquante : nouvelle erreur, même si celle d'avant
        // est corrigée.
        let base = error_counts(&[missing("favicon asset `a_0000000000`")]);
        assert_eq!(
            introduced_errors(&base, vec![missing("favicon asset `a_1111111111`")]),
            vec![missing("favicon asset `a_1111111111`")]
        );
        // Un avertissement ne bloque jamais.
        let warning = Issue::warning(IssueCode::InvalidReference, None, "root node `n` does not exist");
        assert!(introduced_errors(&base, vec![missing("favicon asset `a_0000000000`"), warning]).is_empty());
        // Une seconde erreur sans nœud de même code reste nouvelle.
        let introduced = introduced_errors(
            &base,
            vec![
                missing("favicon asset `a_0000000000`"),
                missing("root node `n_0000000000`"),
            ],
        );
        assert_eq!(introduced, vec![missing("root node `n_0000000000`")]);
    }

    #[test]
    fn stable_messages_keep_identifiers_and_mask_names_and_measures() {
        assert_eq!(
            stable_message("anchor `contact` does not exist on page `À propos`"),
            "anchor `contact` does not exist on page ``"
        );
        assert_eq!(
            stable_message("component `Card` has no prop `title`"),
            "component `` has no prop `title`"
        );
        assert_eq!(
            stable_message("layout `site` must contain exactly one `page` slot (found 2)"),
            "layout `` must contain exactly one `page` slot (found #)"
        );
        assert_eq!(
            stable_message("color token `brand` does not exist"),
            "color token `brand` does not exist"
        );
        assert_eq!(
            stable_message("page `p_0000000000` does not exist"),
            "page `p_0000000000` does not exist"
        );
        assert_eq!(
            stable_message("`none` is only valid for max sizes, not `width`"),
            "`none` is only valid for max sizes, not `width`"
        );
    }
}
