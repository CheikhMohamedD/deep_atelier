//! Abaissement des commandes en ops primitives, appliquées au fil de l'eau.
//!
//! Chaque op est appliquée dès son émission (les commandes suivantes voient l'état courant) et
//! son inverse est conservé ; en cas d'erreur, l'appelant annule tout avec [`Lowering::rollback`].

use std::collections::{BTreeMap, BTreeSet};

use crate::command::command::{Command, CommandError};
use crate::command::neutral::neutral_value;
use crate::command::spec::{
    ActionSpec, ComponentPropSpec, ContainerSpec, KindSpec, MetaPatch, NodeSpec, PropsPatch, TokenEdit,
};
use crate::document::{
    BindableField, Component, ComponentProp, Document, Layout, Owner, Page, PropBinding, Seo, SiteSettings,
    VariantAxis, VariantOverride,
};
use crate::id::{ComponentId, IdGen, LayoutId, NodeId, NodeRef, PageId, TokenName};
use crate::node::{
    A11y, Action, ButtonProps, ContainerKind, ContainerProps, ContainerRole, DEFAULT_SLOT, Href, IconProps, ImageProps,
    ImageSource, InputProps, InputType, InstanceProps, LinkProps, Node, NodeKind, NodeMeta, PAGE_SLOT,
    PlatformOverrides, PlatformScope, PropValue, RawCodeProps, SlotProps, TextProps, TextRole, TextRun, WebOverrides,
};
use crate::op::Op;
use crate::query;
use crate::render::{PARENT_PROPS, style_prop_allowed};
use crate::scope::{Origin, Scope};
use crate::style::color::{ColorRef, ColorSource};
use crate::style::responsive::{Breakpoint, Responsive, ResponsivePatch};
use crate::style::style::{
    Background, InteractionState, PropChange, ResponsiveValue, ResponsiveValuePatch, Ring, StyleError, StyleProp,
};
use crate::style::values::{AspectRatio, Direction, GridColumns, RingWidth, Size};
use crate::tokens::{ColorToken, FontToken, RadiusToken, ShadowToken};

impl From<StyleError> for CommandError {
    fn from(error: StyleError) -> Self {
        CommandError::InvalidCommand(error.to_string())
    }
}

/// Résultat accumulé d'un abaissement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LowerOutput {
    /// `$ref` → id créé.
    pub created: BTreeMap<String, NodeId>,
    /// Racines des sous-arbres insérés (nœuds créés, copies, instances).
    pub inserted: Vec<NodeId>,
    pub pages: Vec<PageId>,
    pub layouts: Vec<LayoutId>,
    pub components: Vec<ComponentId>,
}

/// Contexte d'abaissement d'une transaction.
pub struct Lowering<'a> {
    pub doc: &'a mut Document,
    pub ids: &'a mut IdGen,
    /// Références locales visibles (transaction ou run IA).
    pub refs: &'a mut BTreeMap<String, NodeId>,
    pub scope: &'a Scope,
    pub origin: &'a Origin,
    pub ops: Vec<Op>,
    pub inverse: Vec<Op>,
    pub out: LowerOutput,
}

/// Propriétés propres à la position d'un nœud dans son parent : elles suivent l'instance quand
/// un nœud devient composant.
const PLACEMENT_PROPS: [StyleProp; 9] = [
    StyleProp::Grow,
    StyleProp::Shrink,
    StyleProp::AlignSelf,
    StyleProp::ColSpan,
    StyleProp::Order,
    StyleProp::MarginTop,
    StyleProp::MarginRight,
    StyleProp::MarginBottom,
    StyleProp::MarginLeft,
];

/// Propriétés propres au type de conteneur du nœud qui les porte.
const CONTAINER_PROPS: [StyleProp; 6] = [
    StyleProp::Direction,
    StyleProp::Wrap,
    StyleProp::Columns,
    StyleProp::Gap,
    StyleProp::Align,
    StyleProp::Justify,
];

/// Nœud référencé par un autre du même arbre : cible d'une action de bouton ou champ d'une
/// étiquette.
fn node_reference(kind: &NodeKind) -> Option<&NodeId> {
    match kind {
        NodeKind::Text(TextProps {
            role: TextRole::Label {
                for_input: Some(target),
            },
            ..
        })
        | NodeKind::Button(ButtonProps {
            action: Some(Action::ToggleVisibility { target }),
            ..
        }) => Some(target),
        _ => None,
    }
}

/// Noms des slots d'un composant (y compris ceux transmis à une instance imbriquée), comme la
/// validation les lit.
fn component_slots(doc: &Document, component: &Component) -> BTreeSet<String> {
    doc.subtree(&component.root)
        .iter()
        .filter_map(|n| match &doc.node(n)?.kind {
            NodeKind::Slot(slot) => Some(slot.name.clone()),
            _ => None,
        })
        .collect()
}

/// Références de couleur posées par un patch de valeur.
fn patch_color_refs(patch: &ResponsiveValuePatch) -> Vec<&ColorRef> {
    match patch {
        ResponsiveValuePatch::Color(p) => p.values(),
        ResponsiveValuePatch::Background(p) => p.values().into_iter().flat_map(Background::colors).collect(),
        ResponsiveValuePatch::Ring(p) => p.values().into_iter().map(|ring| &ring.color).collect(),
        _ => Vec::new(),
    }
}

fn is_pascal_case(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && name.chars().all(|c| c.is_ascii_alphanumeric())
}

fn is_prop_name(name: &str) -> bool {
    crate::id::is_camel_case(name)
}

/// Remplace dans un `kind` les références internes à un sous-arbre copié.
pub(crate) fn remap_kind(kind: &mut NodeKind, map: &BTreeMap<NodeId, NodeId>) {
    match kind {
        NodeKind::Text(TextProps {
            role: TextRole::Label {
                for_input: Some(target),
            },
            ..
        }) => {
            if let Some(new) = map.get(target) {
                *target = new.clone();
            }
        }
        NodeKind::Button(ButtonProps {
            action: Some(Action::ToggleVisibility { target }),
            ..
        }) => {
            if let Some(new) = map.get(target) {
                *target = new.clone();
            }
        }
        _ => {}
    }
}

/// Écrit la valeur d'une prop de composant dans le champ lié d'un nœud.
fn write_field(node: &mut Node, field: BindableField, value: &PropValue) {
    match (field, value, &mut node.kind) {
        (BindableField::Text, PropValue::Text(text), NodeKind::Text(props)) => {
            if props.plain_text() != *text {
                props.content = vec![TextRun::plain(text.clone())];
            }
        }
        (BindableField::ImageSource, PropValue::Image(source), NodeKind::Image(props)) => props.source = source.clone(),
        (BindableField::ImageAlt, PropValue::Text(alt), NodeKind::Image(props)) => props.alt = alt.clone(),
        (BindableField::Href, PropValue::Href(href), NodeKind::Link(props)) => props.href = href.clone(),
        (BindableField::Label, PropValue::Text(label), NodeKind::Button(props)) => props.label = Some(label.clone()),
        (BindableField::Label, PropValue::Text(label), NodeKind::Link(props)) => props.label = Some(label.clone()),
        (BindableField::Visible, PropValue::Bool(false), _) => node.visibility = Some(Responsive::new(false)),
        // Un nœud masqué partout redevient visible ; une visibilité responsive qui l'affiche
        // déjà à un breakpoint est conservée (même lecture que `BindableField::read`).
        (BindableField::Visible, PropValue::Bool(true), _)
            if node
                .visibility
                .as_ref()
                .is_some_and(|v| v.values().all(|visible| !*visible)) =>
        {
            node.visibility = None;
        }
        _ => {}
    }
}

fn apply_meta_patch(meta: &NodeMeta, a11y: &A11y, patch: &MetaPatch) -> (NodeMeta, A11y) {
    let mut meta = meta.clone();
    let mut a11y = a11y.clone();
    if let Some(name) = &patch.name {
        meta.name = name.clone();
    }
    if let Some(anchor) = &patch.anchor {
        meta.anchor = anchor.clone();
    }
    if let Some(locked) = patch.locked {
        meta.locked = locked;
    }
    if let Some(slot) = &patch.slot {
        meta.slot = slot.clone();
    }
    if let Some(label) = &patch.a11y_label {
        a11y.label = label.clone();
    }
    if let Some(hidden) = patch.a11y_hidden {
        a11y.hidden = hidden;
    }
    (meta, a11y)
}

fn props_patch_name(patch: &PropsPatch) -> &'static str {
    match patch {
        PropsPatch::Box { .. } => "Box",
        PropsPatch::Stack { .. } => "Stack",
        PropsPatch::Grid { .. } => "Grid",
        PropsPatch::Text { .. } => "Text",
        PropsPatch::Image { .. } => "Image",
        PropsPatch::Icon { .. } => "Icon",
        PropsPatch::Button { .. } => "Button",
        PropsPatch::Input { .. } => "Input",
        PropsPatch::Link { .. } => "Link",
        PropsPatch::ComponentInstance { .. } => "ComponentInstance",
        PropsPatch::Slot { .. } => "Slot",
        PropsPatch::RawCode { .. } => "RawCode",
    }
}

impl<'a> Lowering<'a> {
    pub fn new(
        doc: &'a mut Document,
        ids: &'a mut IdGen,
        refs: &'a mut BTreeMap<String, NodeId>,
        scope: &'a Scope,
        origin: &'a Origin,
    ) -> Self {
        Self {
            doc,
            ids,
            refs,
            scope,
            origin,
            ops: Vec::new(),
            inverse: Vec::new(),
            out: LowerOutput::default(),
        }
    }

    /// Annule toutes les ops émises (ordre inverse).
    pub fn rollback(&mut self) {
        for op in self.inverse.drain(..).rev() {
            // Les inverses viennent d'ops appliquées avec succès : leur application ne peut pas
            // échouer sur un document resté dans l'état qu'elles attendent.
            let _ = op.apply(self.doc);
        }
        self.ops.clear();
    }

    fn emit(&mut self, op: Op) -> Result<Op, CommandError> {
        let inverse = op.apply(self.doc)?;
        self.ops.push(op);
        self.inverse.push(inverse.clone());
        Ok(inverse)
    }

    fn node(&self, id: &NodeId) -> Result<&Node, CommandError> {
        self.doc
            .node(id)
            .ok_or_else(|| CommandError::NodeNotFound(id.to_string()))
    }

    fn resolve(&self, reference: &NodeRef) -> Result<NodeId, CommandError> {
        match reference {
            NodeRef::Id(id) => {
                self.node(id)?;
                Ok(id.clone())
            }
            NodeRef::Local(name) => {
                let id = self
                    .refs
                    .get(name)
                    .ok_or_else(|| CommandError::UnknownRef(name.clone()))?;
                self.node(id)?;
                Ok(id.clone())
            }
        }
    }

    fn new_node_id(&mut self, reserved: &BTreeSet<NodeId>) -> NodeId {
        let doc = &*self.doc;
        self.ids.node(|id| doc.nodes.contains_key(id) || reserved.contains(id))
    }

    fn out_of_scope(&self, command: &Command, reason: impl Into<String>) -> CommandError {
        CommandError::OutOfScope {
            command: command.name(),
            reason: reason.into(),
        }
    }

    fn check_in_scope(&self, command: &Command, id: &NodeId) -> Result<(), CommandError> {
        match &self.scope.subtree {
            Some(roots) if !roots.iter().any(|root| self.doc.is_within(id, root)) => {
                Err(self.out_of_scope(command, format!("node `{id}` is outside the selection")))
            }
            _ => Ok(()),
        }
    }

    fn check_command_scope(&self, command: &Command) -> Result<(), CommandError> {
        if self.scope.content_only && !matches!(command, Command::SetProps { .. }) {
            return Err(self.out_of_scope(command, "content mode only allows editing texts, images and links"));
        }
        if self.scope.breakpoint.is_some()
            && !matches!(command, Command::SetStyle { .. } | Command::SetVisibility { .. })
        {
            return Err(self.out_of_scope(command, "a breakpoint prompt only allows style and visibility changes"));
        }
        let targets_nodes = matches!(
            command,
            Command::InsertNodes { .. }
                | Command::ReplaceNode { .. }
                | Command::DeleteNodes { .. }
                | Command::MoveNode { .. }
                | Command::DuplicateNode { .. }
                | Command::WrapNodes { .. }
                | Command::UnwrapNode { .. }
                | Command::ConvertContainer { .. }
                | Command::SetProps { .. }
                | Command::SetStyle { .. }
                | Command::SetVisibility { .. }
                | Command::SetMeta { .. }
                | Command::SetPlatform { .. }
                | Command::SetWebOverrides { .. }
                | Command::CreateComponent { .. }
                | Command::DetachInstance { .. }
        );
        if self.scope.subtree.is_some() && !targets_nodes {
            return Err(self.out_of_scope(command, "a selection prompt only allows changes inside the selection"));
        }
        Ok(())
    }

    /// Abaisse une commande.
    pub fn lower(&mut self, command: &Command) -> Result<(), CommandError> {
        if self.origin.is_ai() && !command.allowed_for_ai() {
            return Err(CommandError::Forbidden(command.name().to_owned()));
        }
        self.check_command_scope(command)?;
        match command {
            Command::InsertNodes { parent, index, nodes } => {
                let parent = self.resolve(parent)?;
                self.check_in_scope(command, &parent)?;
                self.insert_specs(&parent, *index, nodes)
            }
            Command::ReplaceNode { node, nodes } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                let (parent, index) = self
                    .doc
                    .index_in_parent(&id)
                    .ok_or_else(|| CommandError::RootNode(id.clone()))?;
                // Les nouveaux nœuds sont écrits dans le parent : il doit lui aussi être sélectionné.
                self.check_in_scope(command, &parent)?;
                self.emit(Op::RemoveSubtree { root: id })?;
                self.insert_specs(&parent, Some(index as u32), nodes)
            }
            Command::DeleteNodes { nodes } => {
                let ids = nodes.iter().map(|r| self.resolve(r)).collect::<Result<Vec<_>, _>>()?;
                for id in &ids {
                    self.check_in_scope(command, id)?;
                    if self.node(id)?.parent.is_none() {
                        return Err(CommandError::RootNode(id.clone()));
                    }
                }
                let unique: BTreeSet<NodeId> = ids.into_iter().collect();
                let roots: Vec<NodeId> = unique
                    .iter()
                    .filter(|id| !self.doc.ancestors(id).iter().any(|a| unique.contains(a)))
                    .cloned()
                    .collect();
                for id in roots {
                    self.emit(Op::RemoveSubtree { root: id })?;
                }
                Ok(())
            }
            Command::MoveNode { node, parent, index } => {
                let id = self.resolve(node)?;
                let parent = self.resolve(parent)?;
                self.check_in_scope(command, &id)?;
                self.check_in_scope(command, &parent)?;
                let target = self.node(&parent)?;
                if !target.kind.accepts_children() {
                    return Err(CommandError::InvalidParent {
                        parent: parent.to_string(),
                        kind: target.kind.type_name(),
                    });
                }
                self.emit(Op::MoveNode {
                    node: id.clone(),
                    parent,
                    index: *index,
                })?;
                self.fit_to_parent(&id)
            }
            Command::DuplicateNode { node } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.duplicate(command, &id)
            }
            Command::WrapNodes { nodes, container } => self.wrap(command, nodes, container),
            Command::UnwrapNode { node } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.unwrap(command, &id)
            }
            Command::ConvertContainer { node, to } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.convert(&id, *to)
            }
            Command::SetProps { node, props } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.set_props(command, &id, props)
            }
            Command::SetStyle { node, state, style } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.apply_style(&id, *state, style.entries())
            }
            Command::SetVisibility { node, visibility } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.set_visibility(&id, visibility.as_ref())
            }
            Command::SetMeta { node, meta } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.set_meta(&id, meta)
            }
            Command::SetPlatform { node, scope } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                if self.node(&id)?.platform != *scope {
                    self.emit(Op::SetPlatform {
                        node: id,
                        scope: *scope,
                    })?;
                }
                Ok(())
            }
            Command::SetWebOverrides { node, overrides } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.set_web_overrides(&id, overrides)
            }
            Command::CreateComponent { node, name, props } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.create_component(command, &id, name, props)
            }
            Command::UpdateComponent {
                component,
                name,
                props,
                variants,
            } => self.update_component(component, name.as_deref(), props.as_deref(), variants.as_deref()),
            Command::DetachInstance { node } => {
                let id = self.resolve(node)?;
                self.check_in_scope(command, &id)?;
                self.detach_instance(command, &id)
            }
            Command::DeleteComponent { component } => {
                let found = self
                    .doc
                    .component(component)
                    .cloned()
                    .ok_or_else(|| CommandError::EntityNotFound(format!("component `{component}`")))?;
                if !self.doc.instances_of(component).is_empty() {
                    return Err(CommandError::InUse(format!("component `{}`", found.name)));
                }
                self.emit(Op::RemoveComponent { id: component.clone() })?;
                self.emit(Op::RemoveSubtree { root: found.root })?;
                Ok(())
            }
            Command::CreateLayout { name } => self.create_layout(name),
            Command::UpdateLayout { layout, name } => {
                let (index, mut found) = self.find_layout(layout)?;
                Self::check_name(name, "layout")?;
                found.name = name.clone();
                self.emit(Op::PutLayout { index, layout: found })?;
                Ok(())
            }
            Command::DeleteLayout { layout } => {
                let (_, found) = self.find_layout(layout)?;
                if self.doc.pages.iter().any(|p| p.layout.as_ref() == Some(layout)) {
                    return Err(CommandError::InUse(format!("layout `{}`", found.name)));
                }
                self.emit(Op::RemoveLayout { id: layout.clone() })?;
                self.emit(Op::RemoveSubtree { root: found.root })?;
                Ok(())
            }
            Command::CreatePage {
                name,
                route,
                layout,
                seo,
            } => self.create_page(name, route, layout.as_ref(), seo.as_ref()),
            Command::UpdatePage {
                page,
                name,
                route,
                layout,
                seo,
            } => {
                let index = self
                    .doc
                    .pages
                    .iter()
                    .position(|p| &p.id == page)
                    .ok_or_else(|| CommandError::EntityNotFound(format!("page `{page}`")))?;
                let mut next = self.doc.pages[index].clone();
                if let Some(name) = name {
                    Self::check_name(name, "page")?;
                    next.name = name.clone();
                }
                if let Some(route) = route {
                    next.route = route.clone();
                }
                if let Some(layout) = layout {
                    if let Some(id) = layout {
                        self.find_layout(id)?;
                    }
                    next.layout = layout.clone();
                }
                if let Some(seo) = seo {
                    next.seo = seo.clone();
                }
                if next != self.doc.pages[index] {
                    self.emit(Op::PutPage {
                        index: index as u32,
                        page: next,
                    })?;
                }
                Ok(())
            }
            Command::DeletePage { page } => {
                let found = self
                    .doc
                    .page(page)
                    .cloned()
                    .ok_or_else(|| CommandError::EntityNotFound(format!("page `{page}`")))?;
                if self.page_linked_from_elsewhere(page, &found.root) {
                    return Err(CommandError::InUse(format!("page `{}`", found.name)));
                }
                self.emit(Op::RemovePage { id: page.clone() })?;
                self.emit(Op::RemoveSubtree { root: found.root })?;
                Ok(())
            }
            Command::SetToken { edit } => self.set_token(edit),
            Command::UpdateSettings {
                lang,
                site_name,
                favicon,
            } => {
                let mut next: SiteSettings = self.doc.settings.clone();
                if let Some(lang) = lang {
                    next.lang = lang.clone();
                }
                if let Some(site_name) = site_name {
                    next.site_name = site_name.clone();
                }
                if let Some(favicon) = favicon {
                    if let Some(id) = favicon
                        && self.doc.asset(id).is_none()
                    {
                        return Err(CommandError::EntityNotFound(format!("asset `{id}`")));
                    }
                    next.favicon = favicon.clone();
                }
                if next != self.doc.settings {
                    self.emit(Op::SetSettings { settings: next })?;
                }
                Ok(())
            }
            Command::AddAsset { asset } => {
                if self.doc.asset(&asset.id).is_some() {
                    return Err(CommandError::InvalidCommand(format!(
                        "asset `{}` already exists",
                        asset.id
                    )));
                }
                let index = self.doc.assets.len() as u32;
                self.emit(Op::PutAsset {
                    index,
                    asset: asset.clone(),
                })?;
                Ok(())
            }
            Command::RemoveAsset { asset } => {
                if self.doc.asset(asset).is_none() {
                    return Err(CommandError::EntityNotFound(format!("asset `{asset}`")));
                }
                if self.doc.asset_in_use(asset) {
                    return Err(CommandError::InUse(format!("asset `{asset}`")));
                }
                self.emit(Op::RemoveAsset { id: asset.clone() })?;
                Ok(())
            }
        }
    }

    fn check_name(name: &str, what: &str) -> Result<(), CommandError> {
        if name.trim().is_empty() {
            Err(CommandError::InvalidCommand(format!("{what} name must not be empty")))
        } else {
            Ok(())
        }
    }

    fn find_layout(&self, id: &LayoutId) -> Result<(u32, Layout), CommandError> {
        self.doc
            .layouts
            .iter()
            .position(|l| &l.id == id)
            .map(|i| (i as u32, self.doc.layouts[i].clone()))
            .ok_or_else(|| CommandError::EntityNotFound(format!("layout `{id}`")))
    }

    /// Vrai si la page est la cible d'un lien hors de son propre arbre : nœud `Link`, défaut
    /// d'une prop de composant ou surcharge d'instance (les liens de la page partent avec elle).
    fn page_linked_from_elsewhere(&self, page: &PageId, page_root: &NodeId) -> bool {
        let targets = |value: &PropValue| matches!(value, PropValue::Href(Href::Page { page: p, .. }) if p == page);
        let outside = |id: &NodeId| !self.doc.is_within(id, page_root);
        self.doc.links_to_page(page).iter().any(outside)
            || self
                .doc
                .components
                .iter()
                .any(|c| c.props.iter().any(|p| targets(&p.default)))
            || self.doc.nodes.values().any(|node| match &node.kind {
                NodeKind::ComponentInstance(instance) => {
                    outside(&node.id) && instance.overrides.iter().any(|o| targets(&o.value))
                }
                _ => false,
            })
    }

    /// L'IA ne crée pas de `RawCode`, pas plus par copie (duplication, détachement) que par
    /// insertion (ADR 0001 § 14.11).
    fn check_raw_code_copy(&self, copied: &[NodeId]) -> Result<(), CommandError> {
        let has_raw_code = copied
            .iter()
            .any(|id| matches!(self.doc.node(id).map(|n| &n.kind), Some(NodeKind::RawCode(_))));
        if self.origin.is_ai() && has_raw_code {
            return Err(CommandError::Forbidden("RawCode creation".to_owned()));
        }
        Ok(())
    }

    /// Pose le slot ciblé par un nœud (enfant d'instance).
    fn set_slot(&mut self, id: &NodeId, slot: Option<String>) -> Result<(), CommandError> {
        let meta = self.node(id)?.meta.clone();
        if meta.slot != slot {
            self.emit(Op::SetMeta {
                node: id.clone(),
                meta: NodeMeta { slot, ..meta },
            })?;
        }
        Ok(())
    }

    /// Retire d'un nœud celles des propriétés `props` que son type ou ses hôtes au rendu
    /// n'autorisent pas.
    fn drop_disallowed_style(&mut self, id: &NodeId, props: &[StyleProp]) -> Result<(), CommandError> {
        let node = self.node(id)?;
        let dropped: Vec<StyleProp> = props
            .iter()
            .copied()
            .filter(|prop| node.style.get(*prop).is_ok_and(|v| v.is_some()))
            .filter(|prop| !style_prop_allowed(self.doc, *prop, node))
            .collect();
        for prop in dropped {
            self.apply_style(id, None, vec![(prop, PropChange::Remove)])?;
        }
        Ok(())
    }

    /// Adapte un nœud déplacé à son nouveau parent : slot ciblé effacé hors d'une instance, ou
    /// ramené au slot par défaut quand le composant de l'instance n'a pas ce slot ; puis
    /// propriétés de placement retirées si l'hôte au rendu (le parent, ou pour le contenu d'une
    /// instance le parent du slot ciblé) ne les autorise pas.
    fn fit_to_parent(&mut self, id: &NodeId) -> Result<(), CommandError> {
        let node = self.node(id)?;
        let instance = node
            .parent
            .as_ref()
            .and_then(|p| self.doc.node(p))
            .and_then(|p| match &p.kind {
                NodeKind::ComponentInstance(props) => Some(props.component.clone()),
                _ => None,
            });
        match instance {
            None => self.set_slot(id, None)?,
            Some(component) => {
                let slot = node.meta.slot.clone();
                let known = match (&slot, self.doc.component(&component)) {
                    (Some(name), Some(component)) => component_slots(self.doc, component).contains(name),
                    _ => true,
                };
                if !known {
                    self.set_slot(id, None)?;
                }
            }
        }
        self.drop_disallowed_style(id, &PARENT_PROPS)
    }

    // ---------------------------------------------------------------- création de nœuds

    fn register_ref(&mut self, name: &str, id: &NodeId) -> Result<(), CommandError> {
        if !NodeRef::is_local_name(name) {
            return Err(CommandError::InvalidRef(name.to_owned()));
        }
        if self.refs.contains_key(name) {
            return Err(CommandError::DuplicateRef(name.to_owned()));
        }
        self.refs.insert(name.to_owned(), id.clone());
        self.out.created.insert(name.to_owned(), id.clone());
        Ok(())
    }

    /// Résout une référence de commande en tenant compte des refs de la commande en cours.
    fn resolve_with(&self, reference: &NodeRef, local: &BTreeMap<String, NodeId>) -> Result<NodeId, CommandError> {
        match reference {
            NodeRef::Local(name) if local.contains_key(name) => Ok(local[name].clone()),
            other => self.resolve(other),
        }
    }

    fn build_kind(&self, spec: &KindSpec, local: &BTreeMap<String, NodeId>) -> Result<NodeKind, CommandError> {
        let container = |role: &Option<ContainerRole>| ContainerProps {
            role: role.clone().unwrap_or_default(),
        };
        Ok(match spec {
            KindSpec::Box { role } => NodeKind::Box(container(role)),
            KindSpec::Stack { role } => NodeKind::Stack(container(role)),
            KindSpec::Grid { role } => NodeKind::Grid(container(role)),
            KindSpec::Text {
                role,
                content,
                for_input,
            } => {
                let role = match for_input {
                    Some(target) => TextRole::Label {
                        for_input: Some(self.resolve_with(target, local)?),
                    },
                    None => role.clone().unwrap_or(TextRole::Paragraph),
                };
                NodeKind::Text(TextProps {
                    role,
                    content: content.clone(),
                })
            }
            KindSpec::Image {
                source,
                alt,
                intrinsic,
                priority,
            } => NodeKind::Image(ImageProps {
                source: source.clone().unwrap_or_else(|| ImageSource::Placeholder {
                    label: if alt.is_empty() {
                        "Image".to_owned()
                    } else {
                        alt.clone()
                    },
                    ratio: AspectRatio::Landscape,
                }),
                alt: alt.clone(),
                intrinsic: *intrinsic,
                priority: *priority,
            }),
            KindSpec::Icon { name } => NodeKind::Icon(IconProps { name: name.clone() }),
            KindSpec::Button {
                label,
                button_type,
                disabled,
                action,
            } => NodeKind::Button(ButtonProps {
                label: label.clone(),
                button_type: button_type.unwrap_or_default(),
                disabled: *disabled,
                action: match action {
                    Some(ActionSpec::ToggleVisibility { target }) => Some(Action::ToggleVisibility {
                        target: self.resolve_with(target, local)?,
                    }),
                    None => None,
                },
            }),
            KindSpec::Input {
                input_type,
                name,
                placeholder,
                required,
                autocomplete,
            } => {
                let input_type = input_type.unwrap_or_default();
                let default_name = match input_type {
                    InputType::Multiline => "message",
                    InputType::Text => "text",
                    InputType::Email => "email",
                    InputType::Password => "password",
                    InputType::Number => "number",
                    InputType::Tel => "tel",
                    InputType::Url => "url",
                    InputType::Search => "search",
                };
                NodeKind::Input(InputProps {
                    input_type,
                    name: name.clone().unwrap_or_else(|| default_name.to_owned()),
                    placeholder: placeholder.clone(),
                    required: *required,
                    autocomplete: *autocomplete,
                })
            }
            KindSpec::Link { href, label, new_tab } => NodeKind::Link(LinkProps {
                href: href.clone(),
                label: label.clone(),
                new_tab: *new_tab,
            }),
            KindSpec::ComponentInstance {
                component,
                overrides,
                variants,
            } => {
                if self.doc.component(component).is_none() {
                    return Err(CommandError::EntityNotFound(format!("component `{component}`")));
                }
                NodeKind::ComponentInstance(InstanceProps {
                    component: component.clone(),
                    overrides: overrides.clone(),
                    variants: variants.clone(),
                })
            }
            KindSpec::Slot { name } => {
                let name = name.clone().unwrap_or_else(|| DEFAULT_SLOT.to_owned());
                // Le nom devient une prop TypeScript du composant.
                if !is_prop_name(&name) {
                    return Err(CommandError::InvalidCommand(format!(
                        "slot name `{name}` must be camelCase"
                    )));
                }
                NodeKind::Slot(SlotProps { name })
            }
            KindSpec::RawCode { code, imports, client } => {
                if self.origin.is_ai() {
                    return Err(CommandError::Forbidden("RawCode creation".to_owned()));
                }
                NodeKind::RawCode(RawCodeProps {
                    code: code.clone(),
                    imports: imports.clone(),
                    client: *client,
                })
            }
        })
    }

    /// Couleur de l'anneau de focus par défaut : token `ring`, sinon `primary`, sinon noir.
    fn default_ring(&self) -> Ring {
        let color = ["ring", "primary"]
            .iter()
            .filter_map(|name| name.parse::<TokenName>().ok())
            .find(|name| self.doc.tokens.color(name).is_some())
            .map(ColorRef::token)
            .unwrap_or_else(|| ColorRef::source(ColorSource::Black));
        Ring {
            width: RingWidth::W2,
            color,
            offset: RingWidth::W2,
        }
    }

    /// Défauts explicites d'un nœud créé (principe 3 de l'ADR : rien d'implicite dans le compilateur).
    fn apply_defaults(&mut self, id: &NodeId) -> Result<(), CommandError> {
        let node = self.node(id)?.clone();
        let mut defaults: Vec<(Option<InteractionState>, StyleProp, ResponsiveValue)> = Vec::new();
        match &node.kind {
            NodeKind::Stack(_) if node.style.direction.is_none() => {
                defaults.push((
                    None,
                    StyleProp::Direction,
                    ResponsiveValue::Direction(Responsive::new(Direction::Column)),
                ));
            }
            NodeKind::Grid(_) if node.style.columns.is_none() => {
                defaults.push((
                    None,
                    StyleProp::Columns,
                    ResponsiveValue::GridColumns(Responsive::new(GridColumns::C1)),
                ));
            }
            NodeKind::Button(_) | NodeKind::Link(_) | NodeKind::Input(_)
                if node.states.focus_visible.ring.is_none() =>
            {
                defaults.push((
                    Some(InteractionState::FocusVisible),
                    StyleProp::Ring,
                    ResponsiveValue::Ring(Responsive::new(self.default_ring())),
                ));
            }
            _ => {}
        }
        for (state, prop, value) in defaults {
            self.emit(Op::SetStyleProp {
                node: id.clone(),
                state,
                prop,
                value: Some(value),
            })?;
        }
        Ok(())
    }

    fn insert_specs(&mut self, parent: &NodeId, index: Option<u32>, specs: &[NodeSpec]) -> Result<(), CommandError> {
        if specs.is_empty() {
            return Err(CommandError::InvalidCommand("`nodes` must not be empty".to_owned()));
        }
        let parent_node = self.node(parent)?;
        if !parent_node.kind.accepts_children() {
            return Err(CommandError::InvalidParent {
                parent: parent.to_string(),
                kind: parent_node.kind.type_name(),
            });
        }
        let len = parent_node.children.len();
        let start = index.map_or(len, |i| i as usize);
        if start > len {
            return Err(CommandError::InvalidCommand(format!(
                "index {start} out of bounds (parent has {len} children)"
            )));
        }

        // 1. Ids et références locales.
        let mut reserved = BTreeSet::new();
        let mut ids = Vec::with_capacity(specs.len());
        let mut local: BTreeMap<String, NodeId> = BTreeMap::new();
        let mut local_index: BTreeMap<String, usize> = BTreeMap::new();
        for (i, spec) in specs.iter().enumerate() {
            let id = self.new_node_id(&reserved);
            reserved.insert(id.clone());
            if let Some(name) = &spec.r#ref {
                if !NodeRef::is_local_name(name) {
                    return Err(CommandError::InvalidRef(name.clone()));
                }
                if self.refs.contains_key(name) || local.contains_key(name) {
                    return Err(CommandError::DuplicateRef(name.clone()));
                }
                local.insert(name.clone(), id.clone());
                local_index.insert(name.clone(), i);
            }
            ids.push(id);
        }

        // 2. Nœuds.
        let source = self.origin.node_source();
        let mut parents: Vec<Option<usize>> = Vec::with_capacity(specs.len());
        let mut built: Vec<Node> = Vec::with_capacity(specs.len());
        for (i, spec) in specs.iter().enumerate() {
            let parent_index = match &spec.parent {
                None => None,
                Some(name) => match local_index.get(name) {
                    Some(&p) if p < i => Some(p),
                    _ => {
                        return Err(CommandError::UnknownRef(format!(
                            "{name} (a NodeSpec parent must be the `ref` of a previous NodeSpec of the same command)"
                        )));
                    }
                },
            };
            if let Some(p) = parent_index
                && !built[p].kind.accepts_children()
            {
                return Err(CommandError::InvalidParent {
                    parent: name_of(&specs[p], &ids[p]),
                    kind: built[p].kind.type_name(),
                });
            }
            let kind = self.build_kind(&spec.kind, &local)?;
            let parent_id = parent_index.map_or_else(|| parent.clone(), |p| ids[p].clone());
            let mut node = Node::new(ids[i].clone(), Some(parent_id), kind);
            node.meta.source = source;
            if matches!(node.kind, NodeKind::Icon(_)) {
                node.a11y.hidden = true;
            }
            if let Some(patch) = &spec.meta {
                let (meta, a11y) = apply_meta_patch(&node.meta, &node.a11y, patch);
                node.meta = meta;
                node.a11y = a11y;
                if matches!(node.kind, NodeKind::Icon(_)) && node.a11y.label.is_some() && patch.a11y_hidden.is_none() {
                    node.a11y.hidden = false;
                }
            }
            if let Some(p) = parent_index {
                built[p].children.push(ids[i].clone());
            }
            parents.push(parent_index);
            built.push(node);
        }

        // 3. Un sous-arbre par nœud de premier niveau.
        let mut top_of: Vec<usize> = Vec::with_capacity(specs.len());
        for (i, parent_index) in parents.iter().enumerate() {
            top_of.push(parent_index.map_or(i, |p| top_of[p]));
        }
        let tops: Vec<usize> = (0..specs.len()).filter(|i| parents[*i].is_none()).collect();
        for (k, top) in tops.iter().enumerate() {
            let nodes: Vec<Node> = (0..specs.len())
                .filter(|i| top_of[*i] == *top)
                .map(|i| built[i].clone())
                .collect();
            self.emit(Op::InsertSubtree {
                parent: Some(parent.clone()),
                index: (start + k) as u32,
                nodes,
            })?;
            self.out.inserted.push(ids[*top].clone());
        }
        for (name, id) in &local {
            self.register_ref(name, id)?;
        }

        // 4. Défauts explicites puis styles demandés.
        for (i, spec) in specs.iter().enumerate() {
            let id = ids[i].clone();
            self.apply_defaults(&id)?;
            if let Some(style) = &spec.style {
                self.apply_style(&id, None, style.entries())?;
            }
            if let Some(hover) = &spec.hover {
                self.apply_style(&id, Some(InteractionState::Hover), hover.entries())?;
            }
            if let Some(focus) = &spec.focus_visible {
                self.apply_style(&id, Some(InteractionState::FocusVisible), focus.entries())?;
            }
            if let Some(visibility) = &spec.visibility {
                self.set_visibility(&id, Some(visibility))?;
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- style, visibilité, méta

    fn apply_style(
        &mut self,
        node: &NodeId,
        state: Option<InteractionState>,
        entries: Vec<(StyleProp, PropChange)>,
    ) -> Result<(), CommandError> {
        for (prop, change) in entries {
            if state.is_some() && !prop.in_states() {
                return Err(CommandError::InvalidCommand(format!(
                    "`{}` cannot be set in an interaction state",
                    prop.name()
                )));
            }
            if state.is_none() && !prop.in_base() {
                return Err(CommandError::InvalidCommand(format!(
                    "`{}` requires an interaction state",
                    prop.name()
                )));
            }
            self.check_breakpoint_scope(change.touched())?;
            let current = {
                let target = self.node(node)?;
                match state {
                    None => target.style.get(prop)?,
                    Some(state) => target.states.get(state).get(prop)?,
                }
            };
            let next = match &change {
                PropChange::Remove => None,
                PropChange::Merge(patch) => {
                    let doc = &*self.doc;
                    patch.apply(current.clone(), || neutral_value(doc, node, state, prop))?
                }
            };
            if next != current {
                self.emit(Op::SetStyleProp {
                    node: node.clone(),
                    state,
                    prop,
                    value: next,
                })?;
            }
        }
        Ok(())
    }

    fn check_breakpoint_scope(&self, touched: Option<Vec<Breakpoint>>) -> Result<(), CommandError> {
        let Some(bp) = self.scope.breakpoint else { return Ok(()) };
        let outside = match touched {
            None => true,
            Some(touched) => touched.iter().any(|b| *b != bp),
        };
        if outside {
            Err(CommandError::OutOfScope {
                command: "set_style",
                reason: format!("only `{}` values may change", bp.as_str()),
            })
        } else {
            Ok(())
        }
    }

    fn set_visibility(&mut self, id: &NodeId, patch: Option<&ResponsivePatch<bool>>) -> Result<(), CommandError> {
        self.check_breakpoint_scope(patch.map(ResponsivePatch::touched))?;
        let current = self.node(id)?.visibility.clone();
        let next = patch.and_then(|p| p.apply(current.clone(), || true));
        if next != current {
            self.emit(Op::SetVisibility {
                node: id.clone(),
                visibility: next,
            })?;
        }
        Ok(())
    }

    fn set_meta(&mut self, id: &NodeId, patch: &MetaPatch) -> Result<(), CommandError> {
        let node = self.node(id)?;
        let (meta, a11y) = apply_meta_patch(&node.meta, &node.a11y, patch);
        if meta != node.meta {
            self.emit(Op::SetMeta { node: id.clone(), meta })?;
        }
        if a11y != self.node(id)?.a11y {
            self.emit(Op::SetA11y { node: id.clone(), a11y })?;
        }
        Ok(())
    }

    fn set_web_overrides(&mut self, id: &NodeId, overrides: &WebOverrides) -> Result<(), CommandError> {
        for class in &overrides.extra_classes {
            if class.is_empty() || class.chars().any(char::is_whitespace) {
                return Err(CommandError::InvalidCommand(format!("invalid class `{class}`")));
            }
        }
        for attribute in &overrides.extra_attributes {
            let name = attribute.name.as_str();
            let allowed = (name.starts_with("data-") || name.starts_with("aria-"))
                && name.len() > 5
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !allowed {
                return Err(CommandError::InvalidCommand(format!(
                    "attribute `{name}` is not allowed (only data-* and aria-*)"
                )));
            }
        }
        let next = PlatformOverrides {
            web: if overrides.is_empty() {
                None
            } else {
                Some(overrides.clone())
            },
        };
        if next != self.node(id)?.platform_overrides {
            self.emit(Op::SetOverrides {
                node: id.clone(),
                overrides: next,
            })?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------- structure

    fn duplicate(&mut self, command: &Command, id: &NodeId) -> Result<(), CommandError> {
        let (parent, index) = self
            .doc
            .index_in_parent(id)
            .ok_or_else(|| CommandError::RootNode(id.clone()))?;
        // La copie est insérée dans le parent : il doit lui aussi être sélectionné.
        self.check_in_scope(command, &parent)?;
        let subtree = self.doc.subtree(id);
        self.check_raw_code_copy(&subtree)?;
        let mut reserved = BTreeSet::new();
        let mut map = BTreeMap::new();
        for old in &subtree {
            let new = self.new_node_id(&reserved);
            reserved.insert(new.clone());
            map.insert(old.clone(), new);
        }
        let nodes = clone_subtree(self.doc, &subtree, &map, Some(parent.clone()), true);
        self.emit(Op::InsertSubtree {
            parent: Some(parent),
            index: index as u32 + 1,
            nodes,
        })?;
        self.out.inserted.push(map[id].clone());
        Ok(())
    }

    fn wrap(&mut self, command: &Command, nodes: &[NodeRef], spec: &ContainerSpec) -> Result<(), CommandError> {
        if nodes.is_empty() {
            return Err(CommandError::InvalidCommand("`nodes` must not be empty".to_owned()));
        }
        let ids: BTreeSet<NodeId> = nodes.iter().map(|r| self.resolve(r)).collect::<Result<_, _>>()?;
        let mut positions = Vec::new();
        let mut parent: Option<NodeId> = None;
        for id in &ids {
            self.check_in_scope(command, id)?;
            let (p, i) = self
                .doc
                .index_in_parent(id)
                .ok_or_else(|| CommandError::RootNode(id.clone()))?;
            if parent.as_ref().is_some_and(|known| *known != p) {
                return Err(CommandError::InvalidCommand(
                    "wrapped nodes must be siblings".to_owned(),
                ));
            }
            parent = Some(p);
            positions.push((i, id.clone()));
        }
        positions.sort();
        let first = positions[0].0;
        if positions.last().map(|(i, _)| *i) != Some(first + positions.len() - 1) {
            return Err(CommandError::InvalidCommand(
                "wrapped nodes must be contiguous".to_owned(),
            ));
        }
        let parent = parent.ok_or_else(|| CommandError::InvalidCommand("no node to wrap".to_owned()))?;
        // Le conteneur est inséré dans le parent : il doit lui aussi être sélectionné.
        self.check_in_scope(command, &parent)?;
        let container_id = self.new_node_id(&BTreeSet::new());
        let props = ContainerProps {
            role: spec.role.clone().unwrap_or_default(),
        };
        let mut container = Node::new(
            container_id.clone(),
            Some(parent.clone()),
            NodeKind::from_container(spec.kind, props),
        );
        container.meta.source = self.origin.node_source();
        if matches!(self.node(&parent)?.kind, NodeKind::ComponentInstance(_)) {
            // Dans une instance, le conteneur reprend le slot ciblé par les nœuds enveloppés.
            let mut slots = BTreeSet::new();
            for (_, id) in &positions {
                let slot = self.node(id)?.meta.slot.clone();
                slots.insert(slot.unwrap_or_else(|| DEFAULT_SLOT.to_owned()));
            }
            if slots.len() > 1 {
                return Err(CommandError::InvalidCommand(
                    "wrapped nodes target different slots".to_owned(),
                ));
            }
            container.meta.slot = slots.into_iter().find(|slot| slot != DEFAULT_SLOT);
        }
        if let Some(patch) = &spec.meta {
            let (meta, a11y) = apply_meta_patch(&container.meta, &container.a11y, patch);
            container.meta = meta;
            container.a11y = a11y;
        }
        // Le conteneur prend la place des nœuds enveloppés, pour que la mise en page ne bouge
        // pas (comme `create_component`, ADR § 14.9) : un nœud seul lui cède tout son placement ;
        // plusieurs nœuds lui cèdent le placement qu'ils partagent, marges exceptées (elles
        // espacent les nœuds entre eux).
        let single = positions.len() == 1;
        let shared: &[StyleProp] = if single { &PLACEMENT_PROPS } else { &PARENT_PROPS };
        for prop in shared.iter().copied() {
            let mut values = Vec::with_capacity(positions.len());
            for (_, id) in &positions {
                values.push(self.node(id)?.style.get(prop)?);
            }
            let Some(Some(value)) = values.first().cloned() else {
                continue;
            };
            if values.iter().any(|v| v.as_ref() != Some(&value)) {
                continue;
            }
            container.style.set(prop, Some(value))?;
            if single {
                self.apply_style(&positions[0].1, None, vec![(prop, PropChange::Remove)])?;
            }
        }
        self.emit(Op::InsertSubtree {
            parent: Some(parent),
            index: first as u32,
            nodes: vec![container],
        })?;
        for (k, (_, id)) in positions.iter().enumerate() {
            self.emit(Op::MoveNode {
                node: id.clone(),
                parent: container_id.clone(),
                index: k as u32,
            })?;
            self.fit_to_parent(id)?;
        }
        if let Some(name) = &spec.r#ref {
            self.register_ref(name, &container_id)?;
        }
        self.out.inserted.push(container_id.clone());
        self.apply_defaults(&container_id)?;
        if let Some(style) = &spec.style {
            self.apply_style(&container_id, None, style.entries())?;
        }
        Ok(())
    }

    fn unwrap(&mut self, command: &Command, id: &NodeId) -> Result<(), CommandError> {
        let node = self.node(id)?.clone();
        if node.kind.container().is_none() {
            return Err(CommandError::KindMismatch {
                expected: "container",
                found: node.kind.type_name(),
            });
        }
        let (parent, index) = self
            .doc
            .index_in_parent(id)
            .ok_or_else(|| CommandError::RootNode(id.clone()))?;
        // Les enfants sont déplacés dans le parent : il doit lui aussi être sélectionné.
        self.check_in_scope(command, &parent)?;
        let into_instance = matches!(self.node(&parent)?.kind, NodeKind::ComponentInstance(_));
        for (k, child) in node.children.iter().enumerate() {
            self.emit(Op::MoveNode {
                node: child.clone(),
                parent: parent.clone(),
                index: (index + k) as u32,
            })?;
            if into_instance {
                // Les enfants reprennent le slot ciblé par le conteneur retiré.
                self.set_slot(child, node.meta.slot.clone())?;
            }
            self.fit_to_parent(child)?;
        }
        self.emit(Op::RemoveSubtree { root: id.clone() })?;
        Ok(())
    }

    fn convert(&mut self, id: &NodeId, to: ContainerKind) -> Result<(), CommandError> {
        let node = self.node(id)?.clone();
        let (from, props) = node.kind.container().ok_or(CommandError::KindMismatch {
            expected: "container",
            found: node.kind.type_name(),
        })?;
        if from == to {
            return Ok(());
        }
        self.emit(Op::SetKind {
            node: id.clone(),
            kind: NodeKind::from_container(to, props.clone()),
        })?;
        self.drop_disallowed_style(id, &CONTAINER_PROPS)?;
        for child in &node.children {
            self.drop_disallowed_style(child, &PARENT_PROPS)?;
        }
        self.apply_defaults(id)
    }

    fn set_props(&mut self, command: &Command, id: &NodeId, patch: &PropsPatch) -> Result<(), CommandError> {
        let node = self.node(id)?.clone();
        let content_only = self.scope.content_only;
        let guard = |present: bool, field: &str| -> Result<(), CommandError> {
            if content_only && present {
                Err(CommandError::OutOfScope {
                    command: command.name(),
                    reason: format!("`{field}` cannot change in content mode"),
                })
            } else {
                Ok(())
            }
        };
        let mismatch = || CommandError::KindMismatch {
            expected: props_patch_name(patch),
            found: node.kind.type_name(),
        };
        let next = match (&node.kind, patch) {
            (NodeKind::Box(p), PropsPatch::Box { role })
            | (NodeKind::Stack(p), PropsPatch::Stack { role })
            | (NodeKind::Grid(p), PropsPatch::Grid { role }) => {
                guard(role.is_some(), "role")?;
                let props = ContainerProps {
                    role: role.clone().unwrap_or_else(|| p.role.clone()),
                };
                let (kind, _) = node.kind.container().ok_or_else(mismatch)?;
                NodeKind::from_container(kind, props)
            }
            (NodeKind::Text(p), PropsPatch::Text { role, content }) => {
                guard(role.is_some(), "role")?;
                NodeKind::Text(TextProps {
                    role: role.clone().unwrap_or_else(|| p.role.clone()),
                    content: content.clone().unwrap_or_else(|| p.content.clone()),
                })
            }
            (
                NodeKind::Image(p),
                PropsPatch::Image {
                    source,
                    alt,
                    intrinsic,
                    priority,
                },
            ) => {
                guard(priority.is_some(), "priority")?;
                NodeKind::Image(ImageProps {
                    source: source.clone().unwrap_or_else(|| p.source.clone()),
                    alt: alt.clone().unwrap_or_else(|| p.alt.clone()),
                    intrinsic: intrinsic.unwrap_or(p.intrinsic),
                    priority: priority.unwrap_or(p.priority),
                })
            }
            (NodeKind::Icon(p), PropsPatch::Icon { name }) => {
                guard(name.is_some(), "name")?;
                NodeKind::Icon(IconProps {
                    name: name.clone().unwrap_or_else(|| p.name.clone()),
                })
            }
            (
                NodeKind::Button(p),
                PropsPatch::Button {
                    label,
                    button_type,
                    disabled,
                    action,
                },
            ) => {
                guard(button_type.is_some(), "button_type")?;
                guard(disabled.is_some(), "disabled")?;
                guard(action.is_some(), "action")?;
                let action = match action {
                    None => p.action.clone(),
                    Some(None) => None,
                    Some(Some(ActionSpec::ToggleVisibility { target })) => Some(Action::ToggleVisibility {
                        target: self.resolve(target)?,
                    }),
                };
                NodeKind::Button(ButtonProps {
                    label: label.clone().unwrap_or_else(|| p.label.clone()),
                    button_type: button_type.unwrap_or(p.button_type),
                    disabled: disabled.unwrap_or(p.disabled),
                    action,
                })
            }
            (
                NodeKind::Input(p),
                PropsPatch::Input {
                    input_type,
                    name,
                    placeholder,
                    required,
                    autocomplete,
                },
            ) => {
                guard(
                    input_type.is_some() || name.is_some() || required.is_some() || autocomplete.is_some(),
                    "input",
                )?;
                guard(placeholder.is_some(), "placeholder")?;
                NodeKind::Input(InputProps {
                    input_type: input_type.unwrap_or(p.input_type),
                    name: name.clone().unwrap_or_else(|| p.name.clone()),
                    placeholder: placeholder.clone().unwrap_or_else(|| p.placeholder.clone()),
                    required: required.unwrap_or(p.required),
                    autocomplete: autocomplete.unwrap_or(p.autocomplete),
                })
            }
            (NodeKind::Link(p), PropsPatch::Link { href, label, new_tab }) => {
                guard(new_tab.is_some(), "new_tab")?;
                NodeKind::Link(LinkProps {
                    href: href.clone().unwrap_or_else(|| p.href.clone()),
                    label: label.clone().unwrap_or_else(|| p.label.clone()),
                    new_tab: new_tab.unwrap_or(p.new_tab),
                })
            }
            (NodeKind::ComponentInstance(p), PropsPatch::ComponentInstance { overrides, variants }) => {
                guard(overrides.is_some() || variants.is_some(), "instance")?;
                NodeKind::ComponentInstance(InstanceProps {
                    component: p.component.clone(),
                    overrides: overrides.clone().unwrap_or_else(|| p.overrides.clone()),
                    variants: variants.clone().unwrap_or_else(|| p.variants.clone()),
                })
            }
            (NodeKind::Slot(p), PropsPatch::Slot { name }) => {
                guard(name.is_some(), "name")?;
                NodeKind::Slot(SlotProps {
                    name: name.clone().unwrap_or_else(|| p.name.clone()),
                })
            }
            (NodeKind::RawCode(p), PropsPatch::RawCode { code, imports, client }) => {
                if self.origin.is_ai() {
                    return Err(CommandError::Forbidden("RawCode edition".to_owned()));
                }
                guard(true, "code")?;
                NodeKind::RawCode(RawCodeProps {
                    code: code.clone().unwrap_or_else(|| p.code.clone()),
                    imports: imports.clone().unwrap_or_else(|| p.imports.clone()),
                    client: client.unwrap_or(p.client),
                })
            }
            _ => return Err(mismatch()),
        };
        if next != node.kind {
            self.emit(Op::SetKind {
                node: id.clone(),
                kind: next,
            })?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------- composants

    fn component_props(&self, root: &NodeId, specs: &[ComponentPropSpec]) -> Result<Vec<ComponentProp>, CommandError> {
        let mut names = BTreeSet::new();
        let mut props = Vec::with_capacity(specs.len());
        for spec in specs {
            if !is_prop_name(&spec.name) {
                return Err(CommandError::InvalidCommand(format!(
                    "prop name `{}` must be camelCase",
                    spec.name
                )));
            }
            if !names.insert(spec.name.clone()) {
                return Err(CommandError::InvalidCommand(format!("duplicate prop `{}`", spec.name)));
            }
            let node = self.resolve(&spec.node)?;
            if !self.doc.is_within(&node, root) {
                return Err(CommandError::InvalidCommand(format!(
                    "prop `{}` targets a node outside the component",
                    spec.name
                )));
            }
            let current = spec
                .field
                .read(self.node(&node)?)
                .ok_or_else(|| CommandError::InvalidCommand(format!("node `{node}` has no {:?} field", spec.field)))?;
            let default = spec.default.clone().unwrap_or(current);
            if !spec.field.accepts(&default) {
                return Err(CommandError::InvalidCommand(format!(
                    "default of prop `{}` has the wrong type",
                    spec.name
                )));
            }
            props.push(ComponentProp {
                name: spec.name.clone(),
                default,
                binding: PropBinding {
                    node,
                    field: spec.field,
                },
            });
        }
        Ok(props)
    }

    fn check_component_name(&self, name: &str, except: Option<&ComponentId>) -> Result<(), CommandError> {
        if !is_pascal_case(name) {
            return Err(CommandError::InvalidCommand(format!(
                "component name `{name}` must be PascalCase"
            )));
        }
        if self
            .doc
            .components
            .iter()
            .any(|c| c.name == name && Some(&c.id) != except)
        {
            return Err(CommandError::InvalidCommand(format!(
                "component `{name}` already exists"
            )));
        }
        Ok(())
    }

    /// Refuse d'extraire un sous-arbre dont l'arbre d'origine dépend : slots du layout ou du
    /// composant englobant, références (action de bouton, champ d'étiquette) qui franchissent la
    /// frontière du sous-arbre dans un sens ou dans l'autre, cibles des props ou des variantes
    /// du composant englobant.
    fn check_extractable(&self, id: &NodeId) -> Result<(), CommandError> {
        let subtree: BTreeSet<NodeId> = self.doc.subtree(id).into_iter().collect();
        let owner = self.doc.owner_of(id);
        if matches!(owner, Some(Owner::Layout(_) | Owner::Component(_)))
            && let Some(slot) = subtree
                .iter()
                .find(|n| matches!(self.doc.node(n).map(|n| &n.kind), Some(NodeKind::Slot(_))))
        {
            return Err(CommandError::InvalidCommand(format!(
                "slot `{slot}` belongs to the enclosing layout or component and cannot be moved into a new component"
            )));
        }
        // Le sous-arbre change d'arbre : une référence entre lui et le reste de son arbre
        // d'origine ne serait plus résolue (cible hors de l'arbre).
        let tree: BTreeSet<NodeId> = self.doc.subtree(&self.doc.tree_root(id)).into_iter().collect();
        for source in &tree {
            let Some(target) = self.doc.node(source).and_then(|n| node_reference(&n.kind)) else {
                continue;
            };
            if tree.contains(target) && subtree.contains(source) != subtree.contains(target) {
                return Err(CommandError::InvalidCommand(format!(
                    "node `{source}` references `{target}` across the boundary of the new component"
                )));
            }
        }
        let Some(Owner::Component(owner)) = owner else {
            return Ok(());
        };
        let Some(component) = self.doc.component(&owner) else {
            return Ok(());
        };
        if let Some(prop) = component.props.iter().find(|p| subtree.contains(&p.binding.node)) {
            return Err(CommandError::InvalidCommand(format!(
                "node `{}` is bound to prop `{}` of component `{}`",
                prop.binding.node, prop.name, component.name
            )));
        }
        for axis in &component.variants {
            for option in &axis.options {
                if let Some(over) = option.overrides.iter().find(|o| subtree.contains(&o.node)) {
                    return Err(CommandError::InvalidCommand(format!(
                        "node `{}` is targeted by variant `{}={}` of component `{}`",
                        over.node, axis.name, option.name, component.name
                    )));
                }
            }
        }
        Ok(())
    }

    fn create_component(
        &mut self,
        command: &Command,
        id: &NodeId,
        name: &str,
        specs: &[ComponentPropSpec],
    ) -> Result<(), CommandError> {
        self.check_component_name(name, None)?;
        let (parent, index) = self
            .doc
            .index_in_parent(id)
            .ok_or_else(|| CommandError::RootNode(id.clone()))?;
        // L'instance prend la place du nœud dans le parent : il doit lui aussi être sélectionné.
        self.check_in_scope(command, &parent)?;
        if matches!(self.node(id)?.kind, NodeKind::Slot(_)) {
            return Err(CommandError::InvalidCommand(
                "a slot cannot become a component".to_owned(),
            ));
        }
        self.check_extractable(id)?;
        let props = self.component_props(id, specs)?;
        let component_id = {
            let doc = &*self.doc;
            self.ids.component(|c| doc.component(c).is_some())
        };
        let instance_id = self.new_node_id(&BTreeSet::new());

        // Les propriétés de placement suivent l'instance.
        let mut instance = Node::new(
            instance_id.clone(),
            Some(parent.clone()),
            NodeKind::ComponentInstance(InstanceProps {
                component: component_id.clone(),
                overrides: vec![],
                variants: vec![],
            }),
        );
        instance.meta.name = Some(name.to_owned());
        instance.meta.source = self.origin.node_source();
        let original = self.node(id)?.clone();
        for prop in PLACEMENT_PROPS {
            if let Some(value) = original.style.get(prop)? {
                instance.style.set(prop, Some(value))?;
                self.emit(Op::SetStyleProp {
                    node: id.clone(),
                    state: None,
                    prop,
                    value: None,
                })?;
            }
        }
        // Le slot ciblé dans une instance parente est aussi du placement.
        instance.meta.slot = original.meta.slot.clone();
        self.set_slot(id, None)?;

        let removed = self.emit(Op::RemoveSubtree { root: id.clone() })?;
        let Op::InsertSubtree { mut nodes, .. } = removed else {
            return Err(CommandError::InvalidCommand("unexpected inverse operation".to_owned()));
        };
        nodes[0].parent = None;
        self.emit(Op::InsertSubtree {
            parent: None,
            index: 0,
            nodes,
        })?;
        let component = Component {
            id: component_id.clone(),
            name: name.to_owned(),
            root: id.clone(),
            props,
            variants: vec![],
        };
        let index_component = self.doc.components.len() as u32;
        self.emit(Op::PutComponent {
            index: index_component,
            component,
        })?;
        self.emit(Op::InsertSubtree {
            parent: Some(parent),
            index: index as u32,
            nodes: vec![instance],
        })?;
        self.out.components.push(component_id);
        self.out.inserted.push(instance_id);
        Ok(())
    }

    /// Une surcharge de variante obéit aux règles du style de son nœud cible : tokens existants,
    /// propriétés permises pour son type et ses hôtes au rendu, `none` réservé aux tailles max.
    fn check_variant_override(&self, over: &VariantOverride) -> Result<(), CommandError> {
        let node = self.node(&over.node)?;
        let mut entries = over.style.entries();
        for state in InteractionState::ALL {
            if let Some(patch) = over.states.get(state) {
                entries.extend(patch.entries());
            }
        }
        for (prop, change) in entries {
            let PropChange::Merge(patch) = change else {
                continue;
            };
            if !style_prop_allowed(self.doc, prop, node) {
                return Err(CommandError::InvalidCommand(format!(
                    "`{}` is not allowed on {} `{}`",
                    prop.name(),
                    node.kind.type_name(),
                    over.node
                )));
            }
            match &patch {
                ResponsiveValuePatch::Size(sizes)
                    if !matches!(prop, StyleProp::MaxWidth | StyleProp::MaxHeight)
                        && sizes.values().into_iter().any(|size| *size == Size::None) =>
                {
                    return Err(CommandError::InvalidCommand(format!(
                        "`none` is only valid for max sizes, not `{}`",
                        prop.name()
                    )));
                }
                ResponsiveValuePatch::Token(fonts) => {
                    if let Some(name) = fonts.values().into_iter().find(|f| self.doc.tokens.font(f).is_none()) {
                        return Err(CommandError::EntityNotFound(format!("font token `{name}`")));
                    }
                }
                _ => {}
            }
            if let Some(name) = patch_color_refs(&patch)
                .into_iter()
                .filter_map(ColorRef::token_name)
                .find(|name| self.doc.tokens.color(name).is_none())
            {
                return Err(CommandError::EntityNotFound(format!("color token `{name}`")));
            }
        }
        Ok(())
    }

    fn update_component(
        &mut self,
        id: &ComponentId,
        name: Option<&str>,
        props: Option<&[ComponentPropSpec]>,
        variants: Option<&[VariantAxis]>,
    ) -> Result<(), CommandError> {
        let index = self
            .doc
            .components
            .iter()
            .position(|c| &c.id == id)
            .ok_or_else(|| CommandError::EntityNotFound(format!("component `{id}`")))?;
        let mut next = self.doc.components[index].clone();
        if let Some(name) = name {
            self.check_component_name(name, Some(id))?;
            next.name = name.to_owned();
        }
        if let Some(specs) = props {
            next.props = self.component_props(&next.root, specs)?;
        }
        if let Some(axes) = variants {
            let mut axis_names = BTreeSet::new();
            for axis in axes {
                if !is_prop_name(&axis.name) || !axis_names.insert(axis.name.clone()) {
                    return Err(CommandError::InvalidCommand(format!(
                        "invalid or duplicate variant axis `{}`",
                        axis.name
                    )));
                }
                let mut option_names = BTreeSet::new();
                for option in &axis.options {
                    if option.name.is_empty() || !option_names.insert(option.name.clone()) {
                        return Err(CommandError::InvalidCommand(format!(
                            "invalid or duplicate option `{}`",
                            option.name
                        )));
                    }
                    for over in &option.overrides {
                        if !self.doc.is_within(&over.node, &next.root) {
                            return Err(CommandError::InvalidCommand(format!(
                                "variant override targets `{}` outside the component",
                                over.node
                            )));
                        }
                        if let Some((prop, _)) = over.style.entries().into_iter().find(|(p, _)| !p.in_base()) {
                            return Err(CommandError::InvalidCommand(format!(
                                "`{}` requires an interaction state",
                                prop.name()
                            )));
                        }
                        self.check_variant_override(over)?;
                    }
                }
                if !option_names.contains(&axis.default) {
                    return Err(CommandError::InvalidCommand(format!(
                        "default option `{}` of axis `{}` does not exist",
                        axis.default, axis.name
                    )));
                }
            }
            next.variants = axes.to_vec();
        }
        if next == self.doc.components[index] {
            return Ok(());
        }
        self.emit(Op::PutComponent {
            index: index as u32,
            component: next.clone(),
        })?;

        // Nettoyage des instances : props et variantes disparues.
        for instance in self.doc.instances_of(id) {
            let NodeKind::ComponentInstance(props) = &self.node(&instance)?.kind else {
                continue;
            };
            let mut cleaned = props.clone();
            cleaned
                .overrides
                .retain(|o| next.prop(&o.prop).is_some_and(|p| p.binding.field.accepts(&o.value)));
            cleaned.variants.retain(|v| {
                next.axis(&v.axis)
                    .is_some_and(|a| a.options.iter().any(|o| o.name == v.option))
            });
            if cleaned != *props {
                self.emit(Op::SetKind {
                    node: instance,
                    kind: NodeKind::ComponentInstance(cleaned),
                })?;
            }
        }
        Ok(())
    }

    fn detach_instance(&mut self, command: &Command, id: &NodeId) -> Result<(), CommandError> {
        let instance = self.node(id)?.clone();
        let NodeKind::ComponentInstance(props) = &instance.kind else {
            return Err(CommandError::KindMismatch {
                expected: "ComponentInstance",
                found: instance.kind.type_name(),
            });
        };
        let (parent, index) = self
            .doc
            .index_in_parent(id)
            .ok_or_else(|| CommandError::RootNode(id.clone()))?;
        // La copie prend la place de l'instance dans le parent : il doit lui aussi être sélectionné.
        self.check_in_scope(command, &parent)?;
        let component = self
            .doc
            .component(&props.component)
            .cloned()
            .ok_or_else(|| CommandError::EntityNotFound(format!("component `{}`", props.component)))?;
        if matches!(self.node(&component.root)?.kind, NodeKind::Slot(_)) {
            return Err(CommandError::InvalidCommand(
                "cannot detach a component whose root is a slot".to_owned(),
            ));
        }

        let subtree = self.doc.subtree(&component.root);
        self.check_raw_code_copy(&subtree)?;
        let mut reserved = BTreeSet::new();
        let mut map = BTreeMap::new();
        for old in &subtree {
            let new = self.new_node_id(&reserved);
            reserved.insert(new.clone());
            map.insert(old.clone(), new);
        }
        let mut nodes = clone_subtree(self.doc, &subtree, &map, Some(parent.clone()), false);

        // Valeurs des props (surcharge de l'instance ou défaut).
        for prop in &component.props {
            let value = props
                .overrides
                .iter()
                .find(|o| o.prop == prop.name)
                .map_or(&prop.default, |o| &o.value);
            if let Some(target) = map.get(&prop.binding.node)
                && let Some(node) = nodes.iter_mut().find(|n| &n.id == target)
            {
                write_field(node, prop.binding.field, value);
            }
        }
        // Style, visibilité, méta (nom, ancre, verrou, slot ciblé) et a11y de l'instance
        // reportés sur la racine.
        let root = &mut nodes[0];
        for (prop, value) in instance.style.entries() {
            root.style.set(prop, Some(value))?;
        }
        for state in InteractionState::ALL {
            for (prop, value) in instance.states.get(state).entries() {
                root.states.get_mut(state).set(prop, Some(value))?;
            }
        }
        if instance.visibility.is_some() {
            root.visibility = instance.visibility.clone();
        }
        if instance.meta.name.is_some() {
            root.meta.name = instance.meta.name.clone();
        }
        if instance.meta.anchor.is_some() {
            root.meta.anchor = instance.meta.anchor.clone();
        }
        if instance.meta.locked {
            root.meta.locked = true;
        }
        root.meta.slot = instance.meta.slot.clone();
        if instance.a11y.label.is_some() {
            root.a11y.label = instance.a11y.label.clone();
        }
        if instance.a11y.hidden {
            root.a11y.hidden = true;
        }
        // Plateforme : la copie n'est rendue que là où l'instance et la racine l'étaient.
        root.platform = match (instance.platform, root.platform) {
            (PlatformScope::All, own) => own,
            (outer, own) if own.is_all() || own == outer => outer,
            (outer, own) => {
                return Err(CommandError::InvalidCommand(format!(
                    "instance `{id}` is limited to {outer:?} but its component root to {own:?}: the copy would render nowhere"
                )));
            }
        };
        // Échappatoires web de l'instance ajoutées à celles de la racine (l'attribut de
        // l'instance l'emporte à nom égal).
        if let Some(web) = &instance.platform_overrides.web {
            let merged = root.platform_overrides.web.get_or_insert_with(WebOverrides::default);
            for class in &web.extra_classes {
                if !merged.extra_classes.contains(class) {
                    merged.extra_classes.push(class.clone());
                }
            }
            for attribute in &web.extra_attributes {
                match merged.extra_attributes.iter_mut().find(|a| a.name == attribute.name) {
                    Some(existing) => existing.value = attribute.value.clone(),
                    None => merged.extra_attributes.push(attribute.clone()),
                }
            }
            if merged.is_empty() {
                root.platform_overrides.web = None;
            }
        }
        let root_id = root.id.clone();
        let slots: Vec<(NodeId, String)> = nodes
            .iter()
            .filter_map(|n| match &n.kind {
                NodeKind::Slot(slot) => Some((n.id.clone(), slot.name.clone())),
                _ => None,
            })
            .collect();
        self.emit(Op::InsertSubtree {
            parent: Some(parent),
            index: index as u32,
            nodes,
        })?;

        // Contenu des slots : les enfants de l'instance prennent la place de leur slot.
        let children = instance.children.clone();
        for child in &children {
            let wanted = self
                .node(child)?
                .meta
                .slot
                .clone()
                .unwrap_or_else(|| DEFAULT_SLOT.to_owned());
            if !slots.iter().any(|(_, name)| *name == wanted) {
                return Err(CommandError::InvalidCommand(format!(
                    "instance child `{child}` targets unknown slot `{wanted}`"
                )));
            }
        }
        for (slot_id, slot_name) in &slots {
            let (slot_parent, slot_index) = self
                .doc
                .index_in_parent(slot_id)
                .ok_or_else(|| CommandError::RootNode(slot_id.clone()))?;
            let mut k = 0;
            for child in &children {
                let target = self
                    .node(child)?
                    .meta
                    .slot
                    .clone()
                    .unwrap_or_else(|| DEFAULT_SLOT.to_owned());
                if target != *slot_name {
                    continue;
                }
                self.emit(Op::MoveNode {
                    node: child.clone(),
                    parent: slot_parent.clone(),
                    index: (slot_index + k) as u32,
                })?;
                let meta = self.node(child)?.meta.clone();
                if meta.slot.is_some() {
                    self.emit(Op::SetMeta {
                        node: child.clone(),
                        meta: NodeMeta { slot: None, ..meta },
                    })?;
                }
                k += 1;
            }
            self.emit(Op::RemoveSubtree { root: slot_id.clone() })?;
        }
        self.emit(Op::RemoveSubtree { root: id.clone() })?;

        // Variantes choisies (ou par défaut).
        for axis in &component.variants {
            let chosen = props
                .variants
                .iter()
                .find(|v| v.axis == axis.name)
                .map_or(&axis.default, |v| &v.option);
            let Some(option) = axis.options.iter().find(|o| &o.name == chosen) else {
                continue;
            };
            for over in &option.overrides {
                let Some(target) = map.get(&over.node).cloned() else {
                    continue;
                };
                if self.doc.node(&target).is_none() {
                    continue;
                }
                self.apply_style(&target, None, over.style.entries())?;
                for state in InteractionState::ALL {
                    if let Some(patch) = over.states.get(state) {
                        self.apply_style(&target, Some(state), patch.entries())?;
                    }
                }
            }
        }
        self.out.inserted.push(root_id);
        Ok(())
    }

    // ---------------------------------------------------------------- layouts, pages

    fn root_container(&mut self, name: &str) -> Node {
        let id = self.new_node_id(&BTreeSet::new());
        let mut node = Node::new(
            id,
            None,
            NodeKind::Box(ContainerProps {
                role: ContainerRole::Generic,
            }),
        );
        node.meta.name = Some(name.to_owned());
        node.meta.source = self.origin.node_source();
        node
    }

    fn create_layout(&mut self, name: &str) -> Result<(), CommandError> {
        Self::check_name(name, "layout")?;
        let mut root = self.root_container(name);
        let mut reserved = BTreeSet::new();
        reserved.insert(root.id.clone());
        let slot_id = self.new_node_id(&reserved);
        let mut slot = Node::new(
            slot_id.clone(),
            Some(root.id.clone()),
            NodeKind::Slot(SlotProps {
                name: PAGE_SLOT.to_owned(),
            }),
        );
        slot.meta.source = root.meta.source;
        root.children.push(slot_id);
        let layout_id = {
            let doc = &*self.doc;
            self.ids.layout(|l| doc.layout(l).is_some())
        };
        let root_id = root.id.clone();
        self.emit(Op::InsertSubtree {
            parent: None,
            index: 0,
            nodes: vec![root, slot],
        })?;
        let index = self.doc.layouts.len() as u32;
        self.emit(Op::PutLayout {
            index,
            layout: Layout {
                id: layout_id.clone(),
                name: name.to_owned(),
                root: root_id,
            },
        })?;
        self.out.layouts.push(layout_id);
        Ok(())
    }

    fn create_page(
        &mut self,
        name: &str,
        route: &[crate::document::RouteSegment],
        layout: Option<&LayoutId>,
        seo: Option<&Seo>,
    ) -> Result<(), CommandError> {
        Self::check_name(name, "page")?;
        if let Some(layout) = layout {
            self.find_layout(layout)?;
        }
        let root = self.root_container("Page");
        let root_id = root.id.clone();
        let page_id = {
            let doc = &*self.doc;
            self.ids.page(|p| doc.page(p).is_some())
        };
        self.emit(Op::InsertSubtree {
            parent: None,
            index: 0,
            nodes: vec![root],
        })?;
        let page = Page {
            id: page_id.clone(),
            name: name.to_owned(),
            route: route.to_vec(),
            layout: layout.cloned(),
            root: root_id,
            seo: seo.cloned().unwrap_or_else(|| Seo {
                title: name.to_owned(),
                description: String::new(),
                og_image: None,
            }),
        };
        let index = self.doc.pages.len() as u32;
        self.emit(Op::PutPage { index, page })?;
        self.out.pages.push(page_id);
        Ok(())
    }

    // ---------------------------------------------------------------- tokens

    fn set_token(&mut self, edit: &TokenEdit) -> Result<(), CommandError> {
        let mut tokens = self.doc.tokens.clone();
        match edit {
            TokenEdit::Color {
                name,
                value: Some(value),
            } => {
                let token = ColorToken {
                    name: name.clone(),
                    light: value.light.clone(),
                    dark: value.dark.clone(),
                };
                match tokens.colors.iter_mut().find(|c| &c.name == name) {
                    Some(existing) => *existing = token,
                    None => tokens.colors.push(token),
                }
            }
            TokenEdit::Color { name, value: None } => {
                if tokens.color(name).is_none() {
                    return Err(CommandError::EntityNotFound(format!("color token `{name}`")));
                }
                if query::color_token_in_use(self.doc, name) {
                    return Err(CommandError::InUse(format!("color token `{name}`")));
                }
                tokens.colors.retain(|c| &c.name != name);
            }
            TokenEdit::Font {
                name,
                value: Some(family),
            } => {
                let token = FontToken {
                    name: name.clone(),
                    family: family.clone(),
                };
                match tokens.fonts.iter_mut().find(|f| &f.name == name) {
                    Some(existing) => *existing = token,
                    None => tokens.fonts.push(token),
                }
            }
            TokenEdit::Font { name, value: None } => {
                if tokens.font(name).is_none() {
                    return Err(CommandError::EntityNotFound(format!("font token `{name}`")));
                }
                if query::font_token_in_use(self.doc, name) {
                    return Err(CommandError::InUse(format!("font token `{name}`")));
                }
                tokens.fonts.retain(|f| &f.name != name);
            }
            TokenEdit::Radius { step, px } => match tokens.radii.iter_mut().find(|r| r.step == *step) {
                Some(existing) => existing.px = *px,
                None => tokens.radii.push(RadiusToken { step: *step, px: *px }),
            },
            TokenEdit::Shadow { step, layers } => match tokens.shadows.iter_mut().find(|s| s.step == *step) {
                Some(existing) => existing.layers = layers.clone(),
                None => tokens.shadows.push(ShadowToken {
                    step: *step,
                    layers: layers.clone(),
                }),
            },
            TokenEdit::SpacingUnit { px } => {
                if !(1..=16).contains(px) {
                    return Err(CommandError::InvalidCommand(
                        "spacing unit must be between 1 and 16 px".to_owned(),
                    ));
                }
                tokens.spacing_unit = *px;
            }
        }
        if tokens != self.doc.tokens {
            self.emit(Op::SetTokens { tokens })?;
        }
        Ok(())
    }
}

fn name_of(spec: &NodeSpec, id: &NodeId) -> String {
    spec.r#ref.clone().unwrap_or_else(|| id.to_string())
}

/// Copie un sous-arbre avec de nouveaux ids (`map`), références internes remappées.
fn clone_subtree(
    doc: &Document,
    subtree: &[NodeId],
    map: &BTreeMap<NodeId, NodeId>,
    root_parent: Option<NodeId>,
    clear_anchors: bool,
) -> Vec<Node> {
    subtree
        .iter()
        .filter_map(|old| doc.node(old))
        .enumerate()
        .map(|(i, node)| {
            let mut copy = node.clone();
            copy.id = map[&node.id].clone();
            copy.parent = if i == 0 {
                root_parent.clone()
            } else {
                node.parent.as_ref().and_then(|p| map.get(p).cloned())
            };
            copy.children = node.children.iter().filter_map(|c| map.get(c).cloned()).collect();
            remap_kind(&mut copy.kind, map);
            if clear_anchors {
                copy.meta.anchor = None;
            }
            copy
        })
        .collect()
}
