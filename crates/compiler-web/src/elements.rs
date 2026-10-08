//! Nœuds de l'IR → JSX : balises sémantiques, `next/link`, `next/image`, icônes lucide,
//! instances de composants (props, variantes, slots), bascules de menu, code libre.

use std::collections::BTreeMap;

use ir::{
    Action, Autocomplete, BindableField, ButtonType, Component, ContainerRole, Document, FormMethod, Href, ImageSource,
    InputType, Node, NodeId, NodeKind, PlatformScope, PropValue, TextRole, TextRun,
};

use crate::classes::{Display, node_classes, patch_classes};
use crate::jsx::{AttrValue, Element, Expr, Jsx, escape_text};
use crate::module::{Imports, Item, Statement, Value};
use crate::names::{camel_case, icon_component, pascal_case, unique};
use crate::plan::{ComponentPlan, Island, Plan, Toggle};

/// Fichier en cours d'émission.
#[derive(Debug, Clone, Copy)]
pub enum Scope<'a> {
    /// Page ou layout : les îlots clients sont remplacés par leur composant.
    Route,
    /// Îlot client extrait d'une page ou d'un layout.
    Island(&'a Island),
    /// Composant exporté.
    Component(&'a Component, &'a ComponentPlan),
}

/// Identifiant d'un champ pour son étiquette : chaîne fixe, ou variable `useId()`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FieldId {
    Static(String),
    Var(String),
}

pub struct Emitter<'a> {
    doc: &'a Document,
    plan: &'a Plan,
    scope: Scope<'a>,
    edit: bool,
    pub imports: Imports,
    /// Instructions à placer en tête de la fonction (états, `useId`).
    pub statements: Vec<Statement>,
    /// Tables de classes des variantes.
    pub items: Vec<Item>,
    pub uses_twmerge: bool,
    /// Le fichier rend `{children}` (slot par défaut, slot `page`).
    pub uses_children: bool,
    toggles: Vec<Toggle>,
    field_ids: BTreeMap<NodeId, FieldId>,
    variant_names: BTreeMap<(String, NodeId), String>,
}

/// Valeur littérale TypeScript d'une valeur de prop.
pub fn prop_literal(plan: &Plan, value: &PropValue) -> String {
    match value {
        PropValue::Text(s) => crate::jsx::string_literal(s),
        PropValue::Bool(b) => b.to_string(),
        PropValue::Href(h) => crate::jsx::string_literal(&href_string(plan, h)),
        PropValue::Image(source) => crate::jsx::string_literal(&plan.image_src(source)),
    }
}

/// Adresse d'un lien.
pub fn href_string(plan: &Plan, href: &Href) -> String {
    match href {
        Href::Page { page, anchor } => {
            let path = plan.page_paths.get(page).cloned().unwrap_or_else(|| "/".to_owned());
            match anchor {
                Some(anchor) => format!("{path}#{anchor}"),
                None => path,
            }
        }
        Href::Anchor { anchor } => format!("#{anchor}"),
        Href::External { url } => url.clone(),
        Href::Email { address } => format!("mailto:{address}"),
        Href::Phone { number } => format!("tel:{}", number.replace(' ', "")),
    }
}

/// Affichage naturel de l'élément qui rend un nœud.
pub fn display_of(doc: &Document, node: &Node) -> Display {
    match &node.kind {
        NodeKind::Stack(_) => Display::Flex,
        NodeKind::Grid(_) => Display::Grid,
        NodeKind::Text(text) => match text.role {
            TextRole::Inline | TextRole::Code | TextRole::Label { .. } => Display::Inline,
            _ => Display::Block,
        },
        NodeKind::Button(_) | NodeKind::Input(_) => Display::InlineBlock,
        NodeKind::Link(_) => Display::Inline,
        NodeKind::ComponentInstance(props) => doc
            .component(&props.component)
            .and_then(|c| doc.node(&c.root))
            .filter(|root| !matches!(root.kind, NodeKind::ComponentInstance(_)))
            .map_or(Display::Block, |root| display_of(doc, root)),
        _ => Display::Block,
    }
}

impl<'a> Emitter<'a> {
    pub fn new(doc: &'a Document, plan: &'a Plan, scope: Scope<'a>, edit: bool) -> Self {
        let toggles = match scope {
            Scope::Island(island) => island.toggles.clone(),
            Scope::Component(component, _) => plan.component_toggles.get(&component.id).cloned().unwrap_or_default(),
            Scope::Route => Vec::new(),
        };
        Self {
            doc,
            plan,
            scope,
            edit,
            imports: Imports::default(),
            statements: Vec::new(),
            items: Vec::new(),
            uses_twmerge: false,
            uses_children: false,
            toggles,
            field_ids: BTreeMap::new(),
            variant_names: BTreeMap::new(),
        }
    }

    /// JSX d'un arbre (racine d'une page, d'un layout, d'un composant ou d'un îlot).
    pub fn tree(&mut self, root: &NodeId) -> Jsx {
        self.prepare(root);
        // Racine de composant liée à une prop `Visible` : garde en tête de la fonction.
        let guard = self.doc.node(root).and_then(|n| self.bound(n, BindableField::Visible));
        let mut nodes = match (&self.scope, guard) {
            (Scope::Island(island), _) if island.root == *root => self.node_content(root),
            (_, Some(prop)) => {
                self.statements.push(Statement::ReturnNullUnless(prop));
                self.node_content(root)
            }
            _ => self.node(root),
        };
        if nodes.len() == 1 {
            nodes.remove(0)
        } else {
            Jsx::Fragment(nodes)
        }
    }

    /// États des bascules et identifiants des champs étiquetés.
    fn prepare(&mut self, root: &NodeId) {
        let component = matches!(self.scope, Scope::Component(..));
        let client = component || matches!(self.scope, Scope::Island(_));
        if client {
            for toggle in self.toggles.clone() {
                self.imports.named("react", "useState");
                self.statements.push(Statement::Raw(format!(
                    "const [{}, {}] = useState(false);",
                    toggle.state, toggle.setter
                )));
                if let Some(var) = &toggle.id_var {
                    self.imports.named("react", "useId");
                    self.statements.push(Statement::Raw(format!("const {var} = useId();")));
                }
            }
        }
        let tree = self.doc.subtree(root);
        let mut taken = std::collections::BTreeSet::new();
        for id in &tree {
            let Some(node) = self.doc.node(id) else { continue };
            if let NodeKind::Text(text) = &node.kind
                && let TextRole::Label { for_input: Some(input) } = &text.role
                && let Some(NodeKind::Input(props)) = self.doc.node(input).map(|n| &n.kind)
                && !self.field_ids.contains_key(input)
            {
                let base = camel_case(&props.name);
                let base = if base.is_empty() { "field".to_owned() } else { base };
                let field = if component {
                    let var = unique(&format!("{base}Id"), &mut taken);
                    self.imports.named("react", "useId");
                    self.statements.push(Statement::Raw(format!("const {var} = useId();")));
                    FieldId::Var(var)
                } else {
                    FieldId::Static(crate::names::kebab_case(&unique(&base, &mut taken)))
                };
                self.field_ids.insert(input.clone(), field);
            }
        }
    }

    fn component(&self) -> Option<(&'a Component, &'a ComponentPlan)> {
        match self.scope {
            Scope::Component(component, plan) => Some((component, plan)),
            _ => None,
        }
    }

    /// Nom de la prop qui lie un champ du nœud dans le composant émis.
    fn bound(&self, node: &Node, field: BindableField) -> Option<String> {
        let (_, plan) = self.component()?;
        plan.props
            .iter()
            .find(|p| p.node == node.id && p.field == field)
            .map(|p| p.name.clone())
    }

    /// JSX d'un nœud (zéro, un ou plusieurs nœuds JSX).
    fn node(&mut self, id: &NodeId) -> Vec<Jsx> {
        let Some(node) = self.doc.node(id) else {
            return Vec::new();
        };
        if node.platform == PlatformScope::NativeOnly {
            return Vec::new();
        }
        // Îlot client d'une page ou d'un layout : rendu par son composant.
        if matches!(self.scope, Scope::Route)
            && let Some(island) = self.plan.islands.get(id)
        {
            self.imports
                .named(&format!("@/components/{}", island.name), &island.name);
            let mut element = Element::new(island.name.clone());
            if island.page_slot {
                self.uses_children = true;
                element = element.child(Jsx::Expr(Expr::Atom("children".to_owned())));
            }
            return vec![Jsx::Element(element)];
        }
        let jsx = self.node_content(id);
        match self.bound(node, BindableField::Visible) {
            Some(prop) => jsx
                .into_iter()
                .map(|j| {
                    Jsx::Expr(Expr::And {
                        condition: prop.clone(),
                        then: Box::new(j),
                    })
                })
                .collect(),
            None => jsx,
        }
    }

    fn node_content(&mut self, id: &NodeId) -> Vec<Jsx> {
        let Some(node) = self.doc.node(id) else {
            return Vec::new();
        };
        let doc = self.doc;
        match &node.kind {
            NodeKind::Box(props) | NodeKind::Stack(props) | NodeKind::Grid(props) => {
                let (tag, extra) = match &props.role {
                    ContainerRole::Generic => ("div", Vec::new()),
                    ContainerRole::Section => ("section", Vec::new()),
                    ContainerRole::Header => ("header", Vec::new()),
                    ContainerRole::Footer => ("footer", Vec::new()),
                    ContainerRole::Nav => ("nav", Vec::new()),
                    ContainerRole::Main => ("main", Vec::new()),
                    ContainerRole::Article => ("article", Vec::new()),
                    ContainerRole::Aside => ("aside", Vec::new()),
                    ContainerRole::List => ("ul", Vec::new()),
                    ContainerRole::ListItem => ("li", Vec::new()),
                    ContainerRole::Form { action, method } => {
                        let mut attrs = Vec::new();
                        if let Some(action) = action {
                            attrs.push(("action".to_owned(), AttrValue::Str(action.clone())));
                        }
                        let method = match method {
                            FormMethod::Get => "get",
                            FormMethod::Post => "post",
                        };
                        attrs.push(("method".to_owned(), AttrValue::Str(method.to_owned())));
                        ("form", attrs)
                    }
                };
                let mut element = Element::new(tag);
                for (name, value) in extra {
                    element = element.attr(name, value);
                }
                element.children = self.children(node);
                vec![self.finish(node, element)]
            }
            NodeKind::Text(text) => {
                let tag = match &text.role {
                    TextRole::Heading { level } => format!("h{}", level.number()),
                    TextRole::Paragraph | TextRole::Caption => "p".to_owned(),
                    TextRole::Inline => "span".to_owned(),
                    TextRole::Quote => "blockquote".to_owned(),
                    TextRole::Code => "code".to_owned(),
                    TextRole::Label { .. } => "label".to_owned(),
                };
                let mut element = Element::new(tag);
                if let TextRole::Label { for_input: Some(input) } = &text.role {
                    match self.field_ids.get(input) {
                        Some(FieldId::Static(id)) => element = element.string("htmlFor", id.clone()),
                        Some(FieldId::Var(var)) => {
                            element = element.attr("htmlFor", AttrValue::Expr(Expr::Atom(var.clone())));
                        }
                        None => {}
                    }
                }
                element.children = match self.bound(node, BindableField::Text) {
                    Some(prop) => vec![Jsx::Expr(Expr::Atom(prop))],
                    None => self.runs(&text.content),
                };
                vec![self.finish(node, element)]
            }
            NodeKind::Image(image) => {
                self.imports.default_import("next/image", "Image");
                let mut element = Element::new("Image");
                let src = match self.bound(node, BindableField::ImageSource) {
                    Some(prop) => AttrValue::Expr(Expr::Atom(prop)),
                    None => AttrValue::Str(self.plan.image_src(&image.source)),
                };
                let svg = match &image.source {
                    ImageSource::Placeholder { .. } => true,
                    other => self.plan.image_src(other).to_ascii_lowercase().ends_with(".svg"),
                };
                element = element.attr("src", src);
                element = match self.bound(node, BindableField::ImageAlt) {
                    Some(prop) => element.attr("alt", AttrValue::Expr(Expr::Atom(prop))),
                    None => element.string("alt", if node.a11y.hidden { "" } else { &image.alt }),
                };
                let (width, height) = match (&image.intrinsic, &image.source) {
                    (Some(size), _) => (size.width, size.height),
                    (None, ImageSource::Placeholder { ratio, .. }) => crate::plan::placeholder_size(*ratio),
                    (None, _) => (1200, 900),
                };
                element = element
                    .attr("width", AttrValue::Expr(Expr::Atom(width.to_string())))
                    .attr("height", AttrValue::Expr(Expr::Atom(height.to_string())));
                if image.priority {
                    element = element.attr("preload", AttrValue::Flag);
                }
                match self.bound(node, BindableField::ImageSource) {
                    // Source liée : l'optimiseur de `next/image` refuse le SVG.
                    Some(prop) => {
                        element = element.attr(
                            "unoptimized",
                            AttrValue::Expr(Expr::Call {
                                callee: format!("{prop}.endsWith"),
                                args: vec![Expr::Str(".svg".to_owned())],
                            }),
                        );
                    }
                    None if svg => element = element.attr("unoptimized", AttrValue::Flag),
                    None => {}
                }
                vec![self.finish(node, element)]
            }
            NodeKind::Icon(icon) => {
                let name = icon_component(&icon.name);
                self.imports.named("lucide-react", &name);
                let mut element = Element::new(name);
                match node.a11y.label.as_ref().filter(|l| !l.trim().is_empty()) {
                    Some(label) => element = element.string("role", "img").string("aria-label", label.clone()),
                    None => element = element.string("aria-hidden", "true"),
                }
                vec![self.finish(node, element)]
            }
            NodeKind::Button(button) => {
                let kind = match button.button_type {
                    ButtonType::Button => "button",
                    ButtonType::Submit => "submit",
                    ButtonType::Reset => "reset",
                };
                let mut element = Element::new("button").string("type", kind);
                if button.disabled {
                    element = element.attr("disabled", AttrValue::Flag);
                }
                if let Some(Action::ToggleVisibility { .. }) = &button.action
                    && let Some(toggle) = self.toggles.iter().find(|t| t.button == node.id).cloned()
                {
                    let controls = match &toggle.id_var {
                        Some(var) => AttrValue::Expr(Expr::Atom(var.clone())),
                        None => AttrValue::Str(
                            doc.node(&toggle.target)
                                .and_then(|t| t.meta.anchor.clone())
                                .unwrap_or_default(),
                        ),
                    };
                    element = element
                        .attr("aria-expanded", AttrValue::Expr(Expr::Atom(toggle.state.clone())))
                        .attr("aria-controls", controls)
                        .attr(
                            "onClick",
                            AttrValue::Expr(Expr::Inline(format!("() => {}(!{})", toggle.setter, toggle.state))),
                        );
                }
                element.children = match self.bound(node, BindableField::Label) {
                    Some(prop) => vec![Jsx::Expr(Expr::Atom(prop))],
                    None => match &button.label {
                        Some(label) => self.plain_text(label),
                        None => self.children(node),
                    },
                };
                vec![self.finish(node, element)]
            }
            NodeKind::Input(input) => {
                let mut element = if input.input_type == InputType::Multiline {
                    Element::new("textarea")
                } else {
                    let kind = match input.input_type {
                        InputType::Text => "text",
                        InputType::Email => "email",
                        InputType::Password => "password",
                        InputType::Number => "number",
                        InputType::Tel => "tel",
                        InputType::Url => "url",
                        InputType::Search => "search",
                        InputType::Multiline => "text",
                    };
                    Element::new("input").string("type", kind)
                };
                match self.field_ids.get(&node.id) {
                    Some(FieldId::Static(id)) => element = element.string("id", id.clone()),
                    Some(FieldId::Var(var)) => element = element.attr("id", AttrValue::Expr(Expr::Atom(var.clone()))),
                    None => {}
                }
                element = element.string("name", input.name.clone());
                if let Some(placeholder) = &input.placeholder {
                    element = element.string("placeholder", placeholder.clone());
                }
                if let Some(autocomplete) = input.autocomplete {
                    let value = match autocomplete {
                        Autocomplete::Off => "off",
                        Autocomplete::Name => "name",
                        Autocomplete::Email => "email",
                        Autocomplete::Tel => "tel",
                        Autocomplete::Organization => "organization",
                        Autocomplete::StreetAddress => "street-address",
                        Autocomplete::PostalCode => "postal-code",
                        Autocomplete::Country => "country",
                    };
                    element = element.string("autoComplete", value);
                }
                if input.required {
                    element = element.attr("required", AttrValue::Flag);
                }
                vec![self.finish(node, element)]
            }
            NodeKind::Link(link) => {
                let mut element = match self.bound(node, BindableField::Href) {
                    Some(prop) => {
                        self.imports.default_import("next/link", "Link");
                        Element::new("Link").attr("href", AttrValue::Expr(Expr::Atom(prop)))
                    }
                    None => match &link.href {
                        Href::Page { .. } => {
                            self.imports.default_import("next/link", "Link");
                            Element::new("Link").string("href", href_string(self.plan, &link.href))
                        }
                        other => Element::new("a").string("href", href_string(self.plan, other)),
                    },
                };
                if link.new_tab {
                    element = element.string("target", "_blank").string("rel", "noopener noreferrer");
                }
                element.children = match self.bound(node, BindableField::Label) {
                    Some(prop) => vec![Jsx::Expr(Expr::Atom(prop))],
                    None => match &link.label {
                        Some(label) => self.plain_text(label),
                        None => self.children(node),
                    },
                };
                vec![self.finish(node, element)]
            }
            NodeKind::ComponentInstance(_) => self.instance(node),
            NodeKind::Slot(slot) => {
                let in_component = matches!(self.scope, Scope::Component(..));
                let layout_page = !in_component && slot.name == ir::PAGE_SLOT;
                if in_component || layout_page {
                    let name = if layout_page {
                        "children".to_owned()
                    } else {
                        slot.name.clone()
                    };
                    if name == "children" {
                        self.uses_children = true;
                    }
                    vec![Jsx::Expr(Expr::Atom(name))]
                } else {
                    Vec::new()
                }
            }
            NodeKind::RawCode(raw) => {
                if let Some(name) = self.plan.raw_clients.get(&node.id) {
                    self.imports.named(&format!("@/components/raw/{name}"), name);
                    return vec![Jsx::Element(Element::new(name.clone()))];
                }
                for import in &raw.imports {
                    if let Some(default) = &import.default {
                        self.imports.default_import(&import.module, default);
                    }
                    for name in &import.named {
                        self.imports.named(&import.module, name);
                    }
                }
                vec![Jsx::Raw(raw.code.clone())]
            }
        }
    }

    fn children(&mut self, node: &Node) -> Vec<Jsx> {
        let mut out = Vec::new();
        for child in &node.children {
            out.extend(self.node(child));
        }
        out
    }

    /// Texte d'un libellé (mots séparés par une espace).
    fn plain_text(&self, label: &str) -> Vec<Jsx> {
        let collapsed = label.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            Vec::new()
        } else {
            vec![Jsx::Text(escape_text(&collapsed))]
        }
    }

    /// Segments de texte : mots, mises en forme (`strong`, `em`, `code`, couleur), sauts de ligne.
    fn runs(&self, runs: &[TextRun]) -> Vec<Jsx> {
        text_segments(runs)
            .into_iter()
            .map(|segment| match segment {
                TextSegment::Text(value) => Jsx::Text(escape_text(&value)),
                TextSegment::Formatted(run, value) => formatted(run, &value),
                TextSegment::Break => Jsx::Element(Element::new("br")),
            })
            .collect()
    }

    /// Instance d'un composant : props, variantes, slots nommés, contenu du slot par défaut.
    fn instance(&mut self, node: &Node) -> Vec<Jsx> {
        let NodeKind::ComponentInstance(props) = &node.kind else {
            return Vec::new();
        };
        let Some(plan) = self.plan.components.get(&props.component) else {
            return Vec::new();
        };
        if matches!(self.scope, Scope::Component(c, _) if c.id == props.component) {
            return Vec::new();
        }
        self.imports.named(&format!("@/components/{}", plan.name), &plan.name);
        let mut element = Element::new(plan.name.clone());
        for prop in &plan.props {
            let Some(over) = props.overrides.iter().find(|o| o.prop == prop.name) else {
                continue;
            };
            let value = match &over.value {
                PropValue::Text(s) => AttrValue::Str(s.clone()),
                PropValue::Bool(b) => AttrValue::Expr(Expr::Atom(b.to_string())),
                PropValue::Href(h) => AttrValue::Str(href_string(self.plan, h)),
                PropValue::Image(source) => AttrValue::Str(self.plan.image_src(source)),
            };
            element = element.attr(prop.name.clone(), value);
        }
        let mut seen = std::collections::BTreeSet::new();
        for choice in &props.variants {
            if seen.insert(choice.axis.clone()) {
                element = element.string(choice.axis.clone(), choice.option.clone());
            }
        }
        for slot in plan.slots.iter().filter(|s| *s != ir::DEFAULT_SLOT) {
            let content: Vec<Jsx> = node
                .children
                .iter()
                .filter(|c| ir::render::slot_target(self.doc, c) == slot.as_str())
                .flat_map(|c| self.node(c))
                .collect();
            if content.is_empty() {
                continue;
            }
            let value = if content.len() == 1 && matches!(content[0], Jsx::Element(_)) {
                content.into_iter().next().unwrap_or(Jsx::Fragment(Vec::new()))
            } else {
                Jsx::Fragment(content)
            };
            element = element.attr(slot.clone(), AttrValue::Expr(Expr::Jsx(Box::new(value))));
        }
        let default_content: Vec<Jsx> = node
            .children
            .iter()
            .filter(|c| ir::render::slot_target(self.doc, c) == ir::DEFAULT_SLOT)
            .flat_map(|c| self.node(c))
            .collect();
        element.children = default_content;
        vec![self.finish(node, element)]
    }

    /// Attributs communs (ancre, accessibilité, bascule, classes, échappatoires, id d'édition).
    fn finish(&mut self, node: &Node, mut element: Element) -> Jsx {
        let doc = self.doc;
        let toggle_target = self.toggles.iter().find(|t| t.target == node.id).cloned();
        // Racine d'un composant qui reçoit l'ancre de son instance.
        let forwarded_id = match self.scope {
            Scope::Component(component, plan) if plan.id_prop && component.root == node.id => {
                Some(match &node.meta.anchor {
                    Some(anchor) => format!("id ?? {}", crate::jsx::string_literal(anchor)),
                    None => "id".to_owned(),
                })
            }
            _ => None,
        };
        // Ancre, ou identifiant d'une cible de bascule.
        if let Some(expr) = forwarded_id {
            element.attrs.insert(
                0,
                crate::jsx::Attr {
                    name: "id".to_owned(),
                    value: AttrValue::Expr(Expr::Atom(expr)),
                },
            );
        } else if let Some(anchor) = &node.meta.anchor {
            element.attrs.insert(
                0,
                crate::jsx::Attr {
                    name: "id".to_owned(),
                    value: AttrValue::Str(anchor.clone()),
                },
            );
        } else if let Some(Toggle { id_var: Some(var), .. }) = &toggle_target {
            element.attrs.insert(
                0,
                crate::jsx::Attr {
                    name: "id".to_owned(),
                    value: AttrValue::Expr(Expr::Atom(var.clone())),
                },
            );
        }
        if let Some(toggle) = &toggle_target {
            element = element.attr("data-open", AttrValue::Expr(Expr::Atom(toggle.state.clone())));
        }
        if node.a11y.hidden && !matches!(node.kind, NodeKind::Icon(_)) {
            element = element.string("aria-hidden", "true");
        }
        if let Some(label) = node.a11y.label.as_ref().filter(|l| !l.trim().is_empty())
            && !matches!(node.kind, NodeKind::Icon(_))
        {
            element = element.string("aria-label", label.clone());
        }
        if let Some(web) = &node.platform_overrides.web {
            for attr in &web.extra_attributes {
                element = element.string(attr.name.clone(), attr.value.clone());
            }
        }
        // Racine d'un composant : les attributs de l'instance l'emportent sur les siens.
        if let Scope::Component(component, plan) = self.scope
            && plan.rest
            && component.root == node.id
        {
            element = element.attr("rest", AttrValue::Spread);
        }
        let display = display_of(doc, node);
        let mut classes = node_classes(doc, node, display);
        if toggle_target.is_some() {
            classes.extend(open_classes(node, display));
        }
        if let Some(class_attr) = self.class_attr(node, classes) {
            element = element.attr("className", class_attr);
        }
        if self.edit {
            element = element.string("data-atl-id", node.id.to_string());
        }
        element.node = Some(node.id.clone());
        Jsx::Element(element)
    }

    /// `className` : chaîne, ou fusion `twMerge` avec les variantes et le `className` reçu.
    fn class_attr(&mut self, node: &Node, classes: Vec<String>) -> Option<AttrValue> {
        let base = classes.join(" ");
        let mut parts: Vec<Expr> = Vec::new();
        if let Some((component, plan)) = self.component() {
            for axis in &component.variants {
                let targets = axis
                    .options
                    .iter()
                    .any(|o| o.overrides.iter().any(|ov| ov.node == node.id));
                if !targets {
                    continue;
                }
                let map = self.variant_map(component, axis, node);
                parts.push(Expr::Atom(format!("{map}[{}]", axis.name)));
            }
            if component.root == node.id && plan.class_name {
                parts.push(Expr::Atom("className".to_owned()));
            }
        }
        if parts.is_empty() {
            return (!base.is_empty()).then_some(AttrValue::Str(base));
        }
        if base.is_empty() && parts.len() == 1 && parts[0] == Expr::Atom("className".to_owned()) {
            return Some(AttrValue::Expr(Expr::Atom("className".to_owned())));
        }
        self.uses_twmerge = true;
        self.imports.named("tailwind-merge", "twMerge");
        let mut args = Vec::new();
        if !base.is_empty() {
            args.push(Expr::Str(base));
        }
        args.extend(parts);
        Some(AttrValue::Expr(Expr::Call {
            callee: "twMerge".to_owned(),
            args,
        }))
    }

    /// Table des classes d'un axe de variantes pour un nœud (`intentClasses`).
    fn variant_map(&mut self, component: &Component, axis: &ir::VariantAxis, node: &Node) -> String {
        let key = (axis.name.clone(), node.id.clone());
        if let Some(name) = self.variant_names.get(&key) {
            return name.clone();
        }
        let suffix = if node.id == component.root {
            String::new()
        } else {
            node.meta
                .name
                .as_deref()
                .map(pascal_case)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| node.kind.type_name().to_owned())
        };
        let mut taken: std::collections::BTreeSet<String> = self.variant_names.values().cloned().collect();
        let name = unique(&format!("{}{suffix}Classes", axis.name), &mut taken);
        let props: Vec<(String, Value)> = axis
            .options
            .iter()
            .map(|option| {
                let classes = option
                    .overrides
                    .iter()
                    .filter(|ov| ov.node == node.id)
                    .flat_map(|ov| {
                        let states: Vec<(ir::InteractionState, &ir::StatePatch)> = ir::InteractionState::ALL
                            .into_iter()
                            .filter_map(|s| ov.states.get(s).map(|p| (s, p)))
                            .collect();
                        patch_classes(self.doc, &ov.style, &states)
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                (option.name.clone(), Value::Str(classes))
            })
            .collect();
        self.items.push(Item::Const {
            export: false,
            name: name.clone(),
            annotation: None,
            props,
            as_const: true,
        });
        self.variant_names.insert(key, name.clone());
        name
    }
}

/// Segment d'un texte mis en forme, espaces normalisés comme dans le JSX exporté.
pub(crate) enum TextSegment<'a> {
    Text(String),
    /// Mots mis en forme par leur segment d'origine (`strong`, `em`, `code`, couleur).
    Formatted(&'a TextRun, String),
    Break,
}

/// Segments d'un texte : espaces réduits, textes voisins fusionnés, bords et sauts de ligne
/// nettoyés (partagé par l'export et le canvas).
pub(crate) fn text_segments(runs: &[TextRun]) -> Vec<TextSegment<'_>> {
    enum Seg<'a> {
        Text(String),
        Formatted(&'a TextRun, String),
        Br,
    }
    let mut segs: Vec<Seg<'_>> = Vec::new();
    for run in runs {
        let plain = !run.strong && !run.em && !run.code && run.color.is_none();
        for (i, line) in run.text.split('\n').enumerate() {
            if i > 0 {
                segs.push(Seg::Br);
            }
            let mut collapsed = String::new();
            let mut space = false;
            for c in line.chars() {
                if c.is_whitespace() {
                    space = true;
                } else {
                    if space {
                        collapsed.push(' ');
                    }
                    space = false;
                    collapsed.push(c);
                }
            }
            if space {
                collapsed.push(' ');
            }
            let lead = line.starts_with(char::is_whitespace);
            if lead && !collapsed.starts_with(' ') {
                collapsed.insert(0, ' ');
            }
            if collapsed.trim().is_empty() {
                if !collapsed.is_empty() {
                    segs.push(Seg::Text(" ".to_owned()));
                }
                continue;
            }
            if plain {
                segs.push(Seg::Text(collapsed));
                continue;
            }
            if collapsed.starts_with(' ') {
                segs.push(Seg::Text(" ".to_owned()));
            }
            segs.push(Seg::Formatted(run, collapsed.trim().to_owned()));
            if collapsed.ends_with(' ') {
                segs.push(Seg::Text(" ".to_owned()));
            }
        }
    }
    // Textes voisins fusionnés, espaces doublés réduits, bords et sauts de ligne nettoyés.
    let mut merged: Vec<Seg<'_>> = Vec::new();
    for seg in segs {
        match (merged.last_mut(), seg) {
            (Some(Seg::Text(previous)), Seg::Text(next)) => {
                previous.push_str(&next);
                while previous.contains("  ") {
                    *previous = previous.replace("  ", " ");
                }
            }
            (_, seg) => merged.push(seg),
        }
    }
    let count = merged.len();
    let mut out = Vec::new();
    for i in 0..count {
        let before_br = i + 1 < count && matches!(merged[i + 1], Seg::Br);
        let after_br = i > 0 && matches!(merged[i - 1], Seg::Br);
        match &merged[i] {
            Seg::Text(value) => {
                let mut value = value.as_str();
                if i == 0 || after_br {
                    value = value.trim_start();
                }
                if i + 1 == count || before_br {
                    value = value.trim_end();
                }
                if !value.is_empty() {
                    out.push(TextSegment::Text(value.to_owned()));
                }
            }
            Seg::Formatted(run, value) => out.push(TextSegment::Formatted(run, value.clone())),
            Seg::Br => out.push(TextSegment::Break),
        }
    }
    out
}

/// Élément d'un segment mis en forme : `strong`, `em`, `code` imbriqués, couleur sur le plus
/// externe (`span` sans autre mise en forme).
fn formatted(run: &TextRun, value: &str) -> Jsx {
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
    let mut jsx = Jsx::Text(escape_text(value));
    for (i, tag) in tags.iter().enumerate().rev() {
        let mut element = Element::new(*tag).child(jsx);
        if i == 0
            && let Some(color) = &run.color
        {
            element.attrs.push(crate::jsx::Attr {
                name: "className".to_owned(),
                value: AttrValue::Str(format!("text-{color}")),
            });
        }
        jsx = Jsx::Element(element);
    }
    jsx
}

/// Classes d'une cible de bascule ouverte : visible là où elle est masquée fermée.
pub(crate) fn open_classes(node: &Node, display: Display) -> Vec<String> {
    let Some(visibility) = &node.visibility else {
        return Vec::new();
    };
    let class = match display {
        Display::Block => "block",
        Display::Flex => "flex",
        Display::Grid => "grid",
        Display::Inline => "inline",
        Display::InlineBlock => "inline-block",
    };
    let mut out = Vec::new();
    let mut previous_hidden = false;
    for bp in ir::Breakpoint::ALL {
        let hidden = !*visibility.resolve(bp);
        if hidden && !previous_hidden {
            let prefix = match bp {
                ir::Breakpoint::Base => "",
                ir::Breakpoint::Sm => "sm:",
                ir::Breakpoint::Md => "md:",
                ir::Breakpoint::Lg => "lg:",
                ir::Breakpoint::Xl => "xl:",
                ir::Breakpoint::Xxl => "2xl:",
            };
            out.push(format!("{prefix}data-[open=true]:{class}"));
        }
        previous_hidden = hidden;
    }
    out
}
