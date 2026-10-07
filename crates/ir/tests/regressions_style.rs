//! Régressions de la revue (a), groupe style : patchs responsive et schémas des valeurs.

mod common;

use common::*;
use ir::command::Command;
use ir::*;
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::TestRunner;
use serde_json::{Value, json};

fn command_defs() -> Value {
    let schema = serde_json::to_value(schemars::schema_for!(Command)).unwrap();
    schema["$defs"].clone()
}

/// Chaînes tirées (de façon déterministe) du langage d'un motif ancré `^…$`.
fn samples(pattern: &str, count: usize) -> Vec<String> {
    let inner = pattern
        .strip_prefix('^')
        .and_then(|p| p.strip_suffix('$'))
        .expect("anchored pattern");
    let strategy = proptest::string::string_regex(inner).expect("supported pattern");
    let mut runner = TestRunner::deterministic();
    (0..count)
        .map(|_| strategy.new_tree(&mut runner).unwrap().current())
        .collect()
}

fn has_style_not_allowed(issues: &[Issue]) -> bool {
    issues.iter().any(|i| i.code == IssueCode::StyleNotAllowed)
}

// ------------------------------------------------------------------ #13

#[test]
fn clearing_overrides_on_an_absent_property_leaves_it_absent() {
    let mut session = session();
    let root = home_root(&session);
    let before = session.document().clone();
    // `gap` n'est pas permis sur une Box : le créer ferait apparaître STYLE_NOT_ALLOWED.
    let applied = run_json(
        &mut session,
        json!([
            { "op": "set_style", "node": root.to_string(), "style": {
                "gap": { "md": null },
                "opacity": {},
                "radius": { "sm": null, "2xl": null }
            } },
            { "op": "set_style", "node": root.to_string(), "state": "hover", "style": { "opacity": { "lg": null } } },
            { "op": "set_visibility", "node": root.to_string(), "visibility": { "md": null } }
        ]),
    );
    assert!(applied.ops.is_empty(), "{:?}", applied.ops);
    assert_eq!(session.document(), &before);
    assert!(!session.can_undo(), "a no-op command leaves no undo entry");
    assert!(!has_style_not_allowed(&applied.issues), "{:?}", applied.issues);

    // Prompt limité au breakpoint `md` : effacer une surcharge absente reste sans effet.
    let md = Scope {
        breakpoint: Some(Breakpoint::Md),
        ..Scope::full()
    };
    let applied = try_run(
        &mut session,
        Origin::User,
        md,
        json!([{ "op": "set_style", "node": root.to_string(), "style": { "gap": { "md": null } } }]),
    )
    .expect("clearing `md` stays within the `md` scope");
    assert!(applied.ops.is_empty(), "{:?}", applied.ops);
    assert!(session.document().nodes[&root].style.gap.is_none());

    // Poser une surcharge crée toujours la propriété avec sa base neutre.
    run_json(
        &mut session,
        json!([{ "op": "set_style", "node": root.to_string(), "style": { "opacity": { "md": null, "lg": "50" } } }]),
    );
    let opacity = session.document().nodes[&root].style.opacity.clone().unwrap();
    assert_eq!(
        (opacity.base, opacity.md, opacity.lg),
        (Opacity::O100, None, Some(Opacity::O50))
    );

    // Effacer une surcharge d'une propriété présente l'efface et garde la base.
    run_json(
        &mut session,
        json!([{ "op": "set_style", "node": root.to_string(), "style": { "opacity": { "lg": null } } }]),
    );
    let opacity = session.document().nodes[&root].style.opacity.clone().unwrap();
    assert_eq!(opacity, Responsive::new(Opacity::O100));
}

// ------------------------------------------------------------------ #14

#[test]
fn null_base_in_a_responsive_patch_is_rejected() {
    let error = serde_json::from_value::<ResponsivePatch<Size>>(json!({ "base": null })).unwrap_err();
    assert!(error.to_string().contains("`base` cannot be null"), "{error}");
    let command = json!([{ "op": "set_style", "node": "n_0123456789", "style": { "radius": { "base": null } } }]);
    let error = serde_json::from_value::<Vec<Command>>(command).unwrap_err();
    assert!(error.to_string().contains("`base` cannot be null"), "{error}");
    let command = json!([{ "op": "set_visibility", "node": "n_0123456789", "visibility": { "base": null } }]);
    assert!(serde_json::from_value::<Vec<Command>>(command).is_err());

    // `base` absent ou valué, et `null` sur un breakpoint, restent acceptés.
    let patch: ResponsivePatch<Size> = json(json!({ "md": null }));
    assert_eq!(patch.touched(), vec![Breakpoint::Md]);
    let patch: ResponsivePatch<Size> = json(json!({ "base": "full" }));
    assert_eq!(patch.base, Some(Size::Full));

    // Le schéma des outils ne présente plus `null` comme valeur possible de `base`.
    let defs = command_defs();
    let patches: Vec<_> = defs
        .as_object()
        .unwrap()
        .iter()
        .filter(|(name, _)| name.starts_with("ResponsivePatch_"))
        .collect();
    assert!(patches.len() > 10, "{:?}", defs.as_object().unwrap().keys());
    for (name, patch) in patches {
        let base = &patch["properties"]["base"];
        assert!(!base.is_null(), "`{name}` has a `base` property");
        assert!(!base.to_string().contains("null"), "`{name}`.base is nullable: {base}");
        let md = &patch["properties"]["md"];
        assert!(md.to_string().contains("null"), "`{name}`.md stays nullable: {md}");
        let required = patch["required"].as_array().cloned().unwrap_or_default();
        assert!(!required.contains(&json!("base")), "`{name}`.base stays optional");
    }
}

// ------------------------------------------------------------------ #15

#[test]
fn size_schema_px_pattern_matches_the_parser() {
    let defs = command_defs();
    let pattern = defs["Size"]["anyOf"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|branch| branch["pattern"].as_str())
        .expect("px pattern")
        .to_owned();
    let values = samples(&pattern, 2000);
    let mut largest = 0;
    for value in &values {
        let Ok(Size::Px(px)) = value.parse::<Size>() else {
            panic!("`{value}` matches the Size schema but does not parse");
        };
        largest = largest.max(px);
    }
    assert!(largest > 999, "samples reach four digits: {largest}");
    assert!("4000px".parse::<Size>().is_ok());
    assert!("4001px".parse::<Size>().is_err());
}

#[test]
fn token_name_schema_bounds_length_and_documents_reserved_names() {
    let defs = command_defs();
    let token = &defs["TokenName"];
    assert_eq!(token["maxLength"], json!(48), "{token}");
    let name = "a".repeat(48);
    assert!(name.parse::<TokenName>().is_ok());
    assert!(format!("{name}b").parse::<TokenName>().is_err());

    // Les noms réservés, refusés par le parseur, sont annoncés dans la description.
    let description = token["description"].as_str().expect("description");
    for reserved in ["white", "black", "transparent", "current"] {
        assert!(reserved.parse::<TokenName>().is_err());
        assert!(description.contains(reserved), "{description}");
    }
    for hue in Hue::ALL {
        assert!(format!("{hue}-500").parse::<TokenName>().is_err());
        assert!(description.contains(hue.as_str()), "`{hue}` missing: {description}");
    }
    for shade in Shade::ALL {
        assert!(description.contains(shade.as_str()), "`{shade}` missing: {description}");
    }
}

#[test]
fn color_ref_schema_documents_the_token_name_bound() {
    let defs = command_defs();
    let color = &defs["ColorRef"];
    let pattern = color["pattern"].as_str().expect("ColorRef pattern").to_owned();

    // Borne réelle du parseur sur le nom (avant `/`), avec ou sans opacité.
    let bound = (1..=64)
        .take_while(|&n| "a".repeat(n).parse::<ColorRef>().is_ok())
        .last()
        .expect("short names parse");
    assert_eq!(bound, 48);
    let long = "a".repeat(bound + 1);
    assert!(long.parse::<ColorRef>().is_err());
    assert!(format!("{long}/80").parse::<ColorRef>().is_err());
    assert!(format!("{}/80", "a".repeat(bound)).parse::<ColorRef>().is_ok());
    let command = json!([{ "op": "set_style", "node": "n_0123456789", "style": { "text_color": { "base": long } } }]);
    let error = serde_json::from_value::<Vec<Command>>(command).unwrap_err();
    assert!(error.to_string().contains("ColorRef"), "{error}");

    // Le motif ne peut pas borner le nom : la borne est annoncée dans la description.
    let description = color["description"]
        .as_str()
        .unwrap_or_else(|| panic!("no description: {color}"));
    assert!(description.contains(&format!("{bound} caractères")), "{description}");

    // C'est le seul écart entre le motif et le parseur.
    let values = samples(&pattern, 2000);
    let mut longest = 0;
    for value in &values {
        let name = value.split('/').next().unwrap();
        longest = longest.max(name.len());
        assert_eq!(
            value.parse::<ColorRef>().is_ok(),
            name.len() <= bound,
            "`{value}` matches the ColorRef schema"
        );
    }
    assert!(longest > bound, "samples exceed the bound: {longest}");
}
