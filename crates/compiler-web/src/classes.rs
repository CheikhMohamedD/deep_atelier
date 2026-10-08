//! Style de l'IR → classes Tailwind v4, préfixes mobile-first (`md:`), états (`hover:`).
//!
//! Ordre des classes : les classes de base, puis celles de chaque breakpoint (`sm:` à `2xl:`),
//! puis celles de chaque état d'interaction ; dans chaque bloc, par famille de propriétés
//! (affichage, disposition, placement, position, dimensions, marges, typographie, apparence,
//! transitions). Une valeur de base égale à la valeur initiale CSS d'une propriété non héritée
//! n'est pas émise ; une propriété héritée (typographie) l'est toujours, car un composant ou un
//! contenu de slot n'hérite pas, au rendu, des ancêtres où il est écrit.

use std::collections::BTreeMap;

use ir::command::neutral::initial_value;
use ir::style::style::{Background, InteractionState, PropChange, ResponsiveValue, ResponsiveValuePatch, StyleProp};
use ir::style::values::*;
use ir::{Breakpoint, ColorRef, Document, Node, Responsive, StatePatch, StylePatch};

/// Préfixe d'un breakpoint (`md:`), vide pour `base`.
fn bp_prefix(bp: Breakpoint) -> &'static str {
    match bp {
        Breakpoint::Base => "",
        Breakpoint::Sm => "sm:",
        Breakpoint::Md => "md:",
        Breakpoint::Lg => "lg:",
        Breakpoint::Xl => "xl:",
        Breakpoint::Xxl => "2xl:",
    }
}

fn state_prefix(state: Option<InteractionState>) -> &'static str {
    match state {
        None => "",
        Some(InteractionState::Hover) => "hover:",
        Some(InteractionState::FocusVisible) => "focus-visible:",
        Some(InteractionState::Active) => "active:",
    }
}

/// Famille d'une propriété (ordre des classes).
fn family(prop: StyleProp) -> u8 {
    use StyleProp as P;
    match prop {
        P::Direction | P::Wrap | P::Columns | P::Gap | P::Align | P::Justify => 1,
        P::Grow | P::Shrink | P::AlignSelf | P::ColSpan | P::Order => 2,
        P::Position | P::Top | P::Right | P::Bottom | P::Left | P::ZIndex => 3,
        P::Width | P::MinWidth | P::MaxWidth | P::Height | P::MinHeight | P::MaxHeight | P::AspectRatio => 4,
        P::MarginTop | P::MarginRight | P::MarginBottom | P::MarginLeft => 5,
        P::PaddingTop | P::PaddingRight | P::PaddingBottom | P::PaddingLeft => 6,
        P::FontFamily
        | P::FontSize
        | P::FontWeight
        | P::LineHeight
        | P::LetterSpacing
        | P::TextAlign
        | P::TextColor
        | P::TextTransform
        | P::TextDecoration
        | P::TextWrap => 7,
        P::Background
        | P::BorderTopWidth
        | P::BorderRightWidth
        | P::BorderBottomWidth
        | P::BorderLeftWidth
        | P::BorderColor
        | P::BorderStyle
        | P::Radius
        | P::Shadow
        | P::Opacity
        | P::Overflow
        | P::ObjectFit
        | P::Scale
        | P::Ring => 8,
        P::Transition | P::Duration => 9,
    }
}

/// Classe d'affichage à rendre visible un élément (`flex`, `grid`, `block`, `inline`…).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Block,
    Flex,
    Grid,
    Inline,
    InlineBlock,
}

impl Display {
    fn class(self) -> &'static str {
        match self {
            Display::Block => "block",
            Display::Flex => "flex",
            Display::Grid => "grid",
            Display::Inline => "inline",
            Display::InlineBlock => "inline-block",
        }
    }

    /// Classe émise à `base` sans masquage (seuls flex et grid diffèrent du défaut).
    fn base_class(self) -> Option<&'static str> {
        match self {
            Display::Flex => Some("flex"),
            Display::Grid => Some("grid"),
            _ => None,
        }
    }
}

/// Valeurs déclarées d'une propriété : breakpoint → valeur.
type Declared = BTreeMap<Breakpoint, ResponsiveValue>;

/// Une classe à placer, avec sa clé de tri.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Placed {
    state: u8,
    bp: Breakpoint,
    family: u8,
    prop: u8,
    rank: u8,
    class: String,
}

/// Classes d'un nœud : style, visibilité, états, échappatoires web. Une instance ne produit
/// pas d'élément : elle n'émet pas la classe d'affichage de la racine de son composant, seulement
/// ses changements de visibilité.
pub fn node_classes(doc: &Document, node: &Node, display: Display) -> Vec<String> {
    let mut placed = Vec::new();
    let own_element = !matches!(node.kind, ir::NodeKind::ComponentInstance(_));
    visibility_classes(node, display, own_element, &mut placed);
    let declared = declared_style(node);
    style_classes(doc, &declared, None, &mut placed, |prop, value| {
        omitted_at_base(doc, prop, value)
    });
    for (rank, state) in InteractionState::ALL.into_iter().enumerate() {
        let state_style = node.states.get(state);
        let mut map: BTreeMap<StyleProp, Declared> = BTreeMap::new();
        for (prop, value) in state_style.entries() {
            map.insert(prop, per_breakpoint(&value));
        }
        let base = |prop: StyleProp| node.style.get(prop).ok().flatten();
        style_classes(doc, &map, Some((rank as u8 + 1, state)), &mut placed, |prop, value| {
            // Un état égal au style hors état à `base` ne change rien.
            base(prop).is_some_and(|b| b.at(Breakpoint::Base) == *value)
        });
    }
    placed.sort();
    let mut classes: Vec<String> = Vec::new();
    for p in placed {
        if !classes.contains(&p.class) {
            classes.push(p.class);
        }
    }
    if let Some(web) = &node.platform_overrides.web {
        for extra in &web.extra_classes {
            if !classes.contains(extra) {
                classes.push(extra.clone());
            }
        }
    }
    classes
}

/// Classes d'une surcharge de variante (valeurs posées seulement : une variante s'ajoute au
/// style de base, résolue par `twMerge`).
pub fn patch_classes(doc: &Document, style: &StylePatch, states: &[(InteractionState, &StatePatch)]) -> Vec<String> {
    let mut placed = Vec::new();
    let mut map: BTreeMap<StyleProp, Declared> = BTreeMap::new();
    for (prop, change) in style.entries() {
        if let PropChange::Merge(patch) = change {
            map.insert(prop, patch_values(&patch));
        }
    }
    style_classes(doc, &map, None, &mut placed, |_, _| false);
    for (state, patch) in states {
        let rank = InteractionState::ALL.iter().position(|s| s == state).unwrap_or(0) as u8 + 1;
        let mut map: BTreeMap<StyleProp, Declared> = BTreeMap::new();
        for (prop, change) in patch.entries() {
            if let PropChange::Merge(p) = change {
                map.insert(prop, patch_values(&p));
            }
        }
        style_classes(doc, &map, Some((rank, *state)), &mut placed, |_, _| false);
    }
    placed.sort();
    let mut classes: Vec<String> = Vec::new();
    for p in placed {
        if !classes.contains(&p.class) {
            classes.push(p.class);
        }
    }
    classes
}

fn declared_style(node: &Node) -> BTreeMap<StyleProp, Declared> {
    node.style
        .entries()
        .into_iter()
        .map(|(prop, value)| (prop, per_breakpoint(&value)))
        .collect()
}

/// Valeurs déclarées d'une valeur responsive, une par breakpoint.
fn per_breakpoint(value: &ResponsiveValue) -> Declared {
    value.declared().into_iter().map(|bp| (bp, value.at(bp))).collect()
}

/// Valeurs posées par un patch responsive (les effacements sont ignorés).
fn patch_values(patch: &ResponsiveValuePatch) -> Declared {
    macro_rules! values {
        ($($variant:ident),+) => {
            match patch {
                $(ResponsiveValuePatch::$variant(p) => {
                    let mut out = Declared::new();
                    for bp in Breakpoint::ALL {
                        let value = match bp {
                            Breakpoint::Base => p.base.as_ref(),
                            Breakpoint::Sm => p.sm.as_ref().and_then(Option::as_ref),
                            Breakpoint::Md => p.md.as_ref().and_then(Option::as_ref),
                            Breakpoint::Lg => p.lg.as_ref().and_then(Option::as_ref),
                            Breakpoint::Xl => p.xl.as_ref().and_then(Option::as_ref),
                            Breakpoint::Xxl => p.xxl.as_ref().and_then(Option::as_ref),
                        };
                        if let Some(value) = value {
                            out.insert(bp, ResponsiveValue::$variant(Responsive::new(value.clone())));
                        }
                    }
                    out
                })+
            }
        };
    }
    values!(
        Direction,
        Bool,
        GridColumns,
        Space,
        Align,
        AlignSelf,
        Justify,
        GridSpan,
        Order,
        Margin,
        Size,
        AspectRatio,
        Position,
        Inset,
        ZIndex,
        Token,
        FontSize,
        FontWeight,
        LineHeight,
        LetterSpacing,
        TextAlign,
        Color,
        TextTransform,
        TextDecoration,
        TextWrap,
        Background,
        BorderWidth,
        BorderStyle,
        Radius,
        Shadow,
        Opacity,
        Overflow,
        ObjectFit,
        Transition,
        Duration,
        Scale,
        Ring
    )
}

/// Vrai si une valeur de base est la valeur initiale CSS d'une propriété non héritée.
fn omitted_at_base(doc: &Document, prop: StyleProp, value: &ResponsiveValue) -> bool {
    if prop.is_inherited() || prop == StyleProp::Columns {
        return false;
    }
    initial_value(doc, prop).at(Breakpoint::Base) == *value
}

/// Classes d'affichage : masquage par breakpoint, et classe d'affichage là où l'élément
/// redevient visible.
fn visibility_classes(node: &Node, display: Display, own_element: bool, placed: &mut Vec<Placed>) {
    let push = |placed: &mut Vec<Placed>, bp: Breakpoint, class: &str| {
        placed.push(Placed {
            state: 0,
            bp,
            family: 0,
            prop: 0,
            rank: 0,
            class: format!("{}{class}", bp_prefix(bp)),
        });
    };
    let base_class = display.base_class().filter(|_| own_element);
    let Some(visibility) = &node.visibility else {
        if let Some(class) = base_class {
            push(placed, Breakpoint::Base, class);
        }
        return;
    };
    let mut previous = visibility.base;
    if previous {
        if let Some(class) = base_class {
            push(placed, Breakpoint::Base, class);
        }
    } else {
        push(placed, Breakpoint::Base, "hidden");
    }
    for bp in &Breakpoint::ALL[1..] {
        let Some(visible) = visibility.get(*bp) else { continue };
        if *visible != previous {
            push(placed, *bp, if *visible { display.class() } else { "hidden" });
            previous = *visible;
        }
    }
}

/// Côtés d'une propriété à quatre côtés : (préfixe commun, préfixes par côté dans l'ordre
/// haut, droite, bas, gauche).
fn side_group(prop: StyleProp) -> Option<(usize, &'static str, [&'static str; 4])> {
    use StyleProp as P;
    let (side, name) = match prop {
        P::PaddingTop => (0, "p"),
        P::PaddingRight => (1, "p"),
        P::PaddingBottom => (2, "p"),
        P::PaddingLeft => (3, "p"),
        P::MarginTop => (0, "m"),
        P::MarginRight => (1, "m"),
        P::MarginBottom => (2, "m"),
        P::MarginLeft => (3, "m"),
        P::BorderTopWidth => (0, "border"),
        P::BorderRightWidth => (1, "border"),
        P::BorderBottomWidth => (2, "border"),
        P::BorderLeftWidth => (3, "border"),
        P::Top => (0, "inset"),
        P::Right => (1, "inset"),
        P::Bottom => (2, "inset"),
        P::Left => (3, "inset"),
        _ => return None,
    };
    let sides = match name {
        "p" => ["pt", "pr", "pb", "pl"],
        "m" => ["mt", "mr", "mb", "ml"],
        "border" => ["border-t", "border-r", "border-b", "border-l"],
        _ => ["top", "right", "bottom", "left"],
    };
    Some((side, name, sides))
}

/// Suffixe d'une valeur d'un côté (`4`, `auto`, `2` pour une bordure), vide pour la bordure de
/// 1 px (`border-t`).
fn side_suffix(value: &ResponsiveValue) -> Option<String> {
    let text = match value {
        ResponsiveValue::Space(v) => v.base.as_str().to_owned(),
        ResponsiveValue::Margin(v) => v.base.to_string(),
        ResponsiveValue::Inset(v) => v.base.to_string(),
        ResponsiveValue::BorderWidth(v) => match v.base {
            BorderWidth::W1 => String::new(),
            other => other.as_str().to_owned(),
        },
        _ => return None,
    };
    Some(text)
}

fn join_class(prefix: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix}-{suffix}")
    }
}

/// Classes du style d'un nœud ou d'un état, regroupées par breakpoint.
fn style_classes(
    doc: &Document,
    declared: &BTreeMap<StyleProp, Declared>,
    state: Option<(u8, InteractionState)>,
    placed: &mut Vec<Placed>,
    omit_base: impl Fn(StyleProp, &ResponsiveValue) -> bool,
) {
    let (state_rank, prefix_state) = state.map_or((0, ""), |(rank, s)| (rank, state_prefix(Some(s))));
    // Propriétés à quatre côtés : regroupées par breakpoint.
    let mut sides: BTreeMap<(&'static str, Breakpoint), [Option<String>; 4]> = BTreeMap::new();
    let mut side_names: BTreeMap<&'static str, [&'static str; 4]> = BTreeMap::new();
    let mut side_family: BTreeMap<&'static str, (u8, u8)> = BTreeMap::new();
    for (prop, values) in declared {
        for (bp, value) in values {
            if *bp == Breakpoint::Base && omit_base(*prop, value) {
                continue;
            }
            let prefix = format!("{}{prefix_state}", bp_prefix(*bp));
            if let Some((side, name, names)) = side_group(*prop) {
                if let Some(suffix) = side_suffix(value) {
                    sides.entry((name, *bp)).or_default()[side] = Some(suffix);
                    side_names.insert(name, names);
                    side_family.insert(name, (family(*prop), *prop as u8));
                }
                continue;
            }
            for (rank, utility) in utilities(doc, *prop, value).into_iter().enumerate() {
                placed.push(Placed {
                    state: state_rank,
                    bp: *bp,
                    family: family(*prop),
                    prop: *prop as u8,
                    rank: rank as u8,
                    class: format!("{prefix}{utility}"),
                });
            }
        }
    }
    for ((name, bp), values) in sides {
        let names = side_names[name];
        let (fam, prop) = side_family[name];
        let prefix = format!("{}{prefix_state}", bp_prefix(bp));
        for (rank, utility) in collapse_sides(name, names, &values).into_iter().enumerate() {
            placed.push(Placed {
                state: state_rank,
                bp,
                family: fam,
                prop,
                rank: rank as u8,
                class: format!("{prefix}{utility}"),
            });
        }
    }
}

/// `p-4` si les quatre côtés sont égaux, `py-` / `px-` par paires, sinon côté par côté.
fn collapse_sides(name: &str, names: [&str; 4], values: &[Option<String>; 4]) -> Vec<String> {
    let [top, right, bottom, left] = values;
    let (x, y) = match name {
        "border" => ("border-x", "border-y"),
        "inset" => ("inset-x", "inset-y"),
        _ => (
            if name == "p" { "px" } else { "mx" },
            if name == "p" { "py" } else { "my" },
        ),
    };
    if top.is_some() && top == right && top == bottom && top == left {
        return vec![join_class(name, top.as_deref().unwrap_or_default())];
    }
    let mut out = Vec::new();
    if right.is_some() && right == left {
        out.push(join_class(x, right.as_deref().unwrap_or_default()));
    } else {
        if let Some(v) = right {
            out.push(join_class(names[1], v));
        }
        if let Some(v) = left {
            out.push(join_class(names[3], v));
        }
    }
    if top.is_some() && top == bottom {
        out.push(join_class(y, top.as_deref().unwrap_or_default()));
    } else {
        if let Some(v) = top {
            out.push(join_class(names[0], v));
        }
        if let Some(v) = bottom {
            out.push(join_class(names[2], v));
        }
    }
    out
}

fn size_suffix(size: Size) -> String {
    match size {
        Size::Container(ContainerSize::Prose) => "prose".to_owned(),
        Size::Container(container) => container.as_str().to_owned(),
        Size::Px(px) => format!("[{px}px]"),
        other => other.to_string(),
    }
}

fn color(prefix: &str, value: &ColorRef) -> String {
    format!("{prefix}-{value}")
}

/// Utilitaires d'une valeur de base (sans préfixe de variante).
fn utilities(_doc: &Document, prop: StyleProp, value: &ResponsiveValue) -> Vec<String> {
    use ResponsiveValue as V;
    use StyleProp as P;
    let one = |s: String| vec![s];
    match (prop, value) {
        (P::Direction, V::Direction(v)) => one(match v.base {
            Direction::Row => "flex-row",
            Direction::Column => "flex-col",
            Direction::RowReverse => "flex-row-reverse",
            Direction::ColumnReverse => "flex-col-reverse",
        }
        .to_owned()),
        (P::Wrap, V::Bool(v)) => one(if v.base { "flex-wrap" } else { "flex-nowrap" }.to_owned()),
        (P::Grow, V::Bool(v)) => one(if v.base { "grow" } else { "grow-0" }.to_owned()),
        (P::Shrink, V::Bool(v)) => one(if v.base { "shrink" } else { "shrink-0" }.to_owned()),
        (P::Columns, V::GridColumns(v)) => one(format!("grid-cols-{}", v.base)),
        (P::Gap, V::Space(v)) => one(format!("gap-{}", v.base)),
        (P::Align, V::Align(v)) => one(format!("items-{}", v.base)),
        (P::Justify, V::Justify(v)) => one(format!("justify-{}", v.base)),
        (P::AlignSelf, V::AlignSelf(v)) => one(format!("self-{}", v.base)),
        (P::ColSpan, V::GridSpan(v)) => one(format!("col-span-{}", v.base)),
        (P::Order, V::Order(v)) => one(match v.base {
            Order::First => "order-first",
            Order::Last => "order-last",
            Order::Default => "order-none",
        }
        .to_owned()),
        (P::Width, V::Size(v)) => one(format!("w-{}", size_suffix(v.base))),
        (P::MinWidth, V::Size(v)) => one(format!("min-w-{}", size_suffix(v.base))),
        (P::MaxWidth, V::Size(v)) => one(format!("max-w-{}", size_suffix(v.base))),
        (P::Height, V::Size(v)) => one(format!("h-{}", size_suffix(v.base))),
        (P::MinHeight, V::Size(v)) => one(format!("min-h-{}", size_suffix(v.base))),
        (P::MaxHeight, V::Size(v)) => one(format!("max-h-{}", size_suffix(v.base))),
        (P::AspectRatio, V::AspectRatio(v)) => one(match v.base {
            AspectRatio::Auto => "aspect-auto",
            AspectRatio::Square => "aspect-square",
            AspectRatio::Video => "aspect-video",
            AspectRatio::Portrait => "aspect-3/4",
            AspectRatio::Landscape => "aspect-4/3",
            AspectRatio::Wide => "aspect-21/9",
        }
        .to_owned()),
        (P::Position, V::Position(v)) => one(v.base.as_str().to_owned()),
        (P::ZIndex, V::ZIndex(v)) => one(format!("z-{}", v.base)),
        (P::FontFamily, V::Token(v)) => one(format!("font-{}", v.base)),
        (P::FontSize, V::FontSize(v)) => one(format!("text-{}", v.base)),
        (P::FontWeight, V::FontWeight(v)) => one(format!("font-{}", v.base)),
        (P::LineHeight, V::LineHeight(v)) => one(format!("leading-{}", v.base)),
        (P::LetterSpacing, V::LetterSpacing(v)) => one(format!("tracking-{}", v.base)),
        (P::TextAlign, V::TextAlign(v)) => one(format!("text-{}", v.base)),
        (P::TextColor, V::Color(v)) => one(color("text", &v.base)),
        (P::TextTransform, V::TextTransform(v)) => one(match v.base {
            TextTransform::None => "normal-case",
            other => other.as_str(),
        }
        .to_owned()),
        (P::TextDecoration, V::TextDecoration(v)) => one(match v.base {
            TextDecoration::None => "no-underline",
            other => other.as_str(),
        }
        .to_owned()),
        (P::TextWrap, V::TextWrap(v)) => one(format!("text-{}", v.base)),
        (P::Background, V::Background(v)) => match &v.base {
            Background::Color { color: c } => one(color("bg", c)),
            Background::Gradient {
                direction,
                from,
                via,
                to,
            } => {
                let mut out = vec![format!("bg-linear-{direction}"), color("from", from)];
                if let Some(via) = via {
                    out.push(color("via", via));
                }
                out.push(color("to", to));
                out
            }
        },
        (P::BorderColor, V::Color(v)) => one(color("border", &v.base)),
        (P::BorderStyle, V::BorderStyle(v)) => one(format!("border-{}", v.base)),
        (P::Radius, V::Radius(v)) => one(format!("rounded-{}", v.base)),
        (P::Shadow, V::Shadow(v)) => one(format!("shadow-{}", v.base)),
        (P::Opacity, V::Opacity(v)) => one(format!("opacity-{}", v.base)),
        (P::Overflow, V::Overflow(v)) => one(format!("overflow-{}", v.base)),
        (P::ObjectFit, V::ObjectFit(v)) => one(format!("object-{}", v.base)),
        (P::Transition, V::Transition(v)) => one(match v.base {
            Transition::All => "transition-all".to_owned(),
            other => format!("transition-{other}"),
        }),
        (P::Duration, V::Duration(v)) => one(format!("duration-{}", v.base)),
        (P::Scale, V::Scale(v)) => one(format!("scale-{}", v.base)),
        (P::Ring, V::Ring(v)) => {
            let ring = &v.base;
            let mut out = vec![match ring.width {
                RingWidth::W1 => "ring".to_owned(),
                other => format!("ring-{other}"),
            }];
            if ring.color.source != ir::ColorSource::Current {
                out.push(color("ring", &ring.color));
            }
            if ring.offset != RingWidth::W0 {
                out.push(format!("ring-offset-{}", ring.offset));
            }
            out
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sides(values: [Option<&str>; 4]) -> [Option<String>; 4] {
        values.map(|v| v.map(str::to_owned))
    }

    #[test]
    fn sides_collapse() {
        let p = ["pt", "pr", "pb", "pl"];
        assert_eq!(collapse_sides("p", p, &sides([Some("4"); 4])), vec!["p-4"]);
        assert_eq!(
            collapse_sides("p", p, &sides([Some("8"), Some("4"), Some("8"), Some("4")])),
            vec!["px-4", "py-8"]
        );
        assert_eq!(
            collapse_sides("p", p, &sides([Some("2"), None, None, None])),
            vec!["pt-2"]
        );
        let b = ["border-t", "border-r", "border-b", "border-l"];
        assert_eq!(
            collapse_sides("border", b, &sides([Some(""), None, None, None])),
            vec!["border-t"]
        );
        assert_eq!(collapse_sides("border", b, &sides([Some(""); 4])), vec!["border"]);
    }
}
