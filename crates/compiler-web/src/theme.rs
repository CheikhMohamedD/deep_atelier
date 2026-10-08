//! Tokens → `app/globals.css` (Tailwind v4) : variables CSS des couleurs, `@theme inline` pour
//! les utilitaires (`bg-primary`, `font-display`), valeurs sombres sous
//! `prefers-color-scheme: dark` (ADR 0001 § 12, décision 4), crans de rayon, d'ombre et
//! d'espacement qui diffèrent des valeurs de Tailwind.

use ir::{Document, FontFamily, Radius, Shadow, ShadowLayer, SystemFont};

use crate::names::kebab_case;

/// Noms de variables CSS que Tailwind réserve à son thème.
const RESERVED_VARIABLES: [&str; 1] = ["spacing"];

/// Variable CSS brute d'un token de couleur (`--primary`).
pub fn color_variable(name: &str) -> String {
    if RESERVED_VARIABLES.contains(&name) || name.starts_with("default-") {
        format!("--token-{name}")
    } else {
        format!("--{name}")
    }
}

/// Variable posée par `next/font` pour une famille Google (`--next-font-playfair-display`).
pub fn font_variable(family: &str) -> String {
    format!("--next-font-{}", kebab_case(family))
}

/// Rayons par défaut de Tailwind v4, en px.
fn default_radius(step: Radius) -> Option<u16> {
    Some(match step {
        Radius::Sm => 4,
        Radius::Md => 6,
        Radius::Lg => 8,
        Radius::Xl => 12,
        Radius::Xl2 => 16,
        Radius::Xl3 => 24,
        Radius::None | Radius::Full => return None,
    })
}

/// Couche d'ombre : (x, y, flou, étalement, couleur).
type DefaultLayer = (i16, i16, u16, i16, &'static str);

/// Ombres par défaut de Tailwind v4, couche par couche.
fn default_shadow(step: Shadow) -> Option<Vec<DefaultLayer>> {
    Some(match step {
        Shadow::Xs => vec![(0, 1, 2, 0, "#0000000d")],
        Shadow::Sm => vec![(0, 1, 3, 0, "#0000001a"), (0, 1, 2, -1, "#0000001a")],
        Shadow::Md => vec![(0, 4, 6, -1, "#0000001a"), (0, 2, 4, -2, "#0000001a")],
        Shadow::Lg => vec![(0, 10, 15, -3, "#0000001a"), (0, 4, 6, -4, "#0000001a")],
        Shadow::Xl => vec![(0, 20, 25, -5, "#0000001a"), (0, 8, 10, -6, "#0000001a")],
        Shadow::Xl2 => vec![(0, 25, 50, -12, "#00000040")],
        Shadow::None => return None,
    })
}

fn px(value: i64) -> String {
    if value == 0 {
        "0".to_owned()
    } else {
        format!("{value}px")
    }
}

fn shadow_value(layers: &[ShadowLayer]) -> String {
    layers
        .iter()
        .map(|l| {
            format!(
                "{}{} {} {} {} {}",
                if l.inset { "inset " } else { "" },
                px(i64::from(l.x)),
                px(i64::from(l.y)),
                px(i64::from(l.blur)),
                px(i64::from(l.spread)),
                l.color
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Contenu de `app/globals.css`.
pub fn globals_css(doc: &Document) -> String {
    let tokens = &doc.tokens;
    let mut out = String::from("@import \"tailwindcss\";\n");

    if !tokens.colors.is_empty() {
        out.push_str("\n:root {\n");
        for color in &tokens.colors {
            out.push_str(&format!(
                "  {}: {};\n",
                color_variable(color.name.as_str()),
                color.light
            ));
        }
        out.push_str("}\n");
    }

    let mut inline = Vec::new();
    for color in &tokens.colors {
        inline.push(format!(
            "  --color-{}: var({});",
            color.name,
            color_variable(color.name.as_str())
        ));
    }
    for font in &tokens.fonts {
        match &font.family {
            FontFamily::Google { family, .. } => {
                inline.push(format!("  --font-{}: var({});", font.name, font_variable(family)));
            }
            FontFamily::System { stack } => {
                let system = match stack {
                    SystemFont::Sans => "sans",
                    SystemFont::Serif => "serif",
                    SystemFont::Mono => "mono",
                };
                if font.name.as_str() != system {
                    inline.push(format!("  --font-{}: var(--font-{system});", font.name));
                }
            }
        }
    }
    if !inline.is_empty() {
        out.push_str("\n@theme inline {\n");
        for line in inline {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("}\n");
    }

    let mut theme = Vec::new();
    if tokens.spacing_unit != 4 {
        theme.push(format!("  --spacing: {}px;", tokens.spacing_unit));
    }
    for radius in &tokens.radii {
        if default_radius(radius.step).is_some_and(|d| d != radius.px) {
            theme.push(format!("  --radius-{}: {}px;", radius.step, radius.px));
        }
    }
    for shadow in &tokens.shadows {
        let Some(default) = default_shadow(shadow.step) else {
            continue;
        };
        let same = default.len() == shadow.layers.len()
            && default.iter().zip(&shadow.layers).all(|(d, l)| {
                !l.inset && (l.x, l.y, l.blur, l.spread) == (d.0, d.1, d.2, d.3) && l.color.as_str() == d.4
            });
        if !same && !shadow.layers.is_empty() {
            theme.push(format!("  --shadow-{}: {};", shadow.step, shadow_value(&shadow.layers)));
        }
    }
    if !theme.is_empty() {
        out.push_str("\n@theme {\n");
        for line in theme {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("}\n");
    }

    let dark: Vec<String> = tokens
        .colors
        .iter()
        .filter_map(|c| {
            c.dark
                .as_ref()
                .map(|d| format!("    {}: {};", color_variable(c.name.as_str()), d))
        })
        .collect();
    if !dark.is_empty() {
        out.push_str("\n@media (prefers-color-scheme: dark) {\n  :root {\n");
        for line in dark {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("  }\n}\n");
    }

    let has = |name: &str| tokens.colors.iter().any(|c| c.name.as_str() == name);
    if has("background") || has("foreground") {
        out.push_str("\nbody {\n");
        if has("background") {
            out.push_str(&format!("  background: var({});\n", color_variable("background")));
        }
        if has("foreground") {
            out.push_str(&format!("  color: var({});\n", color_variable("foreground")));
        }
        out.push_str("}\n");
    }
    out
}
