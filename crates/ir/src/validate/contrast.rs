//! Contraste WCAG AA du texte, calculé à chaque breakpoint et dans chaque mode de couleur.
//!
//! Limites : le fond est cherché dans les ancêtres du même arbre (une instance ne connaît pas le
//! fond de la page qui l'accueille), les images de fond et l'opacité des ancêtres sont ignorées,
//! et les états d'interaction ne sont pas vérifiés.

use crate::document::Document;
use crate::id::{NodeId, TokenName};
use crate::node::{Node, NodeKind};
use crate::style::color::{ColorRef, ColorSource, Rgba, contrast_ratio};
use crate::style::responsive::Breakpoint;
use crate::style::style::Background;
use crate::style::values::{FontSize, FontWeight};
use crate::tokens::ColorMode;
use crate::validate::{Context, Issue, IssueCode as C};

/// Couleurs de texte, fonds et taille « grand texte » déjà évalués à un breakpoint précédent.
type Signature = (Vec<[u8; 3]>, Vec<[u8; 3]>, bool);

pub(super) fn check(ctx: &Context<'_>, issues: &mut Vec<Issue>) {
    let doc = ctx.doc;
    let mut modes = vec![ColorMode::Light];
    if doc.tokens.has_dark_mode() {
        modes.push(ColorMode::Dark);
    }
    for node in doc.nodes.values() {
        if ctx.owner(&node.id).is_none() || node.a11y.hidden {
            continue;
        }
        let runs: Vec<Option<&ColorRef>> = match &node.kind {
            NodeKind::Text(text) if !text.plain_text().trim().is_empty() => text
                .content
                .iter()
                .filter(|r| !r.text.trim().is_empty())
                .map(|r| r.color.as_ref())
                .collect(),
            NodeKind::Button(b) if b.label.is_some() => vec![None],
            NodeKind::Link(l) if l.label.is_some() => vec![None],
            NodeKind::Input(_) => vec![None],
            _ => continue,
        };
        for mode in &modes {
            let mut previous: Option<Signature> = None;
            for bp in Breakpoint::ALL {
                if !visible(doc, &node.id, bp) {
                    continue;
                }
                let backgrounds = background_candidates(doc, &node.id, bp, *mode);
                let large = is_large_text(doc, node, bp);
                let mut foregrounds = Vec::new();
                for run in &runs {
                    let color = match run {
                        Some(color) if color.source != ColorSource::Current => Some((*color).clone()),
                        _ => text_color(doc, &node.id, bp),
                    };
                    let resolved = match color {
                        Some(color) => doc.tokens.resolve_color(&color, *mode),
                        None => default_foreground(doc, *mode),
                    };
                    if let Some(rgba) = resolved {
                        foregrounds.push(rgba);
                    }
                }
                let signature = (
                    foregrounds.iter().map(key).collect::<Vec<_>>(),
                    backgrounds.iter().map(key).collect::<Vec<_>>(),
                    large,
                );
                if previous.as_ref() == Some(&signature) {
                    continue;
                }
                previous = Some(signature);
                let threshold = if large { 3.0 } else { 4.5 };
                let worst = foregrounds
                    .iter()
                    .filter(|fg| fg.a > 0.0)
                    .flat_map(|fg| backgrounds.iter().map(move |bg| contrast_ratio(fg.over(*bg), *bg)))
                    .fold(f64::INFINITY, f64::min);
                if worst.is_finite() && worst < threshold {
                    let mode_name = match mode {
                        ColorMode::Light => "light",
                        ColorMode::Dark => "dark",
                    };
                    issues.push(
                        Issue::error(
                            C::A11yContrast,
                            Some(&node.id),
                            format!(
                                "text contrast {worst:.2}:1 is below {threshold}:1 ({mode_name} mode, from {})",
                                bp.as_str()
                            ),
                        )
                        .at(bp),
                    );
                    break;
                }
            }
        }
    }
}

fn key(color: &Rgba) -> [u8; 3] {
    [
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8,
    ]
}

fn token(name: &str) -> Option<TokenName> {
    name.parse().ok()
}

fn default_foreground(doc: &Document, mode: ColorMode) -> Option<Rgba> {
    token("foreground")
        .and_then(|t| doc.tokens.resolve_color(&ColorRef::token(t), mode))
        .or(Some(Rgba::BLACK))
}

fn page_background(doc: &Document, mode: ColorMode) -> Rgba {
    token("background")
        .and_then(|t| doc.tokens.resolve_color(&ColorRef::token(t), mode))
        .map(|c| c.over(Rgba::WHITE))
        .unwrap_or(Rgba::WHITE)
}

fn chain(doc: &Document, id: &NodeId) -> Vec<NodeId> {
    std::iter::once(id.clone()).chain(doc.ancestors(id)).collect()
}

fn visible(doc: &Document, id: &NodeId, bp: Breakpoint) -> bool {
    chain(doc, id).iter().all(|n| {
        doc.node(n)
            .and_then(|n| n.visibility.as_ref())
            .is_none_or(|v| *v.resolve(bp))
    })
}

/// Couleur de texte effective (cascade des ancêtres, `current` = hérité).
fn text_color(doc: &Document, id: &NodeId, bp: Breakpoint) -> Option<ColorRef> {
    chain(doc, id).iter().find_map(|n| {
        let color = doc.node(n)?.style.text_color.as_ref()?.resolve(bp).clone();
        (color.source != ColorSource::Current).then_some(color)
    })
}

/// Fonds possibles derrière le nœud (plusieurs si un dégradé est traversé), opaques.
fn background_candidates(doc: &Document, id: &NodeId, bp: Breakpoint, mode: ColorMode) -> Vec<Rgba> {
    let mut layers: Vec<Vec<Rgba>> = Vec::new();
    for n in chain(doc, id) {
        let Some(background) = doc.node(&n).and_then(|n| n.style.background.as_ref()) else {
            continue;
        };
        let colors: Vec<Rgba> = match background.resolve(bp) {
            Background::Color { color } => doc.tokens.resolve_color(color, mode).into_iter().collect(),
            Background::Gradient { from, via, to, .. } => [Some(from), via.as_ref(), Some(to)]
                .into_iter()
                .flatten()
                .filter_map(|c| doc.tokens.resolve_color(c, mode))
                .collect(),
        };
        if colors.is_empty() || colors.iter().all(|c| c.a == 0.0) {
            continue;
        }
        let opaque = colors.iter().all(|c| c.a >= 1.0);
        layers.push(colors);
        if opaque {
            break;
        }
    }
    let mut candidates = vec![page_background(doc, mode)];
    for layer in layers.iter().rev() {
        candidates = candidates
            .iter()
            .flat_map(|bg| layer.iter().map(move |c| c.over(*bg)))
            .collect();
    }
    candidates
}

fn is_large_text(doc: &Document, node: &Node, bp: Breakpoint) -> bool {
    let ids = chain(doc, &node.id);
    let size = ids
        .iter()
        .find_map(|n| doc.node(n)?.style.font_size.as_ref().map(|v| *v.resolve(bp)))
        .unwrap_or(FontSize::Base)
        .to_px();
    let weight = ids
        .iter()
        .find_map(|n| doc.node(n)?.style.font_weight.as_ref().map(|v| *v.resolve(bp)))
        .unwrap_or(FontWeight::Normal);
    size >= 24.0 || (size >= 18.66 && weight.numeric() >= 700)
}
