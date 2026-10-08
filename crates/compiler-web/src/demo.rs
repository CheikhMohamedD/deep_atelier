//! Landing de démonstration, construite par les commandes de l'IR comme le ferait l'éditeur :
//! layout (en-tête avec menu burger, pied de page), page d'accueil (hero, fonctionnalités,
//! tarifs, formulaire), page « À propos ». Elle sert aux snapshots du compilateur, au build réel
//! du projet exporté et à l'audit Lighthouse.

use ir::{Applied, ComponentId, Document, IdGen, NodeId, PageId, Session, Transaction};
use serde_json::{Value, json};

fn run(session: &mut Session, commands: Value) -> Applied {
    let commands = serde_json::from_value(commands).unwrap_or_else(|e| panic!("commandes de la démo invalides : {e}"));
    session
        .apply(Transaction::user("démo", commands))
        .unwrap_or_else(|e| panic!("la démo ne s'applique pas : {e}"))
}

fn insert(session: &mut Session, parent: &NodeId, nodes: Value) -> Applied {
    run(
        session,
        json!([{ "op": "insert_nodes", "parent": parent.to_string(), "nodes": nodes }]),
    )
}

/// Composant créé à partir d'un sous-arbre inséré sur la page `scratch`, dont l'instance créée
/// est ensuite retirée.
fn component(
    session: &mut Session,
    scratch: &NodeId,
    name: &str,
    nodes: Value,
    props: &[(&str, &str, &str)],
) -> ComponentId {
    let applied = insert(session, scratch, nodes);
    let props: Vec<Value> = props
        .iter()
        .map(|(name, node, field)| json!({ "name": name, "node": applied.created[*node].to_string(), "field": field }))
        .collect();
    let created = run(
        session,
        json!([{ "op": "create_component", "node": applied.created["$root"].to_string(), "name": name, "props": props }]),
    );
    run(
        session,
        json!([{ "op": "delete_nodes", "nodes": [created.inserted[0].to_string()] }]),
    );
    created.components[0].clone()
}

/// Nœud lié à une prop d'un composant.
fn bound_node(session: &Session, component: &ComponentId, prop: &str) -> NodeId {
    session
        .document()
        .component(component)
        .and_then(|c| c.prop(prop))
        .map(|p| p.binding.node.clone())
        .unwrap_or_else(|| panic!("prop `{prop}` absente"))
}

fn color(name: &str) -> Value {
    json!({ "base": { "kind": "Color", "color": name } })
}

fn page_link(page: &PageId, anchor: Option<&str>) -> Value {
    match anchor {
        Some(anchor) => json!({ "kind": "Page", "page": page.to_string(), "anchor": anchor }),
        None => json!({ "kind": "Page", "page": page.to_string() }),
    }
}

fn text(value: &str) -> Value {
    json!([{ "text": value }])
}

/// Bouton-lien principal.
fn primary_link_style() -> Value {
    json!({
        "padding_x": { "base": "6" }, "padding_y": { "base": "3" }, "radius": { "base": "md" },
        "background": color("primary"), "text_color": { "base": "primary-foreground" },
        "font_weight": { "base": "medium" }, "text_align": { "base": "center" }
    })
}

/// Document de la landing de démonstration.
pub fn landing() -> Document {
    let mut ids = IdGen::from_seed(2026);
    let doc = Document::new("Deep Atelier", &mut ids);
    let mut session = Session::new(doc, 1007);
    let home = session.document().pages[0].id.clone();
    let home_root = session.document().pages[0].root.clone();

    run(
        &mut session,
        json!([
            { "op": "update_settings", "lang": "fr", "site_name": "Deep Atelier" },
            { "op": "update_page", "page": home.to_string(), "name": "Accueil",
              "seo": { "title": "Deep Atelier — dessinez votre site, livrez du vrai code",
                       "description": "Un website builder pour développeurs : canvas visuel, prompt et code React propre que vous possédez." } }
        ]),
    );

    // ------------------------------------------------------------ composants
    let feature = component(
        &mut session,
        &home_root,
        "FeatureCard",
        json!([
            { "ref": "$root", "kind": { "type": "Box", "role": { "kind": "ListItem" } },
              "style": { "padding": { "base": "6" }, "radius": { "base": "xl" }, "border_width": { "base": "1" },
                         "border_color": { "base": "border" }, "background": color("card") } },
            { "parent": "$root", "kind": { "type": "Icon", "name": "zap" },
              "style": { "width": { "base": "6" }, "height": { "base": "6" }, "text_color": { "base": "primary" } } },
            { "ref": "$title", "parent": "$root", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h3" }, "content": text("Titre") },
              "style": { "margin_top": { "base": "4" }, "font_size": { "base": "lg" }, "font_weight": { "base": "semibold" } } },
            { "ref": "$description", "parent": "$root", "kind": { "type": "Text", "content": text("Description") },
              "style": { "margin_top": { "base": "2" }, "text_color": { "base": "muted-foreground" } } }
        ]),
        &[("title", "$title", "Text"), ("description", "$description", "Text")],
    );
    let pricing = component(
        &mut session,
        &home_root,
        "PricingCard",
        json!([
            { "ref": "$root", "kind": { "type": "Stack", "role": { "kind": "ListItem" } },
              "style": { "gap": { "base": "4" }, "padding": { "base": "8" }, "radius": { "base": "2xl" }, "border_width": { "base": "1" },
                         "border_color": { "base": "border" }, "background": color("card") } },
            { "ref": "$name", "parent": "$root", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h3" }, "content": text("Offre") },
              "style": { "font_size": { "base": "lg" }, "font_weight": { "base": "semibold" } } },
            { "ref": "$price", "parent": "$root", "kind": { "type": "Text", "content": text("0 €") },
              "style": { "font_size": { "base": "4xl" }, "font_weight": { "base": "bold" }, "letter_spacing": { "base": "tight" } } },
            { "ref": "$description", "parent": "$root", "kind": { "type": "Text", "content": text("Description de l'offre") },
              "style": { "text_color": { "base": "muted-foreground" } } },
            { "ref": "$cta", "parent": "$root", "kind": { "type": "Link", "label": "Choisir", "href": page_link(&home, Some("contact")) },
              "style": primary_link_style() }
        ]),
        &[
            ("name", "$name", "Text"),
            ("price", "$price", "Text"),
            ("description", "$description", "Text"),
            ("cta", "$cta", "Label"),
        ],
    );
    let pricing_root = session
        .document()
        .component(&pricing)
        .map(|c| c.root.clone())
        .unwrap_or_else(|| home_root.clone());
    let pricing_description = bound_node(&session, &pricing, "description");
    let pricing_cta = bound_node(&session, &pricing, "cta");
    run(
        &mut session,
        json!([{ "op": "update_component", "component": pricing.to_string(), "variants": [{
            "name": "tone", "default": "standard",
            "options": [
                { "name": "standard" },
                { "name": "featured", "overrides": [
                    { "node": pricing_root.to_string(),
                      "style": { "background": color("primary"), "text_color": { "base": "primary-foreground" },
                                 "border_color": { "base": "primary" } } },
                    { "node": pricing_description.to_string(), "style": { "text_color": { "base": "primary-foreground/80" } } },
                    { "node": pricing_cta.to_string(),
                      "style": { "background": color("background"), "text_color": { "base": "foreground" } } }
                ] }
            ]
        }] }]),
    );
    let newsletter = component(
        &mut session,
        &home_root,
        "NewsletterForm",
        json!([
            { "ref": "$root", "kind": { "type": "Stack", "role": { "kind": "Form", "method": "post" } },
              "style": { "max_width": { "base": "container.md" }, "margin_x": { "base": "auto" }, "gap": { "base": "4" } } },
            { "ref": "$title", "parent": "$root", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h2" }, "content": text("Restez informé") },
              "style": { "font_size": { "base": "2xl" }, "font_weight": { "base": "bold" } } },
            { "parent": "$root", "kind": { "type": "Text", "for_input": "$email", "content": text("Adresse e-mail") },
              "style": { "font_size": { "base": "sm" }, "font_weight": { "base": "medium" } } },
            { "ref": "$email", "parent": "$root", "kind": { "type": "Input", "input_type": "email", "name": "email", "required": true,
              "autocomplete": "email", "placeholder": "vous@exemple.fr" },
              "style": { "padding_x": { "base": "3" }, "padding_y": { "base": "2" }, "radius": { "base": "md" },
                         "border_width": { "base": "1" }, "border_color": { "base": "border" }, "background": color("background") } },
            { "ref": "$submit", "parent": "$root", "kind": { "type": "Button", "label": "S'inscrire", "button_type": "submit" },
              "style": { "padding_x": { "base": "4" }, "padding_y": { "base": "2" }, "radius": { "base": "md" },
                         "background": color("primary"), "text_color": { "base": "primary-foreground" },
                         "font_weight": { "base": "medium" } } }
        ]),
        &[("title", "$title", "Text"), ("submitLabel", "$submit", "Label")],
    );

    // ------------------------------------------------------------ layout
    let layout = run(&mut session, json!([{ "op": "create_layout", "name": "site" }])).layouts[0].clone();
    let layout_root = session
        .document()
        .layout(&layout)
        .map(|l| l.root.clone())
        .unwrap_or_else(|| home_root.clone());
    run(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": layout_root.to_string(), "index": 0, "nodes": [
            { "ref": "$header", "kind": { "type": "Box", "role": { "kind": "Header" } },
              "style": { "position": { "base": "sticky" }, "top": { "base": "0" }, "z_index": { "base": "40" },
                         "border_bottom_width": { "base": "1" }, "border_color": { "base": "border" },
                         "background": color("background") } },
            { "ref": "$bar", "parent": "$header", "kind": { "type": "Stack" },
              "style": { "direction": { "base": "row" }, "wrap": { "base": true }, "align": { "base": "center" },
                         "justify": { "base": "between" }, "gap": { "base": "4" }, "max_width": { "base": "container.6xl" },
                         "margin_x": { "base": "auto" }, "padding_x": { "base": "4" }, "padding_y": { "base": "3" } } },
            { "parent": "$bar", "kind": { "type": "Link", "label": "Deep Atelier", "href": page_link(&home, None) },
              "style": { "font_size": { "base": "lg" }, "font_weight": { "base": "semibold" } } },
            { "ref": "$burger", "parent": "$bar", "kind": { "type": "Button", "action": { "kind": "ToggleVisibility", "target": "$nav" } },
              "meta": { "a11y_label": "Menu" }, "visibility": { "base": true, "md": false },
              "style": { "padding": { "base": "2" }, "radius": { "base": "md" } } },
            { "parent": "$burger", "kind": { "type": "Icon", "name": "menu" },
              "style": { "width": { "base": "6" }, "height": { "base": "6" } } },
            { "ref": "$nav", "parent": "$bar", "kind": { "type": "Stack", "role": { "kind": "Nav" } },
              "meta": { "name": "Menu principal" }, "visibility": { "base": false, "md": true },
              "style": { "direction": { "base": "column", "md": "row" }, "gap": { "base": "4", "md": "6" },
                         "width": { "base": "full", "md": "auto" } } },
            { "parent": "$nav", "kind": { "type": "Link", "label": "Fonctionnalités", "href": page_link(&home, Some("features")) },
              "style": { "font_size": { "base": "sm" }, "text_color": { "base": "muted-foreground" } } },
            { "parent": "$nav", "kind": { "type": "Link", "label": "Tarifs", "href": page_link(&home, Some("pricing")) },
              "style": { "font_size": { "base": "sm" }, "text_color": { "base": "muted-foreground" } } }
        ] }]),
    );
    insert(
        &mut session,
        &layout_root,
        json!([
            { "ref": "$footer", "kind": { "type": "Box", "role": { "kind": "Footer" } },
              "style": { "border_top_width": { "base": "1" }, "border_color": { "base": "border" } } },
            { "ref": "$bottom", "parent": "$footer", "kind": { "type": "Stack" },
              "style": { "direction": { "base": "column", "md": "row" }, "justify": { "base": "between" }, "gap": { "base": "4" },
                         "max_width": { "base": "container.6xl" }, "margin_x": { "base": "auto" }, "padding_x": { "base": "4" },
                         "padding_y": { "base": "8" }, "font_size": { "base": "sm" }, "text_color": { "base": "muted-foreground" } } },
            { "parent": "$bottom", "kind": { "type": "Text", "content": text("© 2026 Deep Atelier. Code ouvert, sans dépendance.") } },
            { "parent": "$bottom", "kind": { "type": "Link", "label": "contact@example.com", "href": { "kind": "Email", "address": "contact@example.com" } },
              "style": { "text_decoration": { "base": "underline" } } }
        ]),
    );
    run(
        &mut session,
        json!([{ "op": "update_page", "page": home.to_string(), "layout": layout.to_string() }]),
    );

    // ------------------------------------------------------------ accueil
    let section = |anchor: Option<&str>, background: Option<&str>| {
        let mut spec = json!({ "kind": { "type": "Box", "role": { "kind": "Section" } },
                               "style": { "padding_x": { "base": "4" }, "padding_y": { "base": "16", "md": "24" } } });
        if let Some(anchor) = anchor {
            spec["meta"] = json!({ "anchor": anchor });
        }
        if let Some(background) = background {
            spec["style"]["background"] = color(background);
        }
        spec
    };
    let heading_style = json!({ "font_size": { "base": "3xl", "md": "4xl" }, "font_weight": { "base": "bold" },
                                "letter_spacing": { "base": "tight" }, "text_align": { "base": "center" },
                                "text_wrap": { "base": "balance" } });
    let mut hero = section(None, None);
    hero["ref"] = json!("$hero");
    hero["parent"] = json!("$main");
    hero["style"]["padding_y"] = json!({ "base": "20", "md": "32" });
    let mut features = section(Some("features"), Some("muted"));
    features["ref"] = json!("$features");
    features["parent"] = json!("$main");
    let mut prices = section(Some("pricing"), None);
    prices["ref"] = json!("$pricing");
    prices["parent"] = json!("$main");
    let mut contact = section(Some("contact"), Some("muted"));
    contact["ref"] = json!("$contact");
    contact["parent"] = json!("$main");
    let instance = |component: &ComponentId, overrides: Value, variants: Value| json!({ "type": "ComponentInstance", "component": component.to_string(), "overrides": overrides, "variants": variants });
    let prop = |name: &str, value: &str| json!({ "prop": name, "value": { "kind": "Text", "value": value } });
    insert(
        &mut session,
        &home_root,
        json!([
            { "ref": "$main", "kind": { "type": "Box", "role": { "kind": "Main" } } },
            hero,
            { "ref": "$heroInner", "parent": "$hero", "kind": { "type": "Stack" },
              "style": { "max_width": { "base": "container.4xl" }, "margin_x": { "base": "auto" }, "align": { "base": "center" },
                         "text_align": { "base": "center" }, "gap": { "base": "6" } } },
            { "parent": "$heroInner", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" },
              "content": [{ "text": "Dessinez votre site, " }, { "text": "livrez du vrai code", "color": "primary" }] },
              "style": { "font_size": { "base": "4xl", "md": "6xl" }, "font_weight": { "base": "bold" },
                         "letter_spacing": { "base": "tight" }, "text_wrap": { "base": "balance" } } },
            { "parent": "$heroInner", "kind": { "type": "Text", "content": text("Deep Atelier transforme chaque geste sur le canvas et chaque prompt en code React, TypeScript et Tailwind que vous possédez, sans dépendance au builder.") },
              "style": { "max_width": { "base": "container.2xl" }, "font_size": { "base": "lg", "md": "xl" },
                         "text_color": { "base": "muted-foreground" }, "text_wrap": { "base": "pretty" } } },
            { "ref": "$ctas", "parent": "$heroInner", "kind": { "type": "Stack" },
              "style": { "direction": { "base": "column", "sm": "row" }, "justify": { "base": "center" }, "gap": { "base": "3" },
                         "width": { "base": "full", "sm": "auto" } } },
            { "parent": "$ctas", "kind": { "type": "Link", "label": "Commencer gratuitement", "href": page_link(&home, Some("pricing")) },
              "style": primary_link_style() },
            { "parent": "$ctas", "kind": { "type": "Link", "label": "Voir les fonctionnalités", "href": page_link(&home, Some("features")) },
              "style": { "padding_x": { "base": "6" }, "padding_y": { "base": "3" }, "radius": { "base": "md" },
                         "border_width": { "base": "1" }, "border_color": { "base": "border" },
                         "font_weight": { "base": "medium" }, "text_align": { "base": "center" } } },
            { "parent": "$heroInner", "kind": { "type": "Image", "alt": "Aperçu de l'éditeur Deep Atelier", "priority": true,
              "source": { "kind": "Placeholder", "label": "Aperçu de l'éditeur", "ratio": "video" } },
              "style": { "width": { "base": "full" }, "margin_top": { "base": "6" }, "radius": { "base": "xl" },
                         "border_width": { "base": "1" }, "border_color": { "base": "border" } } },
            features,
            { "ref": "$featuresInner", "parent": "$features", "kind": { "type": "Stack" },
              "style": { "max_width": { "base": "container.6xl" }, "margin_x": { "base": "auto" }, "gap": { "base": "10" } } },
            { "parent": "$featuresInner", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h2" }, "content": text("Tout pour livrer vite") },
              "style": heading_style.clone() },
            { "ref": "$featureGrid", "parent": "$featuresInner", "kind": { "type": "Grid", "role": { "kind": "List" } },
              "style": { "columns": { "base": "1", "md": "2", "lg": "3" }, "gap": { "base": "6" } } },
            { "parent": "$featureGrid", "kind": instance(&feature, json!([prop("title", "Canvas d'abord"), prop("description", "Construisez à la souris sur un vrai rendu, en mobile d'abord.")]), json!([])) },
            { "parent": "$featureGrid", "kind": instance(&feature, json!([prop("title", "Prompt intégré"), prop("description", "Décrivez une section : l'IA écrit des commandes que vous pouvez éditer.")]), json!([])) },
            { "parent": "$featureGrid", "kind": instance(&feature, json!([prop("title", "Code à vous"), prop("description", "Next.js, TypeScript strict et Tailwind, exportés sans aucune dépendance.")]), json!([])) },
            prices,
            { "ref": "$pricingInner", "parent": "$pricing", "kind": { "type": "Stack" },
              "style": { "max_width": { "base": "container.5xl" }, "margin_x": { "base": "auto" }, "gap": { "base": "10" } } },
            { "parent": "$pricingInner", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h2" }, "content": text("Des tarifs simples") },
              "style": heading_style.clone() },
            { "ref": "$pricingGrid", "parent": "$pricingInner", "kind": { "type": "Grid", "role": { "kind": "List" } },
              "style": { "columns": { "base": "1", "md": "3" }, "gap": { "base": "6" } } },
            { "parent": "$pricingGrid", "kind": instance(&pricing, json!([prop("name", "Découverte"), prop("price", "0 €"), prop("description", "Un projet, export ZIP."), prop("cta", "Essayer")]), json!([])) },
            { "parent": "$pricingGrid", "kind": instance(&pricing, json!([prop("name", "Pro"), prop("price", "19 €"), prop("description", "Projets illimités, push GitHub."), prop("cta", "Passer Pro")]), json!([{ "axis": "tone", "option": "featured" }])) },
            { "parent": "$pricingGrid", "kind": instance(&pricing, json!([prop("name", "Équipe"), prop("price", "49 €"), prop("description", "Collaboration et bibliothèque partagée."), prop("cta", "Nous contacter")]), json!([])) },
            contact,
            { "parent": "$contact", "kind": instance(&newsletter, json!([]), json!([])),
              "style": { "margin_x": { "base": "auto" } } }
        ]),
    );

    // ------------------------------------------------------------ à propos
    let about = run(
        &mut session,
        json!([{ "op": "create_page", "name": "À propos", "route": [{ "kind": "Static", "name": "a-propos" }],
                 "layout": layout.to_string(),
                 "seo": { "title": "À propos — Deep Atelier", "description": "Pourquoi nous construisons un builder dont le code vous appartient." } }]),
    )
    .pages[0]
        .clone();
    let about_root = session
        .document()
        .page(&about)
        .map(|p| p.root.clone())
        .unwrap_or_else(|| home_root.clone());
    insert(
        &mut session,
        &about_root,
        json!([
            { "ref": "$main", "kind": { "type": "Box", "role": { "kind": "Main" } },
              "style": { "max_width": { "base": "container.3xl" }, "margin_x": { "base": "auto" },
                         "padding_x": { "base": "4" }, "padding_y": { "base": "16", "md": "24" } } },
            { "parent": "$main", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": text("À propos") },
              "style": { "font_size": { "base": "4xl" }, "font_weight": { "base": "bold" }, "letter_spacing": { "base": "tight" } } },
            { "parent": "$main", "kind": { "type": "Text", "content": [
                { "text": "Deep Atelier part d'une idée simple : " },
                { "text": "le canvas d'abord", "strong": true },
                { "text": ", le code toujours. Chaque action produit du code " },
                { "text": "lisible", "em": true },
                { "text": " que vous pouvez relire, modifier et pousser sur GitHub." }
              ] },
              "style": { "margin_top": { "base": "6" }, "font_size": { "base": "lg" }, "text_color": { "base": "muted-foreground" } } },
            { "parent": "$main", "kind": { "type": "Image", "alt": "L'équipe au travail",
              "source": { "kind": "Placeholder", "label": "Équipe", "ratio": "landscape" } },
              "style": { "width": { "base": "full" }, "margin_top": { "base": "10" }, "radius": { "base": "xl" } } }
        ]),
    );
    session.document().clone()
}

/// Document « tout-en-un » : chaque construction que le compilateur sait émettre (slots nommés,
/// variantes sur des nœuds internes, prop `Visible` sur une racine, composant client, code libre,
/// police Google, image distante, assets, texte riche et entités, dégradés, états, visibilité,
/// nœud natif seulement, échappatoires web, route dynamique). Il doit passer Prettier, TypeScript
/// et ESLint comme la landing.
pub fn kitchen_sink() -> Document {
    let mut ids = IdGen::from_seed(313);
    let doc = Document::new("Atelier cuisine", &mut ids);
    let mut session = Session::new(doc, 2602);
    let home = session.document().pages[0].id.clone();
    let home_root = session.document().pages[0].root.clone();
    run(
        &mut session,
        json!([
            { "op": "set_token", "edit": { "kind": "Font", "name": "display",
              "value": { "kind": "Google", "family": "Playfair Display", "weights": [400, 700] } } },
            { "op": "add_asset", "asset": { "id": "a_0000000001", "file_name": "Logo Atelier.png", "mime": "image/png",
              "storage_path": "assets/logo.png", "width": 64, "height": 64, "bytes": 2048 } },
            { "op": "add_asset", "asset": { "id": "a_0000000002", "file_name": "partage.jpg", "mime": "image/jpeg",
              "storage_path": "assets/og.jpg", "width": 1200, "height": 630, "bytes": 40960 } },
            { "op": "update_settings", "lang": "fr", "favicon": "a_0000000001" }
        ]),
    );

    // ------------------------------------------------------------ composants
    let panel = component(
        &mut session,
        &home_root,
        "Panel",
        json!([
            { "ref": "$root", "kind": { "type": "Stack", "role": { "kind": "Article" } },
              "style": { "gap": { "base": "4" }, "padding": { "base": "6" }, "radius": { "base": "lg" },
                         "border_width": { "base": "1" }, "border_color": { "base": "border" } } },
            { "ref": "$heading", "parent": "$root", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h2" }, "content": text("Panneau") },
              "style": { "font_size": { "base": "xl" }, "font_weight": { "base": "semibold" } } },
            { "ref": "$body", "parent": "$root", "kind": { "type": "Stack" }, "meta": { "name": "Body" },
              "style": { "gap": { "base": "3" } } },
            { "parent": "$body", "kind": { "type": "Slot" } },
            { "ref": "$aside", "parent": "$root", "kind": { "type": "Box", "role": { "kind": "Aside" } },
              "style": { "font_size": { "base": "sm" }, "text_color": { "base": "muted-foreground" } } },
            { "parent": "$aside", "kind": { "type": "Slot", "name": "aside" } }
        ]),
        &[("heading", "$heading", "Text"), ("showHeading", "$heading", "Visible"), ("showAside", "$aside", "Visible")],
    );
    let panel_root = session
        .document()
        .component(&panel)
        .map(|c| c.root.clone())
        .unwrap_or_else(|| home_root.clone());
    let panel_body = session
        .document()
        .nodes
        .values()
        .find(|n| n.meta.name.as_deref() == Some("Body"))
        .map(|n| n.id.clone())
        .unwrap_or_else(|| home_root.clone());
    run(
        &mut session,
        json!([{ "op": "update_component", "component": panel.to_string(), "variants": [{
            "name": "density", "default": "comfortable",
            "options": [
                { "name": "compact", "overrides": [
                    { "node": panel_root.to_string(), "style": { "padding": { "base": "4" } } },
                    { "node": panel_body.to_string(), "style": { "gap": { "base": "2" } } }
                ] },
                { "name": "comfortable", "overrides": [
                    { "node": panel_root.to_string(), "style": { "padding": { "base": "8" } } },
                    { "node": panel_body.to_string(), "style": { "gap": { "base": "6" } } }
                ] }
            ]
        }] }]),
    );
    let badge = component(
        &mut session,
        &home_root,
        "Badge",
        json!([
            { "ref": "$root", "kind": { "type": "Text", "role": { "kind": "Inline" }, "content": text("Nouveau") },
              "style": { "padding_x": { "base": "2" }, "padding_y": { "base": "0.5" }, "radius": { "base": "full" },
                         "font_size": { "base": "xs" }, "font_weight": { "base": "medium" },
                         "background": color("accent"), "text_color": { "base": "accent-foreground" },
                         "transition": { "base": "colors" } },
              "hover": { "background": color("primary"), "text_color": { "base": "primary-foreground" } } }
        ]),
        &[("label", "$root", "Text"), ("show", "$root", "Visible")],
    );
    let disclosure = component(
        &mut session,
        &home_root,
        "Disclosure",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" }, "style": { "gap": { "base": "2" } } },
            { "parent": "$root", "kind": { "type": "Button", "label": "Afficher les détails", "action": { "kind": "ToggleVisibility", "target": "$details" } },
              "style": { "padding_x": { "base": "3" }, "padding_y": { "base": "2" }, "border_width": { "base": "1" },
                         "border_color": { "base": "border" }, "radius": { "base": "md" } } },
            { "ref": "$details", "parent": "$root", "kind": { "type": "Box" }, "meta": { "name": "Détails" },
              "visibility": { "base": false } },
            { "parent": "$details", "kind": { "type": "Text", "content": text("Les détails apparaissent au clic.") } }
        ]),
        &[],
    );
    let profile = component(
        &mut session,
        &home_root,
        "Profile",
        json!([
            { "ref": "$root", "kind": { "type": "Stack" }, "style": { "direction": { "base": "row" }, "align": { "base": "center" }, "gap": { "base": "3" } } },
            { "ref": "$photo", "parent": "$root", "kind": { "type": "Image", "alt": "Portrait",
              "source": { "kind": "Placeholder", "label": "Portrait", "ratio": "square" } },
              "style": { "width": { "base": "12" }, "height": { "base": "12" }, "radius": { "base": "full" }, "object_fit": { "base": "cover" } } },
            { "ref": "$link", "parent": "$root", "kind": { "type": "Link", "label": "Profil", "href": page_link(&home, None) },
              "style": { "font_weight": { "base": "medium" } } }
        ]),
        &[
            ("photo", "$photo", "ImageSource"),
            ("photoAlt", "$photo", "ImageAlt"),
            ("url", "$link", "Href"),
            ("linkLabel", "$link", "Label"),
        ],
    );

    // ------------------------------------------------------------ accueil
    insert(
        &mut session,
        &home_root,
        json!([
            { "ref": "$main", "kind": { "type": "Box", "role": { "kind": "Main" } }, "style": { "padding": { "base": "8" } } },
            { "parent": "$main", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": text("Accueil") } }
        ]),
    );

    // ------------------------------------------------------------ cuisine
    let kitchen = run(
        &mut session,
        json!([{ "op": "create_page", "name": "Cuisine", "route": [{ "kind": "Static", "name": "cuisine" }, { "kind": "Param", "name": "slug" }],
                 "seo": { "title": "Cuisine", "description": "Toutes les constructions du compilateur.", "og_image": "a_0000000002" } }]),
    )
    .pages[0]
        .clone();
    let kitchen_root = session
        .document()
        .page(&kitchen)
        .map(|p| p.root.clone())
        .unwrap_or_else(|| home_root.clone());
    let instance = |component: &ComponentId, overrides: Value, variants: Value| json!({ "type": "ComponentInstance", "component": component.to_string(), "overrides": overrides, "variants": variants });
    let applied = insert(
        &mut session,
        &kitchen_root,
        json!([
            { "ref": "$main", "kind": { "type": "Box", "role": { "kind": "Main" } },
              "style": { "max_width": { "base": "container.5xl" }, "margin_x": { "base": "auto" }, "padding_x": { "base": "4" },
                         "padding_y": { "base": "12" } } },
            { "parent": "$main", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": text("La cuisine du compilateur") },
              "style": { "font_family": { "base": "display" }, "font_size": { "base": "4xl", "lg": "5xl" }, "font_weight": { "base": "bold" } } },
            { "ref": "$rich", "parent": "$main", "kind": { "type": "Text", "content": [
                { "text": "L'outil \"simple\" & rapide < > { } : lancez " },
                { "text": "pnpm build", "code": true },
                { "text": " puis " },
                { "text": "tout", "strong": true, "em": true },
                { "text": " s'affiche en " },
                { "text": "couleur", "color": "primary" },
                { "text": ".\nUne nouvelle ligne suit le saut, avec une phrase assez longue pour que Prettier la remplisse sur plusieurs lignes." }
              ] },
              "style": { "margin_top": { "base": "4" }, "text_color": { "base": "muted-foreground" } } },
            { "ref": "$grid", "parent": "$main", "kind": { "type": "Grid" },
              "style": { "columns": { "base": "1", "md": "2", "lg": "4" }, "gap": { "base": "4" }, "margin_top": { "base": "8" } } },
            { "parent": "$grid", "kind": { "type": "Box" },
              "style": { "col_span": { "md": "2" }, "aspect_ratio": { "base": "video" }, "radius": { "base": "xl" }, "shadow": { "base": "md" },
                         "background": { "base": { "kind": "Gradient", "direction": "to-br", "from": "primary", "via": "accent", "to": "secondary" } } } },
            { "ref": "$relative", "parent": "$grid", "kind": { "type": "Box" },
              "style": { "position": { "base": "relative" }, "min_height": { "base": "24" }, "radius": { "base": "xl" },
                         "background": color("muted") } },
            { "parent": "$relative", "kind": instance(&badge, json!([{ "prop": "label", "value": { "kind": "Text", "value": "Bêta" } }]), json!([])),
              "style": { "position": { "base": "absolute" }, "top": { "base": "2" }, "right": { "base": "2" }, "z_index": { "base": "10" } } },
            { "parent": "$grid", "kind": { "type": "Box" }, "visibility": { "base": false, "md": true },
              "style": { "opacity": { "base": "50" }, "radius": { "base": "xl" }, "background": color("secondary") } },
            { "ref": "$native", "parent": "$grid", "kind": { "type": "Box" }, "style": { "background": color("card") } },
            { "ref": "$panel", "parent": "$main", "kind": instance(&panel,
                json!([{ "prop": "heading", "value": { "kind": "Text", "value": "Panneau compact" } }]),
                json!([{ "axis": "density", "option": "compact" }])),
              "meta": { "anchor": "panneau" }, "style": { "margin_top": { "base": "8" } } },
            { "parent": "$panel", "kind": { "type": "Text", "content": text("Contenu principal du panneau.") } },
            { "parent": "$panel", "kind": { "type": "Text", "content": text("Note en marge.") }, "meta": { "slot": "aside" } },
            { "parent": "$panel", "kind": { "type": "Link", "label": "Plus d'infos", "href": { "kind": "Anchor", "anchor": "panneau" } }, "meta": { "slot": "aside" } },
            { "ref": "$panel2", "parent": "$main",
              "kind": instance(&panel, json!([{ "prop": "showHeading", "value": { "kind": "Bool", "value": false } }]), json!([])),
              "style": { "margin_top": { "base": "4" } },
              "meta": { "a11y_label": "Panneau secondaire" } },
            { "parent": "$panel2", "kind": { "type": "Text", "content": text("Une seule note.") }, "meta": { "slot": "aside" } },
            { "ref": "$badges", "parent": "$main", "kind": { "type": "Stack" },
              "style": { "direction": { "base": "row" }, "gap": { "base": "2" }, "margin_top": { "base": "4" } } },
            { "parent": "$badges", "kind": instance(&badge, json!([{ "prop": "show", "value": { "kind": "Bool", "value": false } }]), json!([])) },
            { "parent": "$badges", "kind": instance(&badge, json!([{ "prop": "label", "value": { "kind": "Text", "value": "Stable" } }]), json!([])) },
            { "ref": "$profile", "parent": "$main", "kind": instance(&profile, json!([
                { "prop": "photo", "value": { "kind": "Image", "value": { "kind": "Url", "url": "https://images.example.com/portraits/ada.jpg" } } },
                { "prop": "photoAlt", "value": { "kind": "Text", "value": "Portrait d'Ada" } },
                { "prop": "url", "value": { "kind": "Href", "value": { "kind": "External", "url": "https://example.com/ada" } } },
                { "prop": "linkLabel", "value": { "kind": "Text", "value": "Ada Lovelace" } }
              ]), json!([])), "style": { "margin_top": { "base": "6" } } },
            { "parent": "$main", "kind": instance(&disclosure, json!([]), json!([])), "style": { "margin_top": { "base": "6" } } },
            { "ref": "$links", "parent": "$main", "kind": { "type": "Stack", "role": { "kind": "Nav" } },
              "meta": { "a11y_label": "Liens utiles" },
              "style": { "direction": { "base": "column", "sm": "row" }, "gap": { "base": "4" }, "margin_top": { "base": "6" } } },
            { "parent": "$links", "kind": { "type": "Link", "label": "Documentation", "new_tab": true,
              "href": { "kind": "External", "url": "https://example.com/docs" } },
              "style": { "text_decoration": { "base": "underline" } }, "hover": { "text_color": { "base": "primary" } } },
            { "parent": "$links", "kind": { "type": "Link", "label": "Appeler", "href": { "kind": "Phone", "number": "+33 1 23 45 67 89" } } },
            { "parent": "$links", "kind": { "type": "Link", "label": "Écrire", "href": { "kind": "Email", "address": "bonjour@example.com" } } },
            { "parent": "$links", "kind": { "type": "Link", "label": "Accueil", "href": page_link(&home, None) } },
            { "parent": "$links", "kind": { "type": "Link", "label": "Panneau", "href": { "kind": "Anchor", "anchor": "panneau" } } },
            { "ref": "$form", "parent": "$main", "kind": { "type": "Stack", "role": { "kind": "Form", "action": "https://example.com/newsletter", "method": "get" } },
              "style": { "gap": { "base": "2" }, "margin_top": { "base": "8" }, "max_width": { "base": "container.sm" } } },
            { "parent": "$form", "kind": { "type": "Text", "for_input": "$name", "content": text("Nom complet") } },
            { "ref": "$name", "parent": "$form", "kind": { "type": "Input", "name": "full name", "autocomplete": "name" },
              "style": { "border_width": { "base": "1" }, "border_color": { "base": "border" }, "padding_x": { "base": "3" }, "padding_y": { "base": "2" } } },
            { "parent": "$form", "kind": { "type": "Text", "for_input": "$message", "content": text("Message") } },
            { "ref": "$message", "parent": "$form", "kind": { "type": "Input", "input_type": "multiline", "name": "message", "placeholder": "Votre message…" },
              "style": { "border_width": { "base": "1" }, "border_color": { "base": "border" }, "padding_x": { "base": "3" }, "padding_y": { "base": "2" } } },
            { "parent": "$form", "kind": { "type": "Button", "label": "Envoyer", "button_type": "submit" },
              "style": { "padding_x": { "base": "4" }, "padding_y": { "base": "2" }, "background": color("primary"),
                         "text_color": { "base": "primary-foreground" }, "radius": { "base": "md" } } },
            { "parent": "$main", "kind": { "type": "RawCode", "code": "<time dateTime=\"2026-10-08\">8 octobre 2026</time>" } },
            { "parent": "$main", "kind": { "type": "RawCode", "code": "<button type=\"button\" onClick={() => window.alert(\"Bonjour\")}>\n  Dire bonjour\n</button>", "client": true }, "meta": { "name": "Bonjour" } },
            { "parent": "$main", "kind": { "type": "Image", "alt": "Logo de l'atelier", "source": { "kind": "Asset", "id": "a_0000000001" },
              "intrinsic": { "width": 64, "height": 64 } },
              "style": { "width": { "base": "16" }, "margin_top": { "base": "6" } } },
            { "ref": "$escape", "parent": "$main", "kind": { "type": "Text", "content": text("Texte avec échappatoires web.") } }
        ]),
    );
    run(
        &mut session,
        json!([
            { "op": "set_platform", "node": applied.created["$native"].to_string(), "scope": "NativeOnly" },
            { "op": "set_web_overrides", "node": applied.created["$escape"].to_string(),
              "overrides": { "extra_classes": ["underline-offset-4"], "extra_attributes": [{ "name": "data-testid", "value": "escape" }] } },
            { "op": "set_web_overrides", "node": applied.created["$profile"].to_string(),
              "overrides": { "extra_attributes": [{ "name": "data-testid", "value": "profil" }] } }
        ]),
    );
    session.document().clone()
}
