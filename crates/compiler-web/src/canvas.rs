//! Rendu d'une page pour le canvas de l'éditeur : un arbre d'éléments sérialisable, que le runtime
//! de l'iframe affiche tel quel (ADR 0001 § 8).
//!
//! - Les classes viennent des mêmes fonctions que l'export ([`node_classes`]), appliquées au style
//!   effectif de chaque rendu : surcharges de la variante choisie par l'instance, puis style posé
//!   sur l'instance dont le nœud est la racine (l'instance la plus externe l'emporte, comme
//!   `twMerge` à l'export).
//! - Un même nœud de composant est rendu une fois par instance : chaque élément porte une clé
//!   unique dans la page (instances développées, puis nœud) et l'instance de la page qui le
//!   contient (sélectionnée à sa place au premier clic).
//! - Le thème est la feuille de l'export, pour `@tailwindcss/browser`.

use std::collections::{BTreeMap, BTreeSet};

use ir::command::neutral::neutral_value;
use ir::render::{RenderTree, variant_style};
use ir::{
    Action, BindableField, ButtonType, ContainerRole, Document, FontFamily, FormMethod, Href, ImageSource, ImportDecl,
    InputType, InteractionState, Node, NodeId, NodeKind, PageId, PlatformScope, PropChange, PropValue, TextRole,
};
use serde::Serialize;
use ts_rs::TS;

use crate::classes::node_classes;
use crate::elements::{TextSegment, display_of, open_classes, text_segments};
use crate::plan::{Placeholder, placeholder_size};
use crate::project::placeholder_svg;
use crate::theme::{font_variable, globals_css};

/// Page rendue pour le canvas.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CanvasPage {
    pub page: PageId,
    pub lang: String,
    /// Feuille du thème pour `@tailwindcss/browser` (variables, `@theme`, mode sombre).
    pub css: String,
    /// Feuilles de style des polices Google.
    pub font_stylesheets: Vec<String>,
    /// Racines rendues (racine du layout, ou de la page).
    pub nodes: Vec<CanvasNode>,
    /// Rendu interrompu (composants imbriqués de façon exponentielle).
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CanvasNode {
    Element(CanvasElement),
    Text {
        text: String,
    },
    /// Code libre, rendu par le runtime du canvas.
    Raw(CanvasRaw),
}

/// Élément HTML.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CanvasElement {
    /// Clé de ce rendu, unique dans la page ; absente pour la mise en forme d'un texte (`strong`,
    /// `em`, `br`…), qui ne se sélectionne pas.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub key: Option<String>,
    /// Nœud de l'IR rendu.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node: Option<NodeId>,
    /// Instance écrite dans la page ou le layout qui contient cet élément (la plus externe).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub instance: Option<NodeId>,
    pub tag: String,
    pub class: String,
    pub attrs: BTreeMap<String, String>,
    /// Icône lucide dessinée par l'élément (`svg`), en kebab-case.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub icon: Option<String>,
    /// Bouton à bascule : clé de sa cible, ouverte ou fermée en mode Preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub toggles: Option<String>,
    pub children: Vec<CanvasNode>,
}

/// Bloc de code libre (`RawCode`), jamais réécrit.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CanvasRaw {
    pub key: String,
    pub node: NodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub instance: Option<NodeId>,
    pub code: String,
    pub imports: Vec<ImportDecl>,
    pub client: bool,
}

/// Rendu d'une page pour le canvas ; `None` si la page n'existe pas.
pub fn canvas_page(doc: &Document, page: &PageId) -> Option<CanvasPage> {
    let page = doc.page(page)?;
    let render = RenderTree::of_page(doc, page);
    let mut roots = BTreeSet::from([page.root.clone()]);
    if let Some(layout) = page.layout.as_ref().and_then(|l| doc.layout(l)) {
        roots.insert(layout.root.clone());
    }
    let builder = Builder {
        doc,
        render: &render,
        toggle_targets: toggle_targets(doc),
        roots,
    };
    let nodes = if render.nodes.is_empty() {
        Vec::new()
    } else {
        builder.node(0)
    };
    Some(CanvasPage {
        page: page.id.clone(),
        lang: doc.settings.lang.clone(),
        css: canvas_css(doc),
        font_stylesheets: font_stylesheets(doc),
        nodes,
        truncated: render.truncated,
    })
}

/// Nœuds visés par un bouton à bascule.
fn toggle_targets(doc: &Document) -> BTreeSet<NodeId> {
    doc.nodes
        .values()
        .filter_map(|node| match &node.kind {
            NodeKind::Button(button) => match &button.action {
                Some(Action::ToggleVisibility { target }) => Some(target.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Thème de l'export sans `@import "tailwindcss"` (déjà chargé par `@tailwindcss/browser`), et
/// variables des polices Google que `next/font` pose à l'export.
fn canvas_css(doc: &Document) -> String {
    let css = globals_css(doc);
    let mut out = css
        .strip_prefix("@import \"tailwindcss\";\n")
        .unwrap_or(&css)
        .trim_start()
        .to_owned();
    let fonts: Vec<String> = doc
        .tokens
        .fonts
        .iter()
        .filter_map(|font| match &font.family {
            FontFamily::Google { family, .. } => Some(format!(
                "  {}: \"{}\", system-ui, sans-serif;",
                font_variable(family),
                family
            )),
            FontFamily::System { .. } => None,
        })
        .collect();
    if !fonts.is_empty() {
        out.push_str("\n:root {\n");
        for line in fonts {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("}\n");
    }
    out
}

fn font_stylesheets(doc: &Document) -> Vec<String> {
    let mut out = Vec::new();
    for font in &doc.tokens.fonts {
        let FontFamily::Google { family, weights } = &font.family else {
            continue;
        };
        let name = family.split_whitespace().collect::<Vec<_>>().join("+");
        let mut weights = weights.clone();
        weights.sort_unstable();
        weights.dedup();
        let axis = if weights.is_empty() {
            String::new()
        } else {
            let list = weights.iter().map(u16::to_string).collect::<Vec<_>>().join(";");
            format!(":wght@{list}")
        };
        let url = format!("https://fonts.googleapis.com/css2?family={name}{axis}&display=swap");
        if !out.contains(&url) {
            out.push(url);
        }
    }
    out
}

/// Clé d'un rendu : instances développées puis nœud.
fn key_of(frames: &[NodeId], node: &NodeId) -> String {
    let mut parts: Vec<&str> = frames.iter().map(NodeId::as_str).collect();
    parts.push(node.as_str());
    parts.join("/")
}

/// Nœud tel qu'il est rendu à cette occurrence : variantes choisies par l'instance (style et
/// états), puis ce que portent les instances dont il est la racine, de la plus proche à la plus
/// externe (style, états, visibilité, ancre, accessibilité, échappatoires web).
fn effective(doc: &Document, frames: &[NodeId], node: &Node) -> Node {
    let mut out = node.clone();
    if let Some(style) = variant_style(doc, frames, node) {
        out.style = style;
    }
    apply_variant_states(doc, frames, &mut out);
    let mut root = node.id.clone();
    for (depth, instance_id) in frames.iter().enumerate().rev() {
        let Some(instance) = doc.node(instance_id) else {
            break;
        };
        let NodeKind::ComponentInstance(props) = &instance.kind else {
            break;
        };
        if doc.component(&props.component).is_none_or(|c| c.root != root) {
            break;
        }
        let style = variant_style(doc, &frames[..depth], instance).unwrap_or_else(|| instance.style.clone());
        for (prop, value) in style.entries() {
            let _ = out.style.set(prop, Some(value));
        }
        for state in InteractionState::ALL {
            for (prop, value) in instance.states.get(state).entries() {
                let _ = out.states.get_mut(state).set(prop, Some(value));
            }
        }
        if instance.visibility.is_some() {
            out.visibility = instance.visibility.clone();
        }
        if instance.meta.anchor.is_some() {
            out.meta.anchor = instance.meta.anchor.clone();
        }
        if instance.a11y.label.as_ref().is_some_and(|l| !l.trim().is_empty()) {
            out.a11y.label = instance.a11y.label.clone();
        }
        out.a11y.hidden |= instance.a11y.hidden;
        if let Some(web) = &instance.platform_overrides.web {
            let merged = out.platform_overrides.web.get_or_insert_with(Default::default);
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
        }
        root = instance_id.clone();
    }
    out
}

/// Surcharges d'états (survol, focus, appui) de la variante choisie par l'instance.
fn apply_variant_states(doc: &Document, frames: &[NodeId], node: &mut Node) {
    let Some(instance) = frames.last().and_then(|id| doc.node(id)) else {
        return;
    };
    let NodeKind::ComponentInstance(props) = &instance.kind else {
        return;
    };
    let Some(component) = doc.component(&props.component) else {
        return;
    };
    let id = node.id.clone();
    for axis in &component.variants {
        let chosen = props
            .variants
            .iter()
            .find(|v| v.axis == axis.name)
            .map_or(&axis.default, |v| &v.option);
        let Some(option) = axis.options.iter().find(|o| &o.name == chosen) else {
            continue;
        };
        for over in option.overrides.iter().filter(|o| o.node == id) {
            for state in InteractionState::ALL {
                let Some(patch) = over.states.get(state) else {
                    continue;
                };
                for (prop, change) in patch.entries() {
                    let PropChange::Merge(patch) = change else {
                        continue;
                    };
                    let target = node.states.get_mut(state);
                    let current = target.get(prop).ok().flatten();
                    if let Ok(Some(value)) = patch.apply(current, || neutral_value(doc, &id, Some(state), prop)) {
                        let _ = target.set(prop, Some(value));
                    }
                }
            }
        }
    }
}

struct Builder<'a> {
    doc: &'a Document,
    render: &'a RenderTree,
    toggle_targets: BTreeSet<NodeId>,
    /// Racines de la page et de son layout.
    roots: BTreeSet<NodeId>,
}

impl Builder<'_> {
    /// Rendu d'un nœud rendu : un élément, du code libre, ou rien (nœud natif seul).
    fn node(&self, index: usize) -> Vec<CanvasNode> {
        let entry = &self.render.nodes[index];
        let Some(node) = self.doc.node(&entry.id) else {
            return Vec::new();
        };
        if node.platform == PlatformScope::NativeOnly {
            return Vec::new();
        }
        if ir::render::is_transparent(self.doc, &node.id) {
            return self.children(index);
        }
        let key = key_of(&entry.frames, &node.id);
        let instance = entry.frames.first().cloned();
        if let NodeKind::RawCode(raw) = &node.kind {
            return vec![CanvasNode::Raw(CanvasRaw {
                key,
                node: node.id.clone(),
                instance,
                code: raw.code.clone(),
                imports: raw.imports.clone(),
                client: raw.client,
            })];
        }
        let shown = effective(self.doc, &entry.frames, node);
        let display = display_of(self.doc, &shown);
        let mut classes = node_classes(self.doc, &shown, display);
        if self.toggle_targets.contains(&node.id) {
            classes.extend(open_classes(&shown, display));
        }
        let mut element = CanvasElement {
            key: Some(key),
            node: Some(node.id.clone()),
            instance,
            tag: String::new(),
            class: classes.join(" "),
            attrs: BTreeMap::new(),
            icon: None,
            toggles: None,
            children: Vec::new(),
        };
        self.content(index, &shown, &mut element);
        if let Some(anchor) = &shown.meta.anchor {
            element.attrs.insert("id".to_owned(), anchor.clone());
        }
        if !matches!(shown.kind, NodeKind::Icon(_)) {
            if shown.a11y.hidden {
                element.attrs.insert("aria-hidden".to_owned(), "true".to_owned());
            }
            if let Some(label) = shown.a11y.label.as_ref().filter(|l| !l.trim().is_empty()) {
                element.attrs.insert("aria-label".to_owned(), label.clone());
            }
        }
        if let Some(web) = &shown.platform_overrides.web {
            for attribute in &web.extra_attributes {
                element.attrs.insert(attribute.name.clone(), attribute.value.clone());
            }
        }
        // Racine générique sans classe ni attribut : un fragment à l'export, ses enfants ici.
        if self.roots.contains(&node.id) && element.tag == "div" && element.class.is_empty() && element.attrs.is_empty()
        {
            return element.children;
        }
        vec![CanvasNode::Element(element)]
    }

    fn children(&self, index: usize) -> Vec<CanvasNode> {
        self.render
            .children(index)
            .iter()
            .flat_map(|child| self.node(*child))
            .collect()
    }

    /// Valeur d'un champ lié à une prop pour ce rendu.
    fn bound(&self, index: usize, field: BindableField) -> Option<&PropValue> {
        self.render.bound(self.doc, index, field).map(|b| b.value)
    }

    fn bound_text(&self, index: usize, field: BindableField) -> Option<String> {
        match self.bound(index, field) {
            Some(PropValue::Text(text)) => Some(text.clone()),
            _ => None,
        }
    }

    /// Balise, attributs et enfants selon le type du nœud (mêmes choix que l'export).
    fn content(&self, index: usize, node: &Node, element: &mut CanvasElement) {
        let entry = &self.render.nodes[index];
        match &node.kind {
            NodeKind::Box(props) | NodeKind::Stack(props) | NodeKind::Grid(props) => {
                element.tag = match &props.role {
                    ContainerRole::Generic => "div",
                    ContainerRole::Section => "section",
                    ContainerRole::Header => "header",
                    ContainerRole::Footer => "footer",
                    ContainerRole::Nav => "nav",
                    ContainerRole::Main => "main",
                    ContainerRole::Article => "article",
                    ContainerRole::Aside => "aside",
                    ContainerRole::List => "ul",
                    ContainerRole::ListItem => "li",
                    ContainerRole::Form { action, method } => {
                        if let Some(action) = action {
                            element.attrs.insert("action".to_owned(), action.clone());
                        }
                        let method = match method {
                            FormMethod::Get => "get",
                            FormMethod::Post => "post",
                        };
                        element.attrs.insert("method".to_owned(), method.to_owned());
                        "form"
                    }
                }
                .to_owned();
                element.children = self.children(index);
            }
            NodeKind::Text(text) => {
                element.tag = match &text.role {
                    TextRole::Heading { level } => format!("h{}", level.number()),
                    TextRole::Paragraph | TextRole::Caption => "p".to_owned(),
                    TextRole::Inline => "span".to_owned(),
                    TextRole::Quote => "blockquote".to_owned(),
                    TextRole::Code => "code".to_owned(),
                    TextRole::Label { .. } => "label".to_owned(),
                };
                element.children = match self.bound_text(index, BindableField::Text) {
                    Some(value) => plain(&value),
                    None => runs(&text.content),
                };
            }
            NodeKind::Image(image) => {
                element.tag = "img".to_owned();
                let source = match self.bound(index, BindableField::ImageSource) {
                    Some(PropValue::Image(source)) => source.clone(),
                    _ => image.source.clone(),
                };
                let alt = match self.bound_text(index, BindableField::ImageAlt) {
                    Some(alt) => alt,
                    None if node.a11y.hidden => String::new(),
                    None => image.alt.clone(),
                };
                let (width, height) = match (&image.intrinsic, &source) {
                    (Some(size), _) => (size.width, size.height),
                    (None, ImageSource::Placeholder { ratio, .. }) => placeholder_size(*ratio),
                    (None, _) => (1200, 900),
                };
                match &source {
                    ImageSource::Url { url } => {
                        element.attrs.insert("src".to_owned(), url.clone());
                    }
                    ImageSource::Placeholder { label, .. } => {
                        element
                            .attrs
                            .insert("src".to_owned(), placeholder_data_url(label, width, height));
                    }
                    ImageSource::Asset { id } => {
                        // Résolue par l'éditeur (stockage des assets) ; en attendant, un aplat.
                        let name = self
                            .doc
                            .assets
                            .iter()
                            .find(|a| a.id == *id)
                            .map_or_else(|| id.to_string(), |a| a.file_name.clone());
                        element.attrs.insert("data-asset".to_owned(), id.to_string());
                        element
                            .attrs
                            .insert("src".to_owned(), placeholder_data_url(&name, width, height));
                    }
                }
                element.attrs.insert("alt".to_owned(), alt);
                element.attrs.insert("width".to_owned(), width.to_string());
                element.attrs.insert("height".to_owned(), height.to_string());
            }
            NodeKind::Icon(icon) => {
                element.tag = "svg".to_owned();
                element.icon = Some(icon.name.clone());
                match node.a11y.label.as_ref().filter(|l| !l.trim().is_empty()) {
                    Some(label) => {
                        element.attrs.insert("role".to_owned(), "img".to_owned());
                        element.attrs.insert("aria-label".to_owned(), label.clone());
                    }
                    None => {
                        element.attrs.insert("aria-hidden".to_owned(), "true".to_owned());
                    }
                }
            }
            NodeKind::Button(button) => {
                element.tag = "button".to_owned();
                let kind = match button.button_type {
                    ButtonType::Button => "button",
                    ButtonType::Submit => "submit",
                    ButtonType::Reset => "reset",
                };
                element.attrs.insert("type".to_owned(), kind.to_owned());
                if button.disabled {
                    element.attrs.insert("disabled".to_owned(), String::new());
                }
                if let Some(Action::ToggleVisibility { target }) = &button.action {
                    element.toggles = Some(key_of(&entry.frames, target));
                }
                element.children = match self.bound_text(index, BindableField::Label) {
                    Some(label) => plain(&label),
                    None => match &button.label {
                        Some(label) => plain(label),
                        None => self.children(index),
                    },
                };
            }
            NodeKind::Input(input) => {
                if input.input_type == InputType::Multiline {
                    element.tag = "textarea".to_owned();
                } else {
                    element.tag = "input".to_owned();
                    let kind = match input.input_type {
                        InputType::Text | InputType::Multiline => "text",
                        InputType::Email => "email",
                        InputType::Password => "password",
                        InputType::Number => "number",
                        InputType::Tel => "tel",
                        InputType::Url => "url",
                        InputType::Search => "search",
                    };
                    element.attrs.insert("type".to_owned(), kind.to_owned());
                }
                element.attrs.insert("name".to_owned(), input.name.clone());
                if let Some(placeholder) = &input.placeholder {
                    element.attrs.insert("placeholder".to_owned(), placeholder.clone());
                }
                if input.required {
                    element.attrs.insert("required".to_owned(), String::new());
                }
            }
            NodeKind::Link(link) => {
                element.tag = "a".to_owned();
                let href = match self.bound(index, BindableField::Href) {
                    Some(PropValue::Href(href)) => href.clone(),
                    _ => link.href.clone(),
                };
                match &href {
                    Href::Page { page, anchor } => {
                        element.attrs.insert("data-page".to_owned(), page.to_string());
                        let target = anchor.as_ref().map_or_else(|| "#".to_owned(), |a| format!("#{a}"));
                        element.attrs.insert("href".to_owned(), target);
                    }
                    Href::Anchor { anchor } => {
                        element.attrs.insert("href".to_owned(), format!("#{anchor}"));
                    }
                    Href::External { url } => {
                        element.attrs.insert("href".to_owned(), url.clone());
                    }
                    Href::Email { address } => {
                        element.attrs.insert("href".to_owned(), format!("mailto:{address}"));
                    }
                    Href::Phone { number } => {
                        element
                            .attrs
                            .insert("href".to_owned(), format!("tel:{}", number.replace(' ', "")));
                    }
                }
                element.children = match self.bound_text(index, BindableField::Label) {
                    Some(label) => plain(&label),
                    None => match &link.label {
                        Some(label) => plain(label),
                        None => self.children(index),
                    },
                };
            }
            // Transparents ou traités avant (instances, slots, code libre).
            NodeKind::ComponentInstance(_) | NodeKind::Slot(_) | NodeKind::RawCode(_) => {}
        }
    }
}

/// Libellé : mots séparés par une espace.
fn plain(label: &str) -> Vec<CanvasNode> {
    let collapsed = label.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        Vec::new()
    } else {
        vec![CanvasNode::Text { text: collapsed }]
    }
}

/// Texte mis en forme, segmenté comme à l'export.
fn runs(runs: &[ir::TextRun]) -> Vec<CanvasNode> {
    text_segments(runs)
        .into_iter()
        .map(|segment| match segment {
            TextSegment::Text(text) => CanvasNode::Text { text },
            TextSegment::Formatted(run, text) => formatted(run, text),
            TextSegment::Break => CanvasNode::Element(inline_element("br", String::new(), Vec::new())),
        })
        .collect()
}

/// `strong`, `em`, `code` imbriqués, couleur sur le plus externe (`span` sans autre mise en forme).
fn formatted(run: &ir::TextRun, text: String) -> CanvasNode {
    let mut tags: Vec<&str> = Vec::new();
    if run.strong {
        tags.push("strong");
    }
    if run.em {
        tags.push("em");
    }
    if run.code {
        tags.push("code");
    }
    if tags.is_empty() {
        tags.push("span");
    }
    let mut node = CanvasNode::Text { text };
    for (i, tag) in tags.iter().enumerate().rev() {
        let class = match (&run.color, i) {
            (Some(color), 0) => format!("text-{color}"),
            _ => String::new(),
        };
        node = CanvasNode::Element(inline_element(tag, class, vec![node]));
    }
    node
}

fn inline_element(tag: &str, class: String, children: Vec<CanvasNode>) -> CanvasElement {
    CanvasElement {
        key: None,
        node: None,
        instance: None,
        tag: tag.to_owned(),
        class,
        attrs: BTreeMap::new(),
        icon: None,
        toggles: None,
        children,
    }
}

/// Image de remplissage de l'export, en URL `data:`.
fn placeholder_data_url(label: &str, width: u32, height: u32) -> String {
    let svg = placeholder_svg(&Placeholder {
        path: String::new(),
        label: label.to_owned(),
        width,
        height,
    });
    let mut encoded = String::from("data:image/svg+xml,");
    for byte in svg.trim_end().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'=' | b':' | b'/' | b',' => {
                encoded.push(byte as char);
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}
