//! Avertissements responsive (à `base`, le mobile) et informations de qualité.
//!
//! Les cibles tactiles ne sont vérifiées que sur les dimensions explicites : la taille réelle,
//! qui dépend du contenu, est mesurée dans le navigateur.

use crate::command::Command;
use crate::document::Document;
use crate::id::{NodeId, NodeRef};
use crate::node::{Node, NodeKind, TextRole};
use crate::query::node_colors;
use crate::style::color::ColorSource;
use crate::style::responsive::{Breakpoint, ResponsivePatch};
use crate::style::style::{ResponsiveValue, StylePatch};
use crate::style::values::{Direction, FontSize, GridColumns, Size, Space};
use crate::validate::{Context, Issue, IssueCode as C};

/// Largeur de référence du mobile.
const MOBILE_WIDTH: f64 = 390.0;
/// Taille minimale d'une cible tactile.
const TOUCH_TARGET: f64 = 44.0;

pub(super) fn check(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    for node in ctx.doc.nodes.values() {
        if ctx.owner(&node.id).is_none() {
            continue;
        }
        responsive(ctx.doc, node, issues);
        quality(node, issues);
    }
}

fn set_style(id: &NodeId, style: StylePatch) -> Command {
    Command::SetStyle {
        node: NodeRef::from(id),
        state: None,
        style,
    }
}

/// Patch qui pose `base` et reporte la valeur actuelle en `md` si `md` n'est pas déjà défini.
fn base_then_md<T: Clone>(base: T, current: &crate::style::Responsive<T>) -> ResponsivePatch<T> {
    let mut patch = ResponsivePatch::at(Breakpoint::Base, base);
    if current.get(Breakpoint::Md).is_none() && current.get(Breakpoint::Sm).is_none() {
        patch.md = Some(Some(current.base.clone()));
    }
    patch
}

fn responsive(doc: &Document, node: &Node, issues: &mut Vec<Issue>) {
    let id = &node.id;
    let unit = doc.tokens.spacing_unit;
    let base = Breakpoint::Base;

    // Largeur fixe > 390 px à base.
    if let Some(width) = &node.style.width
        && let Some(px) = width.base.fixed_px(unit)
        && px > MOBILE_WIDTH
    {
        let fix = StylePatch {
            width: Some(Some(ResponsivePatch::at(base, Size::Full))),
            max_width: Some(Some(ResponsivePatch::at(base, width.base))),
            ..StylePatch::default()
        };
        issues.push(
            Issue::warning(
                C::RespFixedWidthOverflow,
                Some(id),
                format!("fixed width of {px}px overflows a 390px screen"),
            )
            .at(base)
            .with_fix(vec![set_style(id, fix)]),
        );
    }
    if let Some(min_width) = &node.style.min_width
        && let Some(px) = min_width.base.fixed_px(unit)
        && px > MOBILE_WIDTH
    {
        let fix = StylePatch {
            min_width: Some(None),
            ..StylePatch::default()
        };
        issues.push(
            Issue::warning(
                C::RespFixedWidthOverflow,
                Some(id),
                format!("minimum width of {px}px overflows a 390px screen"),
            )
            .at(base)
            .with_fix(vec![set_style(id, fix)]),
        );
    }

    match &node.kind {
        NodeKind::Stack(_) => {
            let row = node
                .style
                .direction
                .as_ref()
                .is_some_and(|d| matches!(d.base, Direction::Row | Direction::RowReverse));
            let wraps = node.style.wrap.as_ref().is_some_and(|w| w.base);
            if row && !wraps && node.children.len() > 3 {
                let fix = StylePatch {
                    wrap: Some(Some(ResponsivePatch::at(base, true))),
                    ..StylePatch::default()
                };
                issues.push(
                    Issue::warning(
                        C::RespRowTooManyChildren,
                        Some(id),
                        format!("row with {} children does not wrap on mobile", node.children.len()),
                    )
                    .at(base)
                    .with_fix(vec![set_style(id, fix)]),
                );
            }
        }
        NodeKind::Grid(_) => {
            if let Some(columns) = &node.style.columns
                && columns.base.count() > 2
            {
                let fix = StylePatch {
                    columns: Some(Some(base_then_md(GridColumns::C1, columns))),
                    ..StylePatch::default()
                };
                issues.push(
                    Issue::warning(
                        C::RespTooManyColumns,
                        Some(id),
                        format!("{} columns on mobile", columns.base.count()),
                    )
                    .at(base)
                    .with_fix(vec![set_style(id, fix)]),
                );
            }
        }
        NodeKind::Text(text) if matches!(text.role, TextRole::Heading { .. }) => {
            if let Some(size) = &node.style.font_size
                && size.base >= FontSize::Xl5
            {
                let fix = StylePatch {
                    font_size: Some(Some(base_then_md(FontSize::Xl4, size))),
                    ..StylePatch::default()
                };
                issues.push(
                    Issue::warning(
                        C::RespHeadingTooLarge,
                        Some(id),
                        format!("heading size `{}` is too large on mobile", size.base),
                    )
                    .at(base)
                    .with_fix(vec![set_style(id, fix)]),
                );
            }
        }
        _ => {}
    }

    if matches!(node.kind, NodeKind::Button(_) | NodeKind::Link(_) | NodeKind::Input(_)) {
        let small = [&node.style.height, &node.style.width]
            .iter()
            .filter_map(|v| v.as_ref().and_then(|v| v.base.fixed_px(unit)))
            .any(|px| px < TOUCH_TARGET);
        if small {
            let minimum = if f64::from(unit) * 11.0 == TOUCH_TARGET {
                Size::Space(Space::S11)
            } else {
                Size::Px(44)
            };
            let fix = StylePatch {
                height: Some(None),
                min_height: Some(Some(ResponsivePatch::at(base, minimum))),
                ..StylePatch::default()
            };
            issues.push(
                Issue::warning(
                    C::RespTouchTarget,
                    Some(id),
                    "touch target is smaller than 44×44px on mobile",
                )
                .at(base)
                .with_fix(vec![set_style(id, fix)]),
            );
        }
    }

    if matches!(node.kind, NodeKind::Input(_)) {
        let size = std::iter::once(id.clone())
            .chain(doc.ancestors(id))
            .find_map(|n| doc.node(&n)?.style.font_size.as_ref().map(|v| v.base))
            .unwrap_or(FontSize::Base);
        if size.to_px() < 16.0 {
            let fix = StylePatch {
                font_size: Some(Some(ResponsivePatch::at(base, FontSize::Base))),
                ..StylePatch::default()
            };
            issues.push(
                Issue::warning(
                    C::RespInputFontSize,
                    Some(id),
                    "input text under 16px makes iOS zoom in",
                )
                .at(base)
                .with_fix(vec![set_style(id, fix)]),
            );
        }
    }
}

fn quality(node: &Node, issues: &mut Vec<Issue>) {
    let id = &node.id;
    let off_token = node_colors(node).iter().any(|c| {
        matches!(
            c.source,
            ColorSource::Palette { .. } | ColorSource::White | ColorSource::Black
        )
    });
    if off_token {
        issues.push(Issue::info(
            C::QualityOffTokenColor,
            Some(id),
            "color outside the design tokens",
        ));
    }
    let uses_px = node.style.entries().iter().any(|(_, value)| match value {
        ResponsiveValue::Size(size) => size.values().any(|s| matches!(s, Size::Px(_))),
        _ => false,
    });
    if uses_px {
        issues.push(Issue::info(
            C::QualityPxValue,
            Some(id),
            "pixel value instead of a scale step",
        ));
    }
    if node.kind.container().is_some() && node.children.is_empty() && node.parent.is_some() {
        issues.push(Issue::info(C::QualityEmptyContainer, Some(id), "empty container"));
    }
}
