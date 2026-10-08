//! Canvas : rendu d'une page pour l'éditeur, avec les classes de l'export et le style effectif de
//! chaque rendu d'instance.

use std::collections::BTreeSet;

use compiler_web::canvas::{CanvasElement, CanvasNode, CanvasPage, canvas_page};
use compiler_web::demo;
use ir::{Document, NodeId, NodeKind};

fn page(doc: &Document, index: usize) -> CanvasPage {
    canvas_page(doc, &doc.pages[index].id).expect("page exists")
}

fn elements(nodes: &[CanvasNode]) -> Vec<&CanvasElement> {
    let mut out = Vec::new();
    for node in nodes {
        if let CanvasNode::Element(element) = node {
            out.push(element);
            out.extend(elements(&element.children));
        }
    }
    out
}

fn text(nodes: &[CanvasNode]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            CanvasNode::Text { text } => text.clone(),
            CanvasNode::Element(element) => text(&element.children),
            CanvasNode::Raw(_) => String::new(),
        })
        .collect()
}

fn component_root(doc: &Document, name: &str) -> NodeId {
    doc.components
        .iter()
        .find(|c| c.name == name)
        .map(|c| c.root.clone())
        .unwrap_or_else(|| panic!("component {name}"))
}

fn rendered<'a>(page: &'a CanvasPage, node: &NodeId) -> Vec<&'a CanvasElement> {
    elements(&page.nodes)
        .into_iter()
        .filter(|e| e.node.as_ref() == Some(node))
        .collect()
}

fn has_class(element: &CanvasElement, class: &str) -> bool {
    element.class.split(' ').any(|c| c == class)
}

#[test]
fn every_rendered_element_has_a_unique_key_ending_with_its_node() {
    for doc in [demo::landing(), demo::kitchen_sink()] {
        for index in 0..doc.pages.len() {
            let page = page(&doc, index);
            let mut keys = BTreeSet::new();
            for element in elements(&page.nodes) {
                let Some(key) = &element.key else {
                    // Mise en forme d'un texte : jamais sélectionnable.
                    assert!(element.node.is_none());
                    continue;
                };
                let node = element.node.as_ref().expect("keyed elements render a node");
                assert!(key.ends_with(node.as_str()), "{key}");
                assert!(keys.insert(key.clone()), "duplicate key {key}");
            }
            assert!(!page.truncated);
        }
    }
}

#[test]
fn each_instance_renders_its_component_with_its_variant_and_props() {
    let doc = demo::landing();
    let home = page(&doc, 0);
    let cards = rendered(&home, &component_root(&doc, "PricingCard"));
    assert_eq!(cards.len(), 3);
    let instances: BTreeSet<_> = cards.iter().map(|c| c.instance.clone().expect("instance")).collect();
    assert_eq!(instances.len(), 3);
    for card in &cards {
        let instance = doc
            .node(card.instance.as_ref().expect("instance"))
            .expect("instance node");
        let NodeKind::ComponentInstance(props) = &instance.kind else {
            panic!("instance");
        };
        let featured = props.variants.iter().any(|v| v.option == "featured");
        assert_eq!(has_class(card, "bg-primary"), featured, "{}", card.class);
        assert_eq!(has_class(card, "bg-card"), !featured, "{}", card.class);
        // La description suit aussi la variante (contraste vérifié par la validation).
        let description = elements(&card.children)
            .into_iter()
            .find(|e| {
                e.tag == "p" && (has_class(e, "text-muted-foreground") || has_class(e, "text-primary-foreground/80"))
            })
            .expect("description");
        assert_eq!(has_class(description, "text-primary-foreground/80"), featured);
    }
    let titles: Vec<String> = cards.iter().map(|c| text(&c.children[..1])).collect();
    assert_eq!(titles, ["Découverte", "Pro", "Équipe"]);
}

#[test]
fn the_style_of_an_instance_reaches_its_component_root() {
    let doc = demo::landing();
    let home = page(&doc, 0);
    let form = rendered(&home, &component_root(&doc, "NewsletterForm"));
    assert_eq!(form.len(), 1);
    assert_eq!(form[0].tag, "form");
    assert!(has_class(form[0], "mx-auto"), "{}", form[0].class);

    let doc = demo::kitchen_sink();
    let cuisine = page(&doc, 1);
    let badges = rendered(&cuisine, &component_root(&doc, "Badge"));
    // Le badge dont la prop `show` vaut `false` n'est pas rendu.
    assert_eq!(badges.len(), 2);
    assert!(badges.iter().any(|b| has_class(b, "absolute") && has_class(b, "top-2")));
    let panels = rendered(&cuisine, &component_root(&doc, "Panel"));
    assert_eq!(panels.len(), 2);
    assert!(has_class(panels[0], "p-4"), "variante compact : {}", panels[0].class);
    assert_eq!(panels[0].attrs.get("id").map(String::as_str), Some("panneau"));
    assert_eq!(
        panels[1].attrs.get("aria-label").map(String::as_str),
        Some("Panneau secondaire")
    );
}

#[test]
fn toggle_buttons_point_to_their_target() {
    let doc = demo::landing();
    let home = page(&doc, 0);
    let all = elements(&home.nodes);
    let burger = all
        .iter()
        .find(|e| e.attrs.get("aria-label").map(String::as_str) == Some("Menu"))
        .expect("burger");
    let target = burger.toggles.as_ref().expect("toggle target");
    let nav = all.iter().find(|e| e.key.as_ref() == Some(target)).expect("menu");
    assert_eq!(nav.tag, "nav");
    assert!(
        has_class(nav, "hidden") && has_class(nav, "data-[open=true]:flex"),
        "{}",
        nav.class
    );
    assert_eq!(burger.children.len(), 1);
    let CanvasNode::Element(icon) = &burger.children[0] else {
        panic!("icon");
    };
    assert_eq!((icon.tag.as_str(), icon.icon.as_deref()), ("svg", Some("menu")));
}

#[test]
fn page_roots_without_style_are_fragments_like_the_export() {
    let doc = demo::landing();
    let home = page(&doc, 0);
    let tags: Vec<&str> = home
        .nodes
        .iter()
        .filter_map(|n| match n {
            CanvasNode::Element(e) => Some(e.tag.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(tags, ["header", "main", "footer"]);
}

#[test]
fn links_images_raw_code_and_fonts_are_ready_for_the_runtime() {
    let doc = demo::kitchen_sink();
    let cuisine = page(&doc, 1);
    let all = elements(&cuisine.nodes);
    assert!(all.iter().any(|e| e.tag == "a" && e.attrs.contains_key("data-page")));
    assert!(
        all.iter()
            .any(|e| e.tag == "a" && e.attrs.get("href").is_some_and(|h| h.starts_with("tel:")))
    );
    let images: Vec<_> = all.iter().filter(|e| e.tag == "img").collect();
    assert!(
        images
            .iter()
            .all(|i| i.attrs.contains_key("width") && i.attrs.contains_key("alt"))
    );
    assert!(images.iter().any(|i| i.attrs.contains_key("data-asset")));
    assert!(
        images
            .iter()
            .filter(|i| i.attrs.contains_key("data-asset"))
            .all(|i| i.attrs["src"].starts_with("data:image/svg+xml,"))
    );
    let raw: Vec<_> = raw_blocks(&cuisine.nodes);
    assert_eq!(raw.len(), 2);
    assert!(raw.iter().any(|r| r.client && r.code.contains("Bonjour")));
    // Un nœud natif seul n'apparaît pas.
    let native: Vec<&NodeId> = doc
        .nodes
        .values()
        .filter(|n| n.platform == ir::PlatformScope::NativeOnly)
        .map(|n| &n.id)
        .collect();
    assert!(!native.is_empty());
    assert!(all.iter().all(|e| e.node.as_ref().is_none_or(|n| !native.contains(&n))));

    assert_eq!(
        cuisine.font_stylesheets,
        ["https://fonts.googleapis.com/css2?family=Playfair+Display:wght@400;700&display=swap"]
    );
    assert!(!cuisine.css.contains("@import"));
    assert!(
        cuisine
            .css
            .contains("--next-font-playfair-display: \"Playfair Display\"")
    );
    assert!(cuisine.css.contains("@theme inline"));
    assert_eq!(cuisine.lang, "fr");
}

fn raw_blocks(nodes: &[CanvasNode]) -> Vec<&compiler_web::canvas::CanvasRaw> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            CanvasNode::Raw(raw) => out.push(raw),
            CanvasNode::Element(element) => out.extend(raw_blocks(&element.children)),
            CanvasNode::Text { .. } => {}
        }
    }
    out
}

#[test]
fn the_landing_canvas_snapshot() {
    let doc = demo::landing();
    let json = serde_json::to_string_pretty(&page(&doc, 0)).expect("JSON");
    insta::assert_snapshot!("canvas_landing_home", json);
}
