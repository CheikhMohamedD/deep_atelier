//! Contraste WCAG AA du texte, calculé à chaque breakpoint et dans chaque mode de couleur.
//!
//! Un texte rendu par une page est jugé à chacun de ses rendus : fond, couleur et taille du texte
//! viennent de ses ancêtres rendus (l'arbre du composant qui accueille un contenu de slot, la page
//! qui accueille une instance ; une instance porte le style de la racine de son composant). Un
//! texte jamais rendu est jugé dans l'arbre où il est écrit, sur le fond de la page.
//!
//! Limites : les images de fond et l'opacité des ancêtres sont ignorées, et les états
//! d'interaction ne sont pas vérifiés.

use std::collections::BTreeSet;

use crate::document::{BindableField, Document, Page};
use crate::id::{NodeId, TokenName};
use crate::node::{Node, NodeKind, PropValue};
use crate::render::default_bound;
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
    // Un problème par nœud et par mode : le premier rendu qui échoue.
    let mut reported: BTreeSet<(NodeId, bool)> = BTreeSet::new();
    for (page, render) in &ctx.renders {
        for (index, entry) in render.nodes.iter().enumerate() {
            let Some(node) = doc.node(&entry.id) else { continue };
            if node.a11y.hidden {
                continue;
            }
            let bound = |field: BindableField| render.bound(doc, index, field).map(|b| b.value);
            let Some(runs) = text_runs(node, &bound) else { continue };
            // Les slots ne produisent pas d'élément ; une instance porte le style de la racine
            // de son composant, qui la précède dans la chaîne.
            let chain: Vec<&Node> = render
                .chain(index)
                .filter_map(|i| doc.node(&render.nodes[i].id))
                .filter(|n| !matches!(n.kind, NodeKind::Slot(_)))
                .collect();
            evaluate(doc, node, &chain, &runs, &modes, Some(page), &mut reported, issues);
        }
    }
    for node in doc.nodes.values() {
        if ctx.owner(&node.id).is_none() || ctx.is_rendered(&node.id) || node.a11y.hidden {
            continue;
        }
        let bound = |field: BindableField| default_bound(doc, &node.id, field);
        let Some(runs) = text_runs(node, &bound) else { continue };
        let chain: Vec<&Node> = std::iter::once(node)
            .chain(doc.ancestors(&node.id).iter().filter_map(|a| doc.node(a)))
            .collect();
        evaluate(doc, node, &chain, &runs, &modes, None, &mut reported, issues);
    }
}

/// Couleurs propres des segments de texte visibles d'un nœud (`None` : couleur héritée) ;
/// `None` si le nœud n'affiche pas de texte. Un texte lié à une prop est rendu d'un seul tenant.
fn text_runs<'d>(
    node: &'d Node,
    bound: &dyn Fn(BindableField) -> Option<&'d PropValue>,
) -> Option<Vec<Option<&'d ColorRef>>> {
    let bound_text = |field| match bound(field) {
        Some(PropValue::Text(text)) => Some(!text.trim().is_empty()),
        _ => None,
    };
    let runs: Vec<Option<&ColorRef>> = match &node.kind {
        NodeKind::Text(text) => match bound_text(BindableField::Text) {
            Some(true) => vec![None],
            Some(false) => return None,
            None => text
                .content
                .iter()
                .filter(|r| !r.text.trim().is_empty())
                .map(|r| r.color.as_ref())
                .collect(),
        },
        NodeKind::Button(b) => match bound_text(BindableField::Label) {
            Some(shown) => shown.then(|| vec![None])?,
            None => b.label.as_ref().map(|_| vec![None])?,
        },
        NodeKind::Link(l) => match bound_text(BindableField::Label) {
            Some(shown) => shown.then(|| vec![None])?,
            None => l.label.as_ref().map(|_| vec![None])?,
        },
        NodeKind::Input(_) => vec![None],
        _ => return None,
    };
    (!runs.is_empty()).then_some(runs)
}

/// Contraste d'un texte dans une chaîne d'éléments (le nœud, puis ses ancêtres).
#[allow(clippy::too_many_arguments)]
fn evaluate(
    doc: &Document,
    node: &Node,
    chain: &[&Node],
    runs: &[Option<&ColorRef>],
    modes: &[ColorMode],
    page: Option<&Page>,
    reported: &mut BTreeSet<(NodeId, bool)>,
    issues: &mut Vec<Issue>,
) {
    for mode in modes {
        let dark = *mode == ColorMode::Dark;
        if reported.contains(&(node.id.clone(), dark)) {
            continue;
        }
        let mut previous: Option<Signature> = None;
        for bp in Breakpoint::ALL {
            let visible = chain
                .iter()
                .all(|n| n.visibility.as_ref().is_none_or(|v| *v.resolve(bp)));
            if !visible {
                continue;
            }
            let backgrounds = background_candidates(doc, chain, bp, *mode);
            let large = is_large_text(chain, bp);
            let mut foregrounds = Vec::new();
            for run in runs {
                let color = match run {
                    Some(color) if color.source != ColorSource::Current => Some((*color).clone()),
                    _ => text_color(chain, bp),
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
                let mode_name = if dark { "dark" } else { "light" };
                let place = page.map(|p| format!(" on page `{}`", p.name)).unwrap_or_default();
                issues.push(
                    Issue::error(
                        C::A11yContrast,
                        Some(&node.id),
                        format!(
                            "text contrast {worst:.2}:1 is below {threshold}:1 ({mode_name} mode, from {}){place}",
                            bp.as_str()
                        ),
                    )
                    .at(bp),
                );
                reported.insert((node.id.clone(), dark));
                break;
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

/// Couleur de texte effective (cascade des ancêtres, `current` = hérité).
fn text_color(chain: &[&Node], bp: Breakpoint) -> Option<ColorRef> {
    chain.iter().find_map(|n| {
        let color = n.style.text_color.as_ref()?.resolve(bp).clone();
        (color.source != ColorSource::Current).then_some(color)
    })
}

/// Fonds possibles derrière le nœud (plusieurs si un dégradé est traversé), opaques.
fn background_candidates(doc: &Document, chain: &[&Node], bp: Breakpoint, mode: ColorMode) -> Vec<Rgba> {
    let mut layers: Vec<Vec<Rgba>> = Vec::new();
    for n in chain {
        let Some(background) = n.style.background.as_ref() else {
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

fn is_large_text(chain: &[&Node], bp: Breakpoint) -> bool {
    let size = chain
        .iter()
        .find_map(|n| n.style.font_size.as_ref().map(|v| *v.resolve(bp)))
        .unwrap_or(FontSize::Base)
        .to_px();
    let weight = chain
        .iter()
        .find_map(|n| n.style.font_weight.as_ref().map(|v| *v.resolve(bp)))
        .unwrap_or(FontWeight::Normal);
    size >= 24.0 || (size >= 18.66 && weight.numeric() >= 700)
}
