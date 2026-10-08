//! Plan du projet, calculé avant l'émission : noms des composants et de leurs props, îlots
//! clients (menus à bascule), chemins des assets et des images de remplissage, hôtes d'images
//! distantes, routes des pages.

use std::collections::{BTreeMap, BTreeSet};

use ir::{
    Action, AspectRatio, AssetId, BindableField, Component, ComponentId, Document, ImageSource, NodeId, NodeKind,
    Owner, PageId, PropValue, route_path,
};

use crate::names::{camel_case, kebab_case, pascal_case, unique};

/// Prop TypeScript d'un composant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropPlan {
    pub name: String,
    pub field: BindableField,
    pub node: NodeId,
    pub default: PropValue,
}

/// Composant exporté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentPlan {
    pub id: ComponentId,
    pub name: String,
    pub props: Vec<PropPlan>,
    /// Slots du composant (`children` d'abord s'il existe).
    pub slots: Vec<String>,
    /// Le composant accepte `className` (une instance le style, ou des variantes visent sa racine).
    pub class_name: bool,
    /// Le composant accepte `id` (une instance porte une ancre), posé sur sa racine.
    pub id_prop: bool,
    /// Le composant transmet à sa racine les attributs reçus (`...rest`) : une instance porte une
    /// étiquette ou un masquage d'accessibilité, ou des attributs web `data-*` / `aria-*`.
    pub rest: bool,
    /// Composant client (il contient un bouton à bascule).
    pub client: bool,
}

/// Bascule d'un menu : un bouton, sa cible, et les noms de l'état.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toggle {
    pub button: NodeId,
    pub target: NodeId,
    /// `menuOpen`.
    pub state: String,
    /// `setMenuOpen`.
    pub setter: String,
    /// `menuId`, ou `None` si la cible a une ancre (son `id`).
    pub id_var: Option<String>,
}

/// Îlot client extrait d'une page ou d'un layout : le plus petit sous-arbre qui contient les
/// boutons à bascule et leurs cibles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Island {
    pub root: NodeId,
    pub name: String,
    pub toggles: Vec<Toggle>,
    /// L'îlot contient le slot `page` du layout (il reçoit `children`).
    pub page_slot: bool,
}

/// Image de remplissage générée en SVG local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder {
    pub path: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub components: BTreeMap<ComponentId, ComponentPlan>,
    pub islands: BTreeMap<NodeId, Island>,
    /// Bascules de chaque composant client (dans son propre fichier).
    pub component_toggles: BTreeMap<ComponentId, Vec<Toggle>>,
    pub assets: BTreeMap<AssetId, String>,
    pub placeholders: BTreeMap<(String, AspectRatio), Placeholder>,
    pub remote_hosts: BTreeSet<(String, String)>,
    pub page_paths: BTreeMap<PageId, String>,
    /// Code libre client, extrait dans `components/raw/<Nom>.tsx`.
    pub raw_clients: BTreeMap<NodeId, String>,
    /// Noms des fichiers de `components/` déjà pris.
    pub taken: BTreeSet<String>,
}

/// Dimensions d'une image de remplissage selon son ratio.
pub fn placeholder_size(ratio: AspectRatio) -> (u32, u32) {
    match ratio {
        AspectRatio::Square => (800, 800),
        AspectRatio::Video => (1280, 720),
        AspectRatio::Portrait => (900, 1200),
        AspectRatio::Landscape | AspectRatio::Auto => (1200, 900),
        AspectRatio::Wide => (1680, 720),
    }
}

impl Plan {
    pub fn new(doc: &Document) -> Plan {
        let mut plan = Plan::default();
        for page in &doc.pages {
            plan.page_paths.insert(page.id.clone(), route_path(&page.route));
        }
        for asset in &doc.assets {
            let file = asset_file_name(&asset.file_name);
            plan.assets
                .insert(asset.id.clone(), format!("/assets/{}-{file}", asset.id.as_str()));
        }
        for component in &doc.components {
            let name = unique(&component.name, &mut plan.taken);
            plan.components
                .insert(component.id.clone(), component_plan(doc, component, name));
        }
        for node in doc.nodes.values() {
            if let NodeKind::Image(image) = &node.kind {
                plan.register_source(&image.source);
            }
            if let NodeKind::RawCode(raw) = &node.kind
                && raw.client
            {
                let base = node
                    .meta
                    .name
                    .as_deref()
                    .map(pascal_case)
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "CustomBlock".to_owned());
                let name = unique(&base, &mut plan.taken);
                plan.raw_clients.insert(node.id.clone(), name);
            }
            if let NodeKind::ComponentInstance(instance) = &node.kind {
                for over in &instance.overrides {
                    if let PropValue::Image(source) = &over.value {
                        plan.register_source(source);
                    }
                }
                // Une instance stylée passe `className` à son composant.
                let styled = !node.style.is_empty() || node.visibility.is_some() || !node.states.is_empty();
                if let Some(c) = plan.components.get_mut(&instance.component) {
                    c.class_name |= styled;
                    c.id_prop |= node.meta.anchor.is_some();
                    c.rest |= node.a11y.label.as_ref().is_some_and(|l| !l.trim().is_empty())
                        || node.a11y.hidden
                        || node
                            .platform_overrides
                            .web
                            .as_ref()
                            .is_some_and(|w| !w.extra_attributes.is_empty());
                }
            }
        }
        for component in &doc.components {
            for prop in &component.props {
                if let PropValue::Image(source) = &prop.default {
                    plan.register_source(source);
                }
            }
        }
        plan.toggles(doc);
        plan
    }

    fn register_source(&mut self, source: &ImageSource) {
        match source {
            ImageSource::Placeholder { label, ratio } => {
                let key = (label.clone(), *ratio);
                if !self.placeholders.contains_key(&key) {
                    let slug = kebab_case(label);
                    let slug: String = slug.chars().take(40).collect();
                    let base = format!("{}-{}", ratio.as_str(), if slug.is_empty() { "image" } else { &slug });
                    let mut path = format!("/placeholders/{base}.svg");
                    let mut n = 2;
                    while self.placeholders.values().any(|p| p.path == path) {
                        path = format!("/placeholders/{base}-{n}.svg");
                        n += 1;
                    }
                    let (width, height) = placeholder_size(*ratio);
                    self.placeholders.insert(
                        key,
                        Placeholder {
                            path,
                            label: label.clone(),
                            width,
                            height,
                        },
                    );
                }
            }
            ImageSource::Url { url } => {
                if let Some((protocol, rest)) = url.split_once("://") {
                    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
                    if !host.is_empty() {
                        self.remote_hosts.insert((protocol.to_owned(), host.to_owned()));
                    }
                }
            }
            ImageSource::Asset { .. } => {}
        }
    }

    /// Chemin public d'une source d'image.
    pub fn image_src(&self, source: &ImageSource) -> String {
        match source {
            ImageSource::Asset { id } => self.assets.get(id).cloned().unwrap_or_default(),
            ImageSource::Url { url } => url.clone(),
            ImageSource::Placeholder { label, ratio } => self
                .placeholders
                .get(&(label.clone(), *ratio))
                .map(|p| p.path.clone())
                .unwrap_or_default(),
        }
    }

    /// Bascules : composants clients, et îlots extraits des pages et des layouts.
    fn toggles(&mut self, doc: &Document) {
        let mut by_owner: BTreeMap<Owner, Vec<(NodeId, NodeId)>> = BTreeMap::new();
        for node in doc.nodes.values() {
            if let NodeKind::Button(button) = &node.kind
                && let Some(Action::ToggleVisibility { target }) = &button.action
                && doc.node(target).is_some()
                && let Some(owner) = doc.owner_of(&node.id)
                && doc.owner_of(target).as_ref() == Some(&owner)
            {
                by_owner
                    .entry(owner)
                    .or_default()
                    .push((node.id.clone(), target.clone()));
            }
        }
        for (owner, pairs) in by_owner {
            let mut state_names = BTreeSet::new();
            let toggles: Vec<Toggle> = pairs
                .iter()
                .map(|(button, target)| toggle(doc, button, target, &mut state_names))
                .collect();
            match owner {
                Owner::Component(component) => {
                    if let Some(plan) = self.components.get_mut(&component) {
                        plan.client = true;
                    }
                    self.component_toggles.insert(component, toggles);
                }
                Owner::Page(_) | Owner::Layout(_) => self.islands(doc, toggles),
            }
        }
    }

    /// Regroupe les bascules d'un arbre en îlots disjoints (ancêtre commun le plus proche).
    fn islands(&mut self, doc: &Document, toggles: Vec<Toggle>) {
        let mut islands: Vec<(NodeId, Vec<Toggle>)> = Vec::new();
        for toggle in toggles {
            let root = common_ancestor(doc, &toggle.button, &toggle.target);
            // Fusion avec un îlot qui contient celui-ci, ou qu'il contient.
            let mut merged = false;
            for (existing, list) in &mut islands {
                if doc.is_within(&root, existing) {
                    list.push(toggle.clone());
                    merged = true;
                    break;
                }
                if doc.is_within(existing, &root) {
                    *existing = root.clone();
                    list.push(toggle.clone());
                    merged = true;
                    break;
                }
            }
            if !merged {
                islands.push((root, vec![toggle]));
            }
        }
        for (root, toggles) in islands {
            let node = doc.node(&root);
            let base = node
                .and_then(|n| n.meta.name.as_deref())
                .map(pascal_case)
                .filter(|n| !n.is_empty())
                .or_else(|| node.map(role_name))
                .unwrap_or_else(|| "Menu".to_owned());
            let name = unique(&base, &mut self.taken);
            let page_slot = doc
                .subtree(&root)
                .iter()
                .any(|n| matches!(doc.node(n).map(|n| &n.kind), Some(NodeKind::Slot(s)) if s.name == ir::PAGE_SLOT));
            self.islands.insert(
                root.clone(),
                Island {
                    root,
                    name,
                    toggles,
                    page_slot,
                },
            );
        }
    }
}

/// Nom de fichier d'un asset, réduit aux caractères sûrs.
fn asset_file_name(name: &str) -> String {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    let stem = kebab_case(stem);
    let stem = if stem.is_empty() { "asset".to_owned() } else { stem };
    let ext: String = ext
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    if ext.is_empty() { stem } else { format!("{stem}.{ext}") }
}

/// Nom d'un îlot tiré du rôle de sa racine.
fn role_name(node: &ir::Node) -> String {
    use ir::ContainerRole as R;
    match node.kind.container().map(|(_, p)| &p.role) {
        Some(R::Header) => "SiteHeader",
        Some(R::Nav) => "SiteNav",
        Some(R::Footer) => "SiteFooter",
        Some(R::Aside) => "Sidebar",
        _ => "Menu",
    }
    .to_owned()
}

fn toggle(doc: &Document, button: &NodeId, target: &NodeId, taken: &mut BTreeSet<String>) -> Toggle {
    let target_node = doc.node(target);
    let base = target_node
        .and_then(|n| n.meta.name.as_deref())
        .map(camel_case)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "menu".to_owned());
    let base = unique(&base, taken);
    let state = format!("{base}Open");
    let setter = format!("set{}{}Open", base[..1].to_ascii_uppercase(), &base[1..]);
    let id_var = target_node
        .and_then(|n| n.meta.anchor.as_ref())
        .is_none()
        .then(|| format!("{base}Id"));
    Toggle {
        button: button.clone(),
        target: target.clone(),
        state,
        setter,
        id_var,
    }
}

/// Ancêtre commun le plus proche de deux nœuds du même arbre.
fn common_ancestor(doc: &Document, a: &NodeId, b: &NodeId) -> NodeId {
    let mut chain_a = vec![a.clone()];
    chain_a.extend(doc.ancestors(a));
    let chain_b: BTreeSet<NodeId> = std::iter::once(b.clone()).chain(doc.ancestors(b)).collect();
    chain_a
        .into_iter()
        .find(|n| chain_b.contains(n))
        .unwrap_or_else(|| doc.tree_root(a))
}

fn component_plan(doc: &Document, component: &Component, name: String) -> ComponentPlan {
    let props = component
        .props
        .iter()
        .map(|p| PropPlan {
            name: p.name.clone(),
            field: p.binding.field,
            node: p.binding.node.clone(),
            default: p.default.clone(),
        })
        .collect();
    let mut slots: Vec<String> = Vec::new();
    for id in doc.subtree(&component.root) {
        if let Some(NodeKind::Slot(slot)) = doc.node(&id).map(|n| &n.kind)
            && !slots.contains(&slot.name)
        {
            slots.push(slot.name.clone());
        }
    }
    slots.sort_by_key(|s| (s != ir::DEFAULT_SLOT, s.clone()));
    let root_variants = component.variants.iter().any(|axis| {
        axis.options
            .iter()
            .any(|o| o.overrides.iter().any(|ov| ov.node == component.root))
    });
    ComponentPlan {
        id: component.id.clone(),
        name,
        props,
        slots,
        class_name: root_variants,
        id_prop: false,
        rest: false,
        client: false,
    }
}
