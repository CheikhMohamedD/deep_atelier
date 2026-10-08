//! Avertissements responsive (à `base`, le mobile) et informations de qualité.
//!
//! Les cibles tactiles ne sont vérifiées que sur les dimensions explicites : la taille réelle,
//! qui dépend du contenu, est mesurée dans le navigateur.

use crate::command::Command;
use crate::id::{NodeId, NodeRef};
use crate::node::{Node, NodeKind, TextRole};
use crate::query::node_colors;
use crate::style::color::ColorSource;
use crate::style::responsive::{Breakpoint, Responsive, ResponsivePatch};
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
        responsive(ctx, node, issues);
        quality(node, issues);
    }
}

/// Taille de texte héritée à `base`, à chaque rendu du nœud (ancêtres rendus, slots exclus), ou
/// dans l'arbre où il est écrit s'il n'est jamais rendu.
fn inherited_font_sizes(ctx: &Context<'_>, node: &Node) -> Vec<FontSize> {
    let doc = ctx.doc;
    let own = |n: &Node| n.style.font_size.as_ref().map(|v| v.base);
    if !ctx.is_rendered(&node.id) {
        let size = std::iter::once(node.id.clone())
            .chain(doc.ancestors(&node.id))
            .find_map(|n| doc.node(&n).and_then(own))
            .unwrap_or(FontSize::Base);
        return vec![size];
    }
    ctx.occurrences(&node.id)
        .into_iter()
        .map(|(_, render, index)| {
            render
                .chain(index)
                .filter_map(|i| doc.node(&render.nodes[i].id))
                .filter(|n| !matches!(n.kind, NodeKind::Slot(_)))
                .find_map(own)
                .unwrap_or(FontSize::Base)
        })
        .collect()
}

fn set_style(id: &NodeId, style: StylePatch) -> Command {
    Command::SetStyle {
        node: NodeRef::from(id),
        state: None,
        style,
    }
}

/// Patch qui pose `base` et reporte la valeur actuelle en `md` si `md` n'est pas déjà défini.
fn base_then_md<T: Clone>(base: T, current: &Responsive<T>) -> ResponsivePatch<T> {
    let mut patch = ResponsivePatch::at(Breakpoint::Base, base);
    if current.get(Breakpoint::Md).is_none() && current.get(Breakpoint::Sm).is_none() {
        patch.md = Some(Some(current.base.clone()));
    }
    patch
}

/// Vrai si un minimum garantit à lui seul une cible de 44 px : valeur fixe suffisante, ou taille
/// de l'écran. Un minimum qui dépend du contenu (`fit`, `min`, `max`) ou du parent (`full`,
/// fractions) ne la garantit pas.
fn sufficient_minimum(min: Size, unit: u8) -> bool {
    min == Size::Screen || min.fixed_px(unit).is_some_and(|px| px >= TOUCH_TARGET)
}

/// Correction d'un axe de cible tactile (dimension et minimum), `None` si l'axe n'est pas trop
/// petit à `base` : dimension fixe d'au moins 44 px, ou minimum suffisant (le minimum l'emporte
/// sur la dimension). La dimension suit le contenu (`auto`) et le minimum passe à 44 px. Comme
/// pour les autres corrections, les valeurs actuelles sont reportées en `md` : le rendu au-dessus
/// du mobile ne change pas (un minimum absent y est remis à `auto`, sa valeur neutre).
fn touch_axis(
    size: Option<&Responsive<Size>>,
    min: Option<&Responsive<Size>>,
    minimum: Size,
    unit: u8,
) -> Option<(ResponsivePatch<Size>, ResponsivePatch<Size>)> {
    let size = size.filter(|s| s.base.fixed_px(unit).is_some_and(|px| px < TOUCH_TARGET))?;
    if min.is_some_and(|m| sufficient_minimum(m.base, unit)) {
        return None;
    }
    let neutral = Responsive::new(Size::Auto);
    Some((
        base_then_md(Size::Auto, size),
        base_then_md(minimum, min.unwrap_or(&neutral)),
    ))
}

fn responsive(ctx: &Context<'_>, node: &Node, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
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
        // Seul le mobile perd son minimum : les autres breakpoints gardent leur valeur.
        let fix = StylePatch {
            min_width: Some(Some(base_then_md(Size::Auto, min_width))),
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
        let minimum = if f64::from(unit) * 11.0 == TOUCH_TARGET {
            Size::Space(Space::S11)
        } else {
            Size::Px(44)
        };
        // Chaque axe trop petit est corrigé séparément.
        let mut fix = StylePatch::default();
        if let Some((size, min)) = touch_axis(
            node.style.height.as_ref(),
            node.style.min_height.as_ref(),
            minimum,
            unit,
        ) {
            fix.height = Some(Some(size));
            fix.min_height = Some(Some(min));
        }
        if let Some((size, min)) = touch_axis(node.style.width.as_ref(), node.style.min_width.as_ref(), minimum, unit) {
            fix.width = Some(Some(size));
            fix.min_width = Some(Some(min));
        }
        if fix != StylePatch::default() {
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
        let small = inherited_font_sizes(ctx, node)
            .into_iter()
            .any(|size| size.to_px() < 16.0);
        if small {
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
