//! Valeurs de style abstraites (échelles et mots-clés), sérialisées en chaînes compactes.
//!
//! Aucune unité CSS ici : les échelles reprennent celles de Tailwind (la cible web), mais restent
//! des crans abstraits que le compilateur natif traduira à son tour.

use std::fmt;
use std::str::FromStr;

use ts_rs::TS;

use crate::error::{ValueError, enum_schema};
use crate::macros::{string_enum, string_serde};

string_enum! {
    /// Cran d'espacement (multiple de l'unité d'espacement des tokens, 4 px par défaut).
    pub enum Space {
        S0 = "0", Px = "px", S0p5 = "0.5", S1 = "1", S1p5 = "1.5", S2 = "2", S2p5 = "2.5",
        S3 = "3", S3p5 = "3.5", S4 = "4", S5 = "5", S6 = "6", S7 = "7", S8 = "8", S9 = "9",
        S10 = "10", S11 = "11", S12 = "12", S14 = "14", S16 = "16", S20 = "20", S24 = "24",
        S28 = "28", S32 = "32", S36 = "36", S40 = "40", S44 = "44", S48 = "48", S52 = "52",
        S56 = "56", S60 = "60", S64 = "64", S72 = "72", S80 = "80", S96 = "96",
    }
}

impl Space {
    /// Nombre de crans (`"px"` vaut 1 px quelle que soit l'unité, voir [`Space::to_px`]).
    pub fn steps(self) -> f64 {
        match self {
            Space::Px => 0.0,
            other => other.as_str().parse().unwrap_or(0.0),
        }
    }

    /// Taille en px pour une unité d'espacement donnée.
    pub fn to_px(self, unit: u8) -> f64 {
        match self {
            Space::Px => 1.0,
            other => other.steps() * f64::from(unit),
        }
    }
}

/// Marge : un cran d'espacement ou `auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(type = "string")]
pub enum Margin {
    Space(Space),
    Auto,
}

impl Margin {
    pub fn all() -> Vec<Margin> {
        Space::ALL
            .iter()
            .map(|s| Margin::Space(*s))
            .chain([Margin::Auto])
            .collect()
    }
}

impl fmt::Display for Margin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Margin::Space(space) => space.fmt(f),
            Margin::Auto => f.write_str("auto"),
        }
    }
}

impl FromStr for Margin {
    type Err = ValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Margin::Auto),
            _ => s.parse().map(Margin::Space).map_err(|_| ValueError::new("Margin", s)),
        }
    }
}

string_serde!(Margin, |_| enum_schema(Margin::all().iter().map(ToString::to_string)));

/// Décalage de positionnement : cran, `auto` ou `full` (100 %).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(type = "string")]
pub enum Inset {
    Space(Space),
    Auto,
    Full,
}

impl Inset {
    pub fn all() -> Vec<Inset> {
        Space::ALL
            .iter()
            .map(|s| Inset::Space(*s))
            .chain([Inset::Auto, Inset::Full])
            .collect()
    }
}

impl fmt::Display for Inset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Inset::Space(space) => space.fmt(f),
            Inset::Auto => f.write_str("auto"),
            Inset::Full => f.write_str("full"),
        }
    }
}

impl FromStr for Inset {
    type Err = ValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Inset::Auto),
            "full" => Ok(Inset::Full),
            _ => s.parse().map(Inset::Space).map_err(|_| ValueError::new("Inset", s)),
        }
    }
}

string_serde!(Inset, |_| enum_schema(Inset::all().iter().map(ToString::to_string)));

string_enum! {
    /// Fraction de la taille du parent.
    pub enum Fraction {
        Half = "1/2", Third = "1/3", TwoThirds = "2/3", Quarter = "1/4", ThreeQuarters = "3/4",
    }
}

string_enum! {
    /// Largeurs de conteneur nommées (échelle `--container-*` de Tailwind), plus `prose` (65ch).
    pub enum ContainerSize {
        Xs3 = "3xs", Xs2 = "2xs", Xs = "xs", Sm = "sm", Md = "md", Lg = "lg", Xl = "xl",
        Xl2 = "2xl", Xl3 = "3xl", Xl4 = "4xl", Xl5 = "5xl", Xl6 = "6xl", Xl7 = "7xl", Prose = "prose",
    }
}

impl ContainerSize {
    /// Largeur en px (rem × 16 ; `prose` estimé à 65 caractères de 8,5 px).
    pub fn to_px(self) -> f64 {
        let rem = match self {
            ContainerSize::Xs3 => 16.0,
            ContainerSize::Xs2 => 18.0,
            ContainerSize::Xs => 20.0,
            ContainerSize::Sm => 24.0,
            ContainerSize::Md => 28.0,
            ContainerSize::Lg => 32.0,
            ContainerSize::Xl => 36.0,
            ContainerSize::Xl2 => 42.0,
            ContainerSize::Xl3 => 48.0,
            ContainerSize::Xl4 => 56.0,
            ContainerSize::Xl5 => 64.0,
            ContainerSize::Xl6 => 72.0,
            ContainerSize::Xl7 => 80.0,
            ContainerSize::Prose => return 65.0 * 8.5,
        };
        rem * 16.0
    }
}

/// Valeur maximale acceptée pour `Size::Px`.
pub const MAX_PX: u16 = 4000;

/// Dimension (largeur, hauteur et leurs bornes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(type = "string")]
pub enum Size {
    Auto,
    Full,
    Screen,
    Fit,
    Min,
    Max,
    /// Aucune borne : réservé à `max_width` / `max_height` (valeur neutre de ces propriétés).
    None,
    Fraction(Fraction),
    Space(Space),
    Container(ContainerSize),
    /// Échappatoire en pixels (signalée en `Info` par la validation).
    Px(u16),
}

impl Size {
    const KEYWORDS: [(Size, &'static str); 7] = [
        (Size::Auto, "auto"),
        (Size::Full, "full"),
        (Size::Screen, "screen"),
        (Size::Fit, "fit"),
        (Size::Min, "min"),
        (Size::Max, "max"),
        (Size::None, "none"),
    ];

    /// Largeur fixe en px si la valeur ne dépend pas du parent ni du contenu.
    pub fn fixed_px(self, unit: u8) -> Option<f64> {
        match self {
            Size::Space(space) => Some(space.to_px(unit)),
            Size::Container(container) => Some(container.to_px()),
            Size::Px(px) => Some(f64::from(px)),
            _ => None,
        }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Size::Fraction(fraction) => fraction.fmt(f),
            Size::Space(space) => space.fmt(f),
            Size::Container(ContainerSize::Prose) => f.write_str("prose"),
            Size::Container(container) => write!(f, "container.{container}"),
            Size::Px(px) => write!(f, "{px}px"),
            keyword => {
                let text = Size::KEYWORDS.iter().find(|(k, _)| k == keyword).map_or("", |(_, t)| t);
                f.write_str(text)
            }
        }
    }
}

impl FromStr for Size {
    type Err = ValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((size, _)) = Size::KEYWORDS.iter().find(|(_, text)| *text == s) {
            return Ok(*size);
        }
        if s == "prose" {
            return Ok(Size::Container(ContainerSize::Prose));
        }
        if let Some(name) = s.strip_prefix("container.") {
            return match name.parse() {
                Ok(ContainerSize::Prose) | Err(_) => Err(ValueError::new("Size", s)),
                Ok(container) => Ok(Size::Container(container)),
            };
        }
        if let Some(digits) = s.strip_suffix("px")
            && !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && !digits.starts_with('0')
        {
            return match digits.parse::<u16>() {
                Ok(px) if px <= MAX_PX => Ok(Size::Px(px)),
                _ => Err(ValueError::new("Size", s)),
            };
        }
        if let Ok(fraction) = s.parse() {
            return Ok(Size::Fraction(fraction));
        }
        s.parse().map(Size::Space).map_err(|_| ValueError::new("Size", s))
    }
}

string_serde!(Size, |_| {
    let mut values: Vec<String> = Size::KEYWORDS.iter().map(|(_, t)| (*t).to_owned()).collect();
    values.extend(Fraction::ALL.iter().map(ToString::to_string));
    values.extend(Space::ALL.iter().map(ToString::to_string));
    values.extend(ContainerSize::ALL.iter().map(|c| Size::Container(*c).to_string()));
    schemars::json_schema!({
        "anyOf": [
            { "type": "string", "enum": values },
            { "type": "string", "pattern": "^[1-9][0-9]{0,3}px$" }
        ]
    })
});

string_enum! {
    pub enum Direction { Row = "row", Column = "column", RowReverse = "row-reverse", ColumnReverse = "column-reverse" }
}

string_enum! {
    /// Alignement des enfants sur l'axe secondaire.
    pub enum Align { Start = "start", Center = "center", End = "end", Stretch = "stretch", Baseline = "baseline" }
}

string_enum! {
    /// Alignement propre d'un enfant ; `auto` = suit l'alignement du parent (valeur neutre).
    pub enum AlignSelf {
        Auto = "auto", Start = "start", Center = "center", End = "end", Stretch = "stretch", Baseline = "baseline",
    }
}

string_enum! {
    pub enum Justify {
        Start = "start", Center = "center", End = "end", Between = "between", Around = "around", Evenly = "evenly",
    }
}

string_enum! {
    /// Nombre de colonnes d'une grille.
    pub enum GridColumns {
        C1 = "1", C2 = "2", C3 = "3", C4 = "4", C5 = "5", C6 = "6",
        C7 = "7", C8 = "8", C9 = "9", C10 = "10", C11 = "11", C12 = "12",
    }
}

impl GridColumns {
    pub fn count(self) -> u8 {
        self.as_str().parse().unwrap_or(1)
    }
}

string_enum! {
    /// Nombre de colonnes occupées par un enfant de grille.
    pub enum GridSpan {
        S1 = "1", S2 = "2", S3 = "3", S4 = "4", S5 = "5", S6 = "6",
        S7 = "7", S8 = "8", S9 = "9", S10 = "10", S11 = "11", S12 = "12", Full = "full",
    }
}

string_enum! {
    pub enum Order { First = "first", Last = "last", Default = "default" }
}

string_enum! {
    /// Ratios : 1, 16/9, 3/4, 4/3, 21/9.
    pub enum AspectRatio {
        Auto = "auto", Square = "square", Video = "video", Portrait = "portrait", Landscape = "landscape", Wide = "wide",
    }
}

string_enum! {
    pub enum Position { Static = "static", Relative = "relative", Absolute = "absolute", Fixed = "fixed", Sticky = "sticky" }
}

string_enum! {
    pub enum ZIndex { Auto = "auto", Z0 = "0", Z10 = "10", Z20 = "20", Z30 = "30", Z40 = "40", Z50 = "50" }
}

string_enum! {
    pub enum FontSize {
        Xs = "xs", Sm = "sm", Base = "base", Lg = "lg", Xl = "xl", Xl2 = "2xl", Xl3 = "3xl",
        Xl4 = "4xl", Xl5 = "5xl", Xl6 = "6xl", Xl7 = "7xl", Xl8 = "8xl", Xl9 = "9xl",
    }
}

impl FontSize {
    /// Taille en px (échelle Tailwind, 1 rem = 16 px).
    pub fn to_px(self) -> f64 {
        match self {
            FontSize::Xs => 12.0,
            FontSize::Sm => 14.0,
            FontSize::Base => 16.0,
            FontSize::Lg => 18.0,
            FontSize::Xl => 20.0,
            FontSize::Xl2 => 24.0,
            FontSize::Xl3 => 30.0,
            FontSize::Xl4 => 36.0,
            FontSize::Xl5 => 48.0,
            FontSize::Xl6 => 60.0,
            FontSize::Xl7 => 72.0,
            FontSize::Xl8 => 96.0,
            FontSize::Xl9 => 128.0,
        }
    }
}

string_enum! {
    pub enum FontWeight {
        Thin = "thin", Extralight = "extralight", Light = "light", Normal = "normal", Medium = "medium",
        Semibold = "semibold", Bold = "bold", Extrabold = "extrabold", Black = "black",
    }
}

impl FontWeight {
    pub fn numeric(self) -> u16 {
        100 * (FontWeight::ALL.iter().position(|w| *w == self).unwrap_or(3) as u16 + 1)
    }
}

string_enum! {
    pub enum LineHeight { None = "none", Tight = "tight", Snug = "snug", Normal = "normal", Relaxed = "relaxed", Loose = "loose" }
}

string_enum! {
    pub enum LetterSpacing {
        Tighter = "tighter", Tight = "tight", Normal = "normal", Wide = "wide", Wider = "wider", Widest = "widest",
    }
}

string_enum! {
    pub enum TextAlign { Start = "start", Center = "center", End = "end", Justify = "justify" }
}

string_enum! {
    pub enum TextTransform { None = "none", Uppercase = "uppercase", Lowercase = "lowercase", Capitalize = "capitalize" }
}

string_enum! {
    pub enum TextDecoration { None = "none", Underline = "underline", LineThrough = "line-through" }
}

string_enum! {
    pub enum TextWrap { Wrap = "wrap", NoWrap = "nowrap", Balance = "balance", Pretty = "pretty" }
}

string_enum! {
    pub enum GradientDir {
        ToT = "to-t", ToTr = "to-tr", ToR = "to-r", ToBr = "to-br", ToB = "to-b", ToBl = "to-bl", ToL = "to-l", ToTl = "to-tl",
    }
}

string_enum! {
    pub enum BorderWidth { W0 = "0", W1 = "1", W2 = "2", W4 = "4", W8 = "8" }
}

string_enum! {
    pub enum BorderStyle { Solid = "solid", Dashed = "dashed", Dotted = "dotted", None = "none" }
}

string_enum! {
    pub enum Radius { None = "none", Sm = "sm", Md = "md", Lg = "lg", Xl = "xl", Xl2 = "2xl", Xl3 = "3xl", Full = "full" }
}

string_enum! {
    pub enum Shadow { None = "none", Xs = "xs", Sm = "sm", Md = "md", Lg = "lg", Xl = "xl", Xl2 = "2xl" }
}

string_enum! {
    /// Opacité en pourcentage, par pas de 5.
    pub enum Opacity {
        O0 = "0", O5 = "5", O10 = "10", O15 = "15", O20 = "20", O25 = "25", O30 = "30", O35 = "35",
        O40 = "40", O45 = "45", O50 = "50", O55 = "55", O60 = "60", O65 = "65", O70 = "70", O75 = "75",
        O80 = "80", O85 = "85", O90 = "90", O95 = "95", O100 = "100",
    }
}

impl Opacity {
    pub fn fraction(self) -> f64 {
        self.as_str().parse::<f64>().unwrap_or(100.0) / 100.0
    }
}

string_enum! {
    /// Canal alpha d'une référence de couleur (`primary/80`), par pas de 5.
    pub enum Alpha {
        A5 = "5", A10 = "10", A15 = "15", A20 = "20", A25 = "25", A30 = "30", A35 = "35", A40 = "40",
        A45 = "45", A50 = "50", A55 = "55", A60 = "60", A65 = "65", A70 = "70", A75 = "75", A80 = "80",
        A85 = "85", A90 = "90", A95 = "95",
    }
}

impl Alpha {
    pub fn fraction(self) -> f64 {
        self.as_str().parse::<f64>().unwrap_or(100.0) / 100.0
    }
}

string_enum! {
    pub enum Overflow { Visible = "visible", Hidden = "hidden", Clip = "clip", Auto = "auto", Scroll = "scroll" }
}

string_enum! {
    pub enum ObjectFit { Cover = "cover", Contain = "contain", Fill = "fill", None = "none", ScaleDown = "scale-down" }
}

string_enum! {
    pub enum Transition {
        None = "none", Colors = "colors", Opacity = "opacity", Shadow = "shadow", Transform = "transform", All = "all",
    }
}

string_enum! {
    /// Durée de transition en millisecondes.
    pub enum Duration {
        D75 = "75", D100 = "100", D150 = "150", D200 = "200", D300 = "300", D500 = "500", D700 = "700", D1000 = "1000",
    }
}

string_enum! {
    /// Mise à l'échelle (états d'interaction seulement).
    pub enum Scale { S95 = "95", S100 = "100", S105 = "105", S110 = "110" }
}

string_enum! {
    pub enum RingWidth { W0 = "0", W1 = "1", W2 = "2", W4 = "4" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_through_their_text_form() {
        for size in ["auto", "none", "1/2", "16", "px", "container.7xl", "prose", "372px"] {
            assert_eq!(size.parse::<Size>().unwrap().to_string(), size);
        }
        for bad in ["0px", "4001px", "container.prose", "17", "12.5"] {
            assert!(bad.parse::<Size>().is_err(), "{bad}");
        }
        assert_eq!("auto".parse::<Margin>().unwrap(), Margin::Auto);
        assert_eq!("full".parse::<Inset>().unwrap(), Inset::Full);
        assert_eq!(serde_json::to_string(&Space::S0p5).unwrap(), "\"0.5\"");
        assert_eq!(serde_json::from_str::<FontSize>("\"2xl\"").unwrap(), FontSize::Xl2);
    }

    #[test]
    fn scales_convert_to_pixels() {
        assert_eq!(Space::S4.to_px(4), 16.0);
        assert_eq!(Space::Px.to_px(4), 1.0);
        assert_eq!(Size::Px(500).fixed_px(4), Some(500.0));
        assert_eq!(FontWeight::Bold.numeric(), 700);
        assert_eq!(GridColumns::C12.count(), 12);
    }
}
