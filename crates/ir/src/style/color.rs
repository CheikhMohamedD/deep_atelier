//! Couleurs : références (`primary/80`, `slate-900`), hexadécimal, palette et calcul de contraste.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

use ts_rs::TS;

use crate::error::{ValueError, pattern_schema};
use crate::id::{TOKEN_NAME_MAX_LEN, TokenName};
use crate::macros::{string_enum, string_serde};
use crate::style::values::Alpha;

string_enum! {
    /// Teintes de la palette Tailwind (4.3).
    pub enum Hue {
        Slate = "slate", Gray = "gray", Zinc = "zinc", Neutral = "neutral", Stone = "stone",
        Mauve = "mauve", Olive = "olive", Mist = "mist", Taupe = "taupe",
        Red = "red", Orange = "orange", Amber = "amber", Yellow = "yellow", Lime = "lime",
        Green = "green", Emerald = "emerald", Teal = "teal", Cyan = "cyan", Sky = "sky",
        Blue = "blue", Indigo = "indigo", Violet = "violet", Purple = "purple",
        Fuchsia = "fuchsia", Pink = "pink", Rose = "rose",
    }
}

string_enum! {
    /// Nuances de la palette.
    pub enum Shade {
        S50 = "50", S100 = "100", S200 = "200", S300 = "300", S400 = "400", S500 = "500",
        S600 = "600", S700 = "700", S800 = "800", S900 = "900", S950 = "950",
    }
}

/// Analyse `<teinte>-<nuance>` (`slate-900`).
pub(crate) fn parse_palette(text: &str) -> Option<(Hue, Shade)> {
    let (hue, shade) = text.rsplit_once('-')?;
    Some((hue.parse().ok()?, shade.parse().ok()?))
}

/// Couleur hexadécimale `#rrggbb` ou `#rrggbbaa`, normalisée en minuscules.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(type = "string")]
pub struct Hex(String);

impl Hex {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Couleur sRGB avec alpha.
    pub fn rgba(&self) -> Rgba {
        let byte = |i: usize| u8::from_str_radix(&self.0[i..i + 2], 16).unwrap_or(0);
        let alpha = if self.0.len() == 9 {
            f64::from(byte(7)) / 255.0
        } else {
            1.0
        };
        Rgba {
            r: f64::from(byte(1)) / 255.0,
            g: f64::from(byte(3)) / 255.0,
            b: f64::from(byte(5)) / 255.0,
            a: alpha,
        }
    }
}

impl fmt::Display for Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Hex {
    type Err = ValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let valid = s.starts_with('#') && matches!(s.len(), 7 | 9) && s[1..].bytes().all(|b| b.is_ascii_hexdigit());
        if valid {
            Ok(Self(s.to_ascii_lowercase()))
        } else {
            Err(ValueError::new("Hex", s))
        }
    }
}

string_serde!(Hex, |_| pattern_schema("^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$"));

/// Source d'une couleur référencée par le style.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColorSource {
    /// Token de couleur du projet (recommandé).
    Token(TokenName),
    /// Couleur de la palette (signalée en `Info` : hors tokens).
    Palette {
        hue: Hue,
        shade: Shade,
    },
    White,
    Black,
    Transparent,
    /// Couleur de texte héritée (`currentColor`).
    Current,
}

/// Référence de couleur : `"primary"`, `"primary/80"`, `"slate-900"`, `"white"`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(type = "string")]
pub struct ColorRef {
    pub source: ColorSource,
    pub alpha: Option<Alpha>,
}

impl ColorRef {
    pub fn token(name: TokenName) -> Self {
        Self {
            source: ColorSource::Token(name),
            alpha: None,
        }
    }

    pub fn source(source: ColorSource) -> Self {
        Self { source, alpha: None }
    }

    /// Nom de token référencé, le cas échéant.
    pub fn token_name(&self) -> Option<&TokenName> {
        match &self.source {
            ColorSource::Token(name) => Some(name),
            _ => None,
        }
    }
}

impl fmt::Display for ColorRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            ColorSource::Token(name) => f.write_str(name.as_str())?,
            ColorSource::Palette { hue, shade } => write!(f, "{hue}-{shade}")?,
            ColorSource::White => f.write_str("white")?,
            ColorSource::Black => f.write_str("black")?,
            ColorSource::Transparent => f.write_str("transparent")?,
            ColorSource::Current => f.write_str("current")?,
        }
        if let Some(alpha) = self.alpha {
            write!(f, "/{alpha}")?;
        }
        Ok(())
    }
}

impl FromStr for ColorRef {
    type Err = ValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let error = || ValueError::new("ColorRef", s);
        let (name, alpha) = match s.split_once('/') {
            Some((name, alpha)) => (name, Some(alpha.parse::<Alpha>().map_err(|_| error())?)),
            None => (s, None),
        };
        let source = match name {
            "white" => ColorSource::White,
            "black" => ColorSource::Black,
            "transparent" => ColorSource::Transparent,
            "current" => ColorSource::Current,
            _ => match parse_palette(name) {
                Some((hue, shade)) => ColorSource::Palette { hue, shade },
                None => ColorSource::Token(name.parse().map_err(|_| error())?),
            },
        };
        Ok(Self { source, alpha })
    }
}

/// Schéma de `ColorRef`. Le motif ne peut pas borner la longueur du nom sans lookahead, et
/// `maxLength` (refusé par les schémas d'outils stricts) porterait sur la chaîne entière, opacité
/// comprise : la borne des noms de token est annoncée dans la description.
fn color_ref_schema() -> schemars::Schema {
    let description = format!(
        "Référence de couleur : token du projet (`primary`), couleur de palette `<teinte>-<nuance>` \
         (`slate-900`), `white`, `black`, `transparent` ou `current`, suivie d'une opacité facultative \
         `/5` à `/95` par pas de 5 (`primary/80`). Nom de token en kebab-case, {TOKEN_NAME_MAX_LEN} \
         caractères au plus."
    );
    schemars::json_schema!({
        "type": "string",
        "pattern": "^[a-z][a-z0-9]*(-[a-z0-9]+)*(/(5|[1-9][05]))?$",
        "description": description,
    })
}

string_serde!(ColorRef, |_| color_ref_schema());

/// Couleur sRGB normalisée (composantes 0..=1) avec alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub const WHITE: Rgba = Rgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Rgba = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Rgba = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub fn with_alpha(self, factor: f64) -> Rgba {
        Rgba {
            a: self.a * factor,
            ..self
        }
    }

    /// Composition « source over » de `self` sur un fond opaque.
    pub fn over(self, background: Rgba) -> Rgba {
        let a = self.a.clamp(0.0, 1.0);
        Rgba {
            r: self.r * a + background.r * (1.0 - a),
            g: self.g * a + background.g * (1.0 - a),
            b: self.b * a + background.b * (1.0 - a),
            a: 1.0,
        }
    }

    /// Luminance relative WCAG 2.x.
    pub fn luminance(self) -> f64 {
        let channel = |c: f64| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

/// Rapport de contraste WCAG entre deux couleurs opaques (1..=21).
pub fn contrast_ratio(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (light, dark) = if la > lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// Conversion OKLCH (L 0..=1, C, H en degrés) → sRGB, avec écrêtage dans la gamme.
pub fn oklch_to_srgb(l: f64, c: f64, h: f64) -> Rgba {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548 * b;
    let (l3, m3, s3) = (l_.powi(3), m_.powi(3), s_.powi(3));
    let r = 4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3;
    let g = -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3;
    let bl = -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701 * s3;
    let gamma = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    Rgba {
        r: gamma(r),
        g: gamma(g),
        b: gamma(bl),
        a: 1.0,
    }
}

/// Table de la palette, chargée depuis `data/tailwind-palette.txt`.
fn palette_table() -> &'static BTreeMap<(Hue, Shade), Rgba> {
    static TABLE: OnceLock<BTreeMap<(Hue, Shade), Rgba>> = OnceLock::new();
    TABLE.get_or_init(|| {
        include_str!("../../data/tailwind-palette.txt")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let hue = parts.next()?.parse().ok()?;
                let shade = parts.next()?.parse().ok()?;
                let l: f64 = parts.next()?.parse().ok()?;
                let c: f64 = parts.next()?.parse().ok()?;
                let h: f64 = parts.next()?.parse().ok()?;
                Some(((hue, shade), oklch_to_srgb(l / 100.0, c, h)))
            })
            .collect()
    })
}

/// Couleur sRGB d'une entrée de la palette.
pub fn palette_color(hue: Hue, shade: Shade) -> Rgba {
    palette_table().get(&(hue, shade)).copied().unwrap_or(Rgba::BLACK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_refs_round_trip() {
        for text in [
            "primary",
            "primary/80",
            "slate-900",
            "white",
            "current",
            "muted-foreground/5",
        ] {
            assert_eq!(text.parse::<ColorRef>().unwrap().to_string(), text);
        }
        assert!("primary/81".parse::<ColorRef>().is_err());
        assert!("Primary".parse::<ColorRef>().is_err());
        assert!(matches!(
            "red-500".parse::<ColorRef>().unwrap().source,
            ColorSource::Palette {
                hue: Hue::Red,
                shade: Shade::S500
            }
        ));
    }

    #[test]
    fn hex_is_normalized() {
        assert_eq!("#1D4ED8".parse::<Hex>().unwrap().as_str(), "#1d4ed8");
        assert!("#12345".parse::<Hex>().is_err());
        assert!((("#00000080".parse::<Hex>().unwrap().rgba().a) - 0.502).abs() < 0.01);
    }

    #[test]
    fn palette_covers_every_hue_and_shade() {
        assert_eq!(palette_table().len(), Hue::ALL.len() * Shade::ALL.len());
        // blue-600 Tailwind ≈ #155dfc
        let blue = palette_color(Hue::Blue, Shade::S600);
        assert!((blue.r * 255.0 - 21.0).abs() < 3.0, "{blue:?}");
        assert!((blue.b * 255.0 - 252.0).abs() < 3.0, "{blue:?}");
    }

    #[test]
    fn contrast_matches_wcag_reference_values() {
        assert!((contrast_ratio(Rgba::WHITE, Rgba::BLACK) - 21.0).abs() < 1e-9);
        let gray = "#777777".parse::<Hex>().unwrap().rgba();
        assert!((contrast_ratio(gray, Rgba::WHITE) - 4.48).abs() < 0.01);
    }
}
