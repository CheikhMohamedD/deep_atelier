//! Validation : schéma, accessibilité, contraste, responsive (avec corrections), qualité.

mod common;

use common::*;
use ir::*;
use serde_json::json;

fn codes(issues: &[Issue]) -> Vec<IssueCode> {
    issues.iter().map(|i| i.code).collect()
}

fn insert(session: &mut Session, nodes: serde_json::Value) -> Applied {
    let root = home_root(session);
    run_json(
        session,
        json!([{ "op": "insert_nodes", "parent": root.to_string(), "nodes": nodes }]),
    )
}

#[test]
fn a_new_document_has_no_errors() {
    let session = session();
    let issues = session.validate();
    assert!(issues.iter().all(|i| !i.is_error()), "{issues:#?}");
    assert!(codes(&issues).contains(&IssueCode::A11yMissingH1));
}

#[test]
fn accessibility_rules() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$img", "kind": { "type": "Image" } },
            { "ref": "$deco", "kind": { "type": "Image" }, "meta": { "a11y_hidden": true } },
            { "ref": "$h1", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": [{ "text": "A" }] } },
            { "ref": "$h1b", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": [{ "text": "B" }] } },
            { "ref": "$h4", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h4" }, "content": [{ "text": "C" }] } },
            { "ref": "$icon-button", "kind": { "type": "Button" } },
            { "parent": "$icon-button", "kind": { "type": "Icon", "name": "x" } },
            { "ref": "$input", "kind": { "type": "Input", "input_type": "email" } },
            { "ref": "$labelled", "kind": { "type": "Input", "input_type": "text" } },
            { "kind": { "type": "Text", "for_input": "$labelled", "content": [{ "text": "Nom" }] } },
            { "kind": { "type": "Box", "role": { "kind": "Main" } } },
            { "kind": { "type": "Box", "role": { "kind": "Main" } } }
        ]),
    );
    let issues = session.validate();
    let has = |code: IssueCode, reference: &str| {
        issues
            .iter()
            .any(|i| i.code == code && i.node.as_ref() == Some(&applied.created[reference]))
    };
    assert!(has(IssueCode::A11yImgAlt, "$img"));
    assert!(!has(IssueCode::A11yImgAlt, "$deco"));
    assert!(has(IssueCode::A11yMultipleH1, "$h1b"));
    assert!(has(IssueCode::A11yHeadingSkip, "$h4"));
    assert!(has(IssueCode::A11yAccessibleName, "$icon-button"));
    assert!(has(IssueCode::A11yAccessibleName, "$input"));
    assert!(!has(IssueCode::A11yAccessibleName, "$labelled"));
    assert!(codes(&issues).contains(&IssueCode::A11yMultipleMain));

    run_json(&mut session, json!([{ "op": "update_settings", "lang": "français" }]));
    assert!(codes(&session.validate()).contains(&IssueCode::A11yLang));
}

#[test]
fn contrast_is_checked_per_breakpoint_and_mode() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$ok", "kind": { "type": "Text", "content": [{ "text": "Lisible" }] } },
            { "ref": "$card", "kind": { "type": "Box" },
              "style": { "background": { "base": { "kind": "Color", "color": "white" }, "lg": { "kind": "Color", "color": "slate-300" } } } },
            { "ref": "$low", "parent": "$card", "kind": { "type": "Text", "content": [{ "text": "Pâle" }] },
              "style": { "text_color": { "base": "slate-500" } } },
            { "ref": "$big", "kind": { "type": "Text", "content": [{ "text": "Grand" }] },
              "style": { "text_color": { "base": "gray-500" }, "font_size": { "base": "3xl" } } }
        ]),
    );
    let issues = session.validate();
    let contrast: Vec<&Issue> = issues.iter().filter(|i| i.code == IssueCode::A11yContrast).collect();
    let low = contrast
        .iter()
        .find(|i| i.node.as_ref() == Some(&applied.created["$low"]))
        .expect("low contrast at lg");
    assert_eq!(low.breakpoint, Some(Breakpoint::Lg));
    assert!(
        contrast
            .iter()
            .all(|i| i.node.as_ref() != Some(&applied.created["$ok"]))
    );
    // Grand texte : seuil 3:1, gray-500 sur blanc passe.
    assert!(
        contrast
            .iter()
            .all(|i| i.node.as_ref() != Some(&applied.created["$big"]) || i.message.contains("dark"))
    );
}

#[test]
fn responsive_warnings_come_with_working_fixes() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$wide", "kind": { "type": "Box" }, "style": { "width": { "base": "500px" } } },
            { "ref": "$row", "kind": { "type": "Stack" }, "style": { "direction": { "base": "row" } } },
            { "parent": "$row", "kind": { "type": "Box" } }, { "parent": "$row", "kind": { "type": "Box" } },
            { "parent": "$row", "kind": { "type": "Box" } }, { "parent": "$row", "kind": { "type": "Box" } },
            { "ref": "$grid", "kind": { "type": "Grid" }, "style": { "columns": { "base": "3" } } },
            { "parent": "$grid", "kind": { "type": "Box" } },
            { "ref": "$title", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" }, "content": [{ "text": "Énorme" }] },
              "style": { "font_size": { "base": "6xl" } } },
            { "ref": "$tiny", "kind": { "type": "Button", "label": "OK" }, "style": { "height": { "base": "8" } } },
            { "ref": "$field", "kind": { "type": "Input" }, "style": { "font_size": { "base": "sm" } }, "meta": { "a11y_label": "Champ" } }
        ]),
    );
    let issues = session.validate();
    let expected = [
        (IssueCode::RespFixedWidthOverflow, "$wide"),
        (IssueCode::RespRowTooManyChildren, "$row"),
        (IssueCode::RespTooManyColumns, "$grid"),
        (IssueCode::RespHeadingTooLarge, "$title"),
        (IssueCode::RespTouchTarget, "$tiny"),
        (IssueCode::RespInputFontSize, "$field"),
    ];
    let mut fixes = Vec::new();
    for (code, reference) in expected {
        let issue = issues
            .iter()
            .find(|i| i.code == code && i.node.as_ref() == Some(&applied.created[reference]))
            .unwrap_or_else(|| panic!("{code:?} on {reference}"));
        assert_eq!(issue.severity, Severity::Warning);
        fixes.extend(issue.fix.clone().expect("a fix is proposed"));
    }
    run(&mut session, fixes);
    let after = session.validate();
    for (code, _) in expected {
        assert!(!codes(&after).contains(&code), "{code:?} remains after its fix");
    }
    let doc = session.document();
    let grid = doc.nodes[&applied.created["$grid"]].style.columns.as_ref().unwrap();
    assert_eq!(
        (grid.base, grid.md),
        (GridColumns::C1, Some(GridColumns::C3)),
        "desktop layout preserved"
    );
    let wide = &doc.nodes[&applied.created["$wide"]].style;
    assert_eq!(wide.width.as_ref().unwrap().base, Size::Full);
    assert_eq!(wide.max_width.as_ref().unwrap().base, Size::Px(500));
}

#[test]
fn schema_rules_catch_misplaced_nodes_and_styles() {
    let mut session = session();
    let applied = insert(
        &mut session,
        json!([
            { "ref": "$item", "kind": { "type": "Box", "role": { "kind": "ListItem" } } },
            { "ref": "$slot", "kind": { "type": "Slot" } },
            { "ref": "$link", "kind": { "type": "Link", "href": { "kind": "Anchor", "anchor": "nowhere" } } },
            { "ref": "$nested", "parent": "$link", "kind": { "type": "Button", "label": "Clic" } },
            { "ref": "$icon", "kind": { "type": "Icon", "name": "not-an-icon" } },
            { "ref": "$photo", "kind": { "type": "Image", "alt": "Photo", "source": { "kind": "Url", "url": "https://example.com/a.jpg" } } },
            { "ref": "$boxed", "kind": { "type": "Box" }, "style": { "direction": { "base": "row" }, "max_height": { "base": "none" } } },
            { "ref": "$sized", "kind": { "type": "Box" }, "style": { "width": { "base": "none" } } }
        ]),
    );
    let issues = session.validate();
    let has = |code: IssueCode, reference: &str| {
        issues
            .iter()
            .any(|i| i.code == code && i.node.as_ref() == Some(&applied.created[reference]) && i.is_error())
    };
    assert!(has(IssueCode::ListItemOutsideList, "$item"));
    assert!(has(IssueCode::SlotOutsideComponent, "$slot"));
    assert!(has(IssueCode::InvalidReference, "$link"));
    assert!(has(IssueCode::NestedInteractive, "$nested"));
    assert!(has(IssueCode::UnknownIcon, "$icon"));
    assert!(has(IssueCode::MissingIntrinsic, "$photo"));
    assert!(has(IssueCode::StyleNotAllowed, "$boxed"));
    assert!(has(IssueCode::StyleNotAllowed, "$sized"));
    assert_eq!(
        issues
            .iter()
            .filter(|i| i.code == IssueCode::StyleNotAllowed && i.node.as_ref() == Some(&applied.created["$boxed"]))
            .count(),
        1,
        "max_height: none is valid"
    );
}

#[test]
fn tree_corruption_is_detected() {
    let mut doc = session().into_document();
    let root = doc.pages[0].root.clone();
    let orphan: NodeId = "n_zzzzzzzzzz".parse().unwrap();
    doc.nodes.insert(
        orphan.clone(),
        Node::new(
            orphan.clone(),
            None,
            NodeKind::Box(ContainerProps {
                role: ContainerRole::Generic,
            }),
        ),
    );
    doc.nodes
        .get_mut(&root)
        .unwrap()
        .children
        .push("n_yyyyyyyyyy".parse().unwrap());
    let issues = validate(&doc);
    assert!(
        issues
            .iter()
            .any(|i| i.code == IssueCode::OrphanNode && i.node.as_ref() == Some(&orphan))
    );
    assert!(
        issues
            .iter()
            .any(|i| i.code == IssueCode::TreeInconsistent && i.node.as_ref() == Some(&root))
    );
}

#[test]
fn recursive_components_are_rejected() {
    let mut session = session();
    let applied = insert(&mut session, json!([{ "ref": "$card", "kind": { "type": "Box" } }]));
    let card = applied.created["$card"].clone();
    let applied = run_json(
        &mut session,
        json!([{ "op": "create_component", "node": card.to_string(), "name": "Card", "props": [] }]),
    );
    let component = applied.components[0].clone();
    run_json(
        &mut session,
        json!([{ "op": "insert_nodes", "parent": card.to_string(), "nodes": [
            { "kind": { "type": "ComponentInstance", "component": component.to_string() } }
        ]}]),
    );
    assert!(codes(&session.validate()).contains(&IssueCode::RecursiveComponent));
}

#[test]
fn quality_infos() {
    let mut session = session();
    insert(
        &mut session,
        json!([{ "kind": { "type": "Box" }, "style": { "background": { "base": { "kind": "Color", "color": "red-500" } }, "width": { "base": "120px" } } }]),
    );
    let issues = session.validate();
    for code in [
        IssueCode::QualityOffTokenColor,
        IssueCode::QualityPxValue,
        IssueCode::QualityEmptyContainer,
    ] {
        assert!(
            issues.iter().any(|i| i.code == code && i.severity == Severity::Info),
            "{code:?}"
        );
    }
}

#[test]
fn issues_serialize_with_stable_codes() {
    let issue = Issue::warning(IssueCode::RespTouchTarget, None, "small").at(Breakpoint::Base);
    let value = serde_json::to_value(&issue).unwrap();
    assert_eq!(value["code"], "RESP_TOUCH_TARGET");
    assert_eq!(value["severity"], "Warning");
    assert_eq!(value["breakpoint"], "base");
}
