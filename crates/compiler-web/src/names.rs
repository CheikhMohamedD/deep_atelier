//! Noms dérivés de l'IR : identifiants TypeScript, chemins de route, noms de fichiers.

use ir::{RouteSegment, route_path};

/// Retire les accents des lettres latines courantes.
fn fold(c: char) -> &'static str {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => "a",
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => "A",
        'æ' => "ae",
        'Æ' => "AE",
        'ç' => "c",
        'Ç' => "C",
        'è' | 'é' | 'ê' | 'ë' => "e",
        'È' | 'É' | 'Ê' | 'Ë' => "E",
        'ì' | 'í' | 'î' | 'ï' => "i",
        'Ì' | 'Í' | 'Î' | 'Ï' => "I",
        'ñ' => "n",
        'Ñ' => "N",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => "o",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => "O",
        'œ' => "oe",
        'Œ' => "OE",
        'ù' | 'ú' | 'û' | 'ü' => "u",
        'Ù' | 'Ú' | 'Û' | 'Ü' => "U",
        'ý' | 'ÿ' => "y",
        'Ý' => "Y",
        'ß' => "ss",
        _ => "",
    }
}

/// Mots ASCII d'un texte libre (accents retirés, séparateurs ignorés).
fn words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for c in text.chars() {
        let folded = if c.is_ascii() { "" } else { fold(c) };
        let piece: String = if c.is_ascii_alphanumeric() {
            c.to_string()
        } else {
            folded.to_owned()
        };
        if piece.is_empty() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            previous_lower = false;
            continue;
        }
        // Frontière camelCase : `HeroSection` → `Hero`, `Section`.
        let upper = piece.chars().next().is_some_and(|c| c.is_ascii_uppercase());
        if upper && previous_lower && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        previous_lower = piece
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        current.push_str(&piece);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Identifiant PascalCase (`"À propos"` → `"APropos"`), vide si le texte n'a pas de lettre.
pub fn pascal_case(text: &str) -> String {
    let mut out = String::new();
    for word in words(text) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars.map(|c| c.to_ascii_lowercase()));
        }
    }
    // Un identifiant ne commence pas par un chiffre.
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, 'N');
    }
    out
}

/// Identifiant camelCase (`"menu principal"` → `"menuPrincipal"`).
pub fn camel_case(text: &str) -> String {
    let pascal = pascal_case(text);
    let mut chars = pascal.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// Segment de chemin kebab-case (`"Photo d'équipe"` → `"photo-d-equipe"`).
pub fn kebab_case(text: &str) -> String {
    words(text)
        .into_iter()
        .map(|w| w.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

/// Composant lucide-react d'une icône (`"arrow-right"` → `"ArrowRightIcon"`).
pub fn icon_component(name: &str) -> String {
    let mut out: String = name
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();
    out.push_str("Icon");
    out
}

/// Fonction `next/font/google` d'une famille (`"Playfair Display"` → `"Playfair_Display"`).
pub fn google_font_function(family: &str) -> String {
    family
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("_")
}

/// Dossier d'une route sous `app/` (`[blog, [slug]]` → `blog/[slug]`), vide pour `/`.
pub fn route_folder(route: &[RouteSegment]) -> String {
    route_path(route).trim_start_matches('/').to_owned()
}

/// Rend un identifiant unique parmi ceux déjà pris (`Card`, `Card2`…).
pub fn unique(base: &str, taken: &mut std::collections::BTreeSet<String>) -> String {
    let mut name = base.to_owned();
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{base}{n}");
        n += 1;
    }
    taken.insert(name.clone());
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers() {
        assert_eq!(pascal_case("À propos"), "APropos");
        assert_eq!(pascal_case("HeroSection"), "HeroSection");
        assert_eq!(pascal_case("menu principal"), "MenuPrincipal");
        assert_eq!(pascal_case("2 colonnes"), "N2Colonnes");
        assert_eq!(camel_case("Menu principal"), "menuPrincipal");
        assert_eq!(kebab_case("Photo d'équipe"), "photo-d-equipe");
        assert_eq!(icon_component("arrow-down-0-1"), "ArrowDown01Icon");
        assert_eq!(google_font_function("Playfair Display"), "Playfair_Display");
        assert_eq!(
            route_folder(&[RouteSegment::Static("blog".into()), RouteSegment::Param("slug".into())]),
            "blog/[slug]"
        );
    }
}
