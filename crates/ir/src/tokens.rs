//! Design tokens : couleurs (clair/sombre), polices, unité d'espacement, rayons, ombres.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::TokenName;
use crate::style::color::{ColorRef, ColorSource, Hex, Rgba, palette_color};
use crate::style::values::{Radius, Shadow};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct DesignTokens {
    pub colors: Vec<ColorToken>,
    pub fonts: Vec<FontToken>,
    /// Px par cran d'espacement (4 par défaut), exporté en `--spacing`.
    pub spacing_unit: u8,
    pub radii: Vec<RadiusToken>,
    pub shadows: Vec<ShadowToken>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct ColorToken {
    pub name: TokenName,
    pub light: Hex,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dark: Option<Hex>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct FontToken {
    pub name: TokenName,
    pub family: FontFamily,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind")]
pub enum FontFamily {
    Google { family: String, weights: Vec<u16> },
    System { stack: SystemFont },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SystemFont {
    Sans,
    Serif,
    Mono,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct RadiusToken {
    pub step: Radius,
    pub px: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct ShadowToken {
    pub step: Shadow,
    pub layers: Vec<ShadowLayer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct ShadowLayer {
    pub x: i16,
    pub y: i16,
    pub blur: u16,
    pub spread: i16,
    pub color: Hex,
    #[serde(default)]
    pub inset: bool,
}

/// Mode de couleur pour la résolution des tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColorMode {
    Light,
    Dark,
}

fn token(name: &str) -> TokenName {
    name.parse()
        .unwrap_or_else(|_| panic!("default token name {name} is valid"))
}

fn hex(value: &str) -> Hex {
    value
        .parse()
        .unwrap_or_else(|_| panic!("default color {value} is valid"))
}

fn layer(x: i16, y: i16, blur: u16, spread: i16, color: &str) -> ShadowLayer {
    ShadowLayer {
        x,
        y,
        blur,
        spread,
        color: hex(color),
        inset: false,
    }
}

impl Default for DesignTokens {
    /// Préréglage par défaut : couleurs sémantiques shadcn (neutre), polices système, échelles
    /// Tailwind.
    fn default() -> Self {
        let colors = [
            ("background", "#ffffff", "#0a0a0a"),
            ("foreground", "#0a0a0a", "#fafafa"),
            ("primary", "#171717", "#e5e5e5"),
            ("primary-foreground", "#fafafa", "#171717"),
            ("secondary", "#f5f5f5", "#262626"),
            ("secondary-foreground", "#171717", "#fafafa"),
            ("muted", "#f5f5f5", "#262626"),
            ("muted-foreground", "#737373", "#a1a1a1"),
            ("accent", "#f5f5f5", "#262626"),
            ("accent-foreground", "#171717", "#fafafa"),
            ("border", "#e5e5e5", "#ffffff1a"),
            ("card", "#ffffff", "#171717"),
            ("card-foreground", "#0a0a0a", "#fafafa"),
            ("destructive", "#e7000b", "#ff6467"),
        ]
        .into_iter()
        .map(|(name, light, dark)| ColorToken {
            name: token(name),
            light: hex(light),
            dark: Some(hex(dark)),
        })
        .collect();
        let fonts = vec![
            FontToken {
                name: token("sans"),
                family: FontFamily::System {
                    stack: SystemFont::Sans,
                },
            },
            FontToken {
                name: token("mono"),
                family: FontFamily::System {
                    stack: SystemFont::Mono,
                },
            },
        ];
        let radii = [
            (Radius::None, 0),
            (Radius::Sm, 4),
            (Radius::Md, 6),
            (Radius::Lg, 8),
            (Radius::Xl, 12),
            (Radius::Xl2, 16),
            (Radius::Xl3, 24),
            (Radius::Full, 9999),
        ]
        .into_iter()
        .map(|(step, px)| RadiusToken { step, px })
        .collect();
        let shadows = vec![
            ShadowToken {
                step: Shadow::None,
                layers: vec![],
            },
            ShadowToken {
                step: Shadow::Xs,
                layers: vec![layer(0, 1, 2, 0, "#0000000d")],
            },
            ShadowToken {
                step: Shadow::Sm,
                layers: vec![layer(0, 1, 3, 0, "#0000001a"), layer(0, 1, 2, -1, "#0000001a")],
            },
            ShadowToken {
                step: Shadow::Md,
                layers: vec![layer(0, 4, 6, -1, "#0000001a"), layer(0, 2, 4, -2, "#0000001a")],
            },
            ShadowToken {
                step: Shadow::Lg,
                layers: vec![layer(0, 10, 15, -3, "#0000001a"), layer(0, 4, 6, -4, "#0000001a")],
            },
            ShadowToken {
                step: Shadow::Xl,
                layers: vec![layer(0, 20, 25, -5, "#0000001a"), layer(0, 8, 10, -6, "#0000001a")],
            },
            ShadowToken {
                step: Shadow::Xl2,
                layers: vec![layer(0, 25, 50, -12, "#00000040")],
            },
        ];
        Self {
            colors,
            fonts,
            spacing_unit: 4,
            radii,
            shadows,
        }
    }
}

impl DesignTokens {
    pub fn color(&self, name: &TokenName) -> Option<&ColorToken> {
        self.colors.iter().find(|c| &c.name == name)
    }

    pub fn font(&self, name: &TokenName) -> Option<&FontToken> {
        self.fonts.iter().find(|f| &f.name == name)
    }

    /// Vrai si au moins une couleur définit une valeur sombre.
    pub fn has_dark_mode(&self) -> bool {
        self.colors.iter().any(|c| c.dark.is_some())
    }

    /// Résout une référence de couleur en sRGB (`None` pour `current`, ou token inconnu).
    pub fn resolve_color(&self, color: &ColorRef, mode: ColorMode) -> Option<Rgba> {
        let base = match &color.source {
            ColorSource::Token(name) => {
                let token = self.color(name)?;
                let hex = match mode {
                    ColorMode::Dark => token.dark.as_ref().unwrap_or(&token.light),
                    ColorMode::Light => &token.light,
                };
                hex.rgba()
            }
            ColorSource::Palette { hue, shade } => palette_color(*hue, *shade),
            ColorSource::White => Rgba::WHITE,
            ColorSource::Black => Rgba::BLACK,
            ColorSource::Transparent => Rgba::TRANSPARENT,
            ColorSource::Current => return None,
        };
        Some(match color.alpha {
            Some(alpha) => base.with_alpha(alpha.fraction()),
            None => base,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_resolve_in_both_modes() {
        let tokens = DesignTokens::default();
        let primary: ColorRef = "primary/50".parse().unwrap();
        let light = tokens.resolve_color(&primary, ColorMode::Light).unwrap();
        let dark = tokens.resolve_color(&primary, ColorMode::Dark).unwrap();
        assert!((light.a - 0.5).abs() < 1e-9);
        assert!(dark.r > light.r);
        assert!(
            tokens
                .resolve_color(&"current".parse().unwrap(), ColorMode::Light)
                .is_none()
        );
        assert!(
            tokens
                .resolve_color(&"unknown".parse().unwrap(), ColorMode::Light)
                .is_none()
        );
        assert_eq!(tokens.spacing_unit, 4);
    }
}
