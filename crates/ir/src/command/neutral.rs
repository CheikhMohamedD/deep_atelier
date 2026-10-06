//! Valeurs neutres : base posée quand une surcharge crée une propriété absente.
//!
//! Poser `lg` sur une propriété absente crée `{ base: <neutre>, lg: v }` : le rendu en `base` ne
//! change pas. Le neutre est la valeur initiale (CSS / Tailwind) pour les propriétés non héritées,
//! la valeur héritée des ancêtres pour la typographie, et la valeur hors état pour un état
//! d'interaction.

use crate::document::Document;
use crate::id::{NodeId, TokenName};
use crate::style::color::{ColorRef, ColorSource};
use crate::style::responsive::{Breakpoint, Responsive};
use crate::style::style::{Background, InteractionState, ResponsiveValue, Ring, StyleProp};
use crate::style::values::*;

/// Valeur initiale d'une propriété, indépendante du contexte.
pub fn initial_value(doc: &Document, prop: StyleProp) -> ResponsiveValue {
    use ResponsiveValue as V;
    use StyleProp as P;
    match prop {
        P::Direction => V::Direction(Responsive::new(Direction::Row)),
        P::Wrap | P::Grow => V::Bool(Responsive::new(false)),
        P::Shrink => V::Bool(Responsive::new(true)),
        P::Columns => V::GridColumns(Responsive::new(GridColumns::C1)),
        P::Gap | P::PaddingTop | P::PaddingRight | P::PaddingBottom | P::PaddingLeft => {
            V::Space(Responsive::new(Space::S0))
        }
        P::Align => V::Align(Responsive::new(Align::Stretch)),
        P::Justify => V::Justify(Responsive::new(Justify::Start)),
        P::AlignSelf => V::AlignSelf(Responsive::new(AlignSelf::Auto)),
        P::ColSpan => V::GridSpan(Responsive::new(GridSpan::S1)),
        P::Order => V::Order(Responsive::new(Order::Default)),
        P::MarginTop | P::MarginRight | P::MarginBottom | P::MarginLeft => {
            V::Margin(Responsive::new(Margin::Space(Space::S0)))
        }
        P::Width | P::MinWidth | P::Height | P::MinHeight => V::Size(Responsive::new(Size::Auto)),
        P::MaxWidth | P::MaxHeight => V::Size(Responsive::new(Size::None)),
        P::AspectRatio => V::AspectRatio(Responsive::new(AspectRatio::Auto)),
        P::Position => V::Position(Responsive::new(Position::Static)),
        P::Top | P::Right | P::Bottom | P::Left => V::Inset(Responsive::new(Inset::Auto)),
        P::ZIndex => V::ZIndex(Responsive::new(ZIndex::Auto)),
        P::FontFamily => V::Token(Responsive::new(default_font(doc))),
        P::FontSize => V::FontSize(Responsive::new(FontSize::Base)),
        P::FontWeight => V::FontWeight(Responsive::new(FontWeight::Normal)),
        P::LineHeight => V::LineHeight(Responsive::new(LineHeight::Normal)),
        P::LetterSpacing => V::LetterSpacing(Responsive::new(LetterSpacing::Normal)),
        P::TextAlign => V::TextAlign(Responsive::new(TextAlign::Start)),
        P::TextColor | P::BorderColor => V::Color(Responsive::new(ColorRef::source(ColorSource::Current))),
        P::TextTransform => V::TextTransform(Responsive::new(TextTransform::None)),
        P::TextDecoration => V::TextDecoration(Responsive::new(TextDecoration::None)),
        P::TextWrap => V::TextWrap(Responsive::new(TextWrap::Wrap)),
        P::Background => V::Background(Responsive::new(Background::Color {
            color: ColorRef::source(ColorSource::Transparent),
        })),
        P::BorderTopWidth | P::BorderRightWidth | P::BorderBottomWidth | P::BorderLeftWidth => {
            V::BorderWidth(Responsive::new(BorderWidth::W0))
        }
        P::BorderStyle => V::BorderStyle(Responsive::new(BorderStyle::Solid)),
        P::Radius => V::Radius(Responsive::new(Radius::None)),
        P::Shadow => V::Shadow(Responsive::new(Shadow::None)),
        P::Opacity => V::Opacity(Responsive::new(Opacity::O100)),
        P::Overflow => V::Overflow(Responsive::new(Overflow::Visible)),
        P::ObjectFit => V::ObjectFit(Responsive::new(ObjectFit::Fill)),
        P::Transition => V::Transition(Responsive::new(Transition::None)),
        P::Duration => V::Duration(Responsive::new(Duration::D150)),
        P::Scale => V::Scale(Responsive::new(Scale::S100)),
        P::Ring => V::Ring(Responsive::new(Ring {
            width: RingWidth::W0,
            color: ColorRef::source(ColorSource::Current),
            offset: RingWidth::W0,
        })),
    }
}

fn default_font(doc: &Document) -> TokenName {
    doc.tokens.fonts.first().map(|f| f.name.clone()).unwrap_or_else(|| {
        "sans"
            .parse()
            .unwrap_or_else(|_| unreachable!("`sans` is a valid token name"))
    })
}

/// Valeur neutre (réduite à `base`) d'une propriété pour un nœud.
pub fn neutral_value(
    doc: &Document,
    node: &NodeId,
    state: Option<InteractionState>,
    prop: StyleProp,
) -> ResponsiveValue {
    if state.is_some()
        && prop.in_base()
        && let Some(value) = doc.node(node).and_then(|n| n.style.get(prop).ok().flatten())
    {
        return value.at(Breakpoint::Base);
    }
    if prop.is_inherited() {
        let start = if state.is_some() {
            vec![node.clone()]
        } else {
            Vec::new()
        };
        for ancestor in start.into_iter().chain(doc.ancestors(node)) {
            if let Some(value) = doc.node(&ancestor).and_then(|n| n.style.get(prop).ok().flatten()) {
                return value.at(Breakpoint::Base);
            }
        }
    }
    initial_value(doc, prop)
}
