//! Identifiants stables de l'IR et générateur pseudo-aléatoire déterministe.
//!
//! Un identifiant est un préfixe d'entité suivi de 10 caractères base36 (`n_k3f9x2a7qz`).
//! Il est attribué à la création et ne change plus jamais (clé de synchro code ⇄ canvas).

use std::fmt;
use std::str::FromStr;

use ts_rs::TS;

use crate::error::{ValueError, pattern_schema};
use crate::macros::string_serde;

/// Longueur de la partie aléatoire d'un identifiant.
pub const ID_SUFFIX_LEN: usize = 10;

const BASE36: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

fn is_valid_id(text: &str, prefix: &str) -> bool {
    text.len() == prefix.len() + 1 + ID_SUFFIX_LEN
        && text.starts_with(prefix)
        && text.as_bytes()[prefix.len()] == b'_'
        && text[prefix.len() + 1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
}

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident, $prefix:literal, $pattern:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
        #[ts(type = "string")]
        pub struct $name(String);

        impl $name {
            /// Préfixe d'entité.
            pub const PREFIX: &'static str = $prefix;

            /// Forme textuelle.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = ValueError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if is_valid_id(s, $prefix) {
                    Ok(Self(s.to_owned()))
                } else {
                    Err(ValueError::new(stringify!($name), s))
                }
            }
        }

        string_serde!($name, |_| pattern_schema($pattern));
    };
}

define_id!(
    /// Identifiant de nœud (`n_…`).
    NodeId, "n", "^n_[0-9a-z]{10}$"
);
define_id!(
    /// Identifiant de page (`p_…`).
    PageId, "p", "^p_[0-9a-z]{10}$"
);
define_id!(
    /// Identifiant de layout (`l_…`).
    LayoutId, "l", "^l_[0-9a-z]{10}$"
);
define_id!(
    /// Identifiant de composant (`c_…`).
    ComponentId, "c", "^c_[0-9a-z]{10}$"
);
define_id!(
    /// Identifiant d'asset (`a_…`).
    AssetId, "a", "^a_[0-9a-z]{10}$"
);

/// Nom de token en kebab-case (`primary`, `muted-foreground`).
///
/// Les noms réservés aux couleurs de base (`white`, `black`, `transparent`, `current`) et ceux de
/// la forme `<teinte>-<nuance>` de la palette Tailwind sont refusés, pour que `ColorRef` reste
/// non ambigu.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(type = "string")]
pub struct TokenName(String);

impl TokenName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TokenName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Vrai si `text` est en kebab-case ASCII minuscule (`a`, `a-b`, `h2-title`).
pub(crate) fn is_kebab_case(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    !text.ends_with('-')
        && !text.contains("--")
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Vrai si `text` est un identifiant camelCase ASCII (`title`, `imageAlt`, `children`) : nom de
/// prop, d'axe de variantes ou de slot, qui devient une prop TypeScript du composant.
pub(crate) fn is_camel_case(text: &str) -> bool {
    text.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && text.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Longueur maximale d'un nom de token.
pub(crate) const TOKEN_NAME_MAX_LEN: usize = 48;

/// Couleurs de base réservées (en plus des noms de palette `<teinte>-<nuance>`).
const RESERVED_TOKEN_NAMES: [&str; 4] = ["white", "black", "transparent", "current"];

impl FromStr for TokenName {
    type Err = ValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let reserved = RESERVED_TOKEN_NAMES.contains(&s) || crate::style::color::parse_palette(s).is_some();
        if is_kebab_case(s) && s.len() <= TOKEN_NAME_MAX_LEN && !reserved {
            Ok(Self(s.to_owned()))
        } else {
            Err(ValueError::new("TokenName", s))
        }
    }
}

/// Schéma de `TokenName` : motif kebab-case et longueur maximale. Exclure les noms réservés
/// demanderait `not` ou un lookahead, mal pris en charge par les schémas d'outils stricts : ils
/// sont listés dans la description.
fn token_name_schema() -> schemars::Schema {
    use crate::style::color::{Hue, Shade};
    let hues: Vec<&str> = Hue::ALL.iter().map(|hue| hue.as_str()).collect();
    let shades: Vec<&str> = Shade::ALL.iter().map(|shade| shade.as_str()).collect();
    let description = format!(
        "Nom de token en kebab-case, {TOKEN_NAME_MAX_LEN} caractères au plus. Noms réservés, refusés : {}, \
         et les couleurs de palette `<teinte>-<nuance>` (teintes : {} ; nuances : {}).",
        RESERVED_TOKEN_NAMES.join(", "),
        hues.join(", "),
        shades.join(", "),
    );
    schemars::json_schema!({
        "type": "string",
        "pattern": "^[a-z][a-z0-9]*(-[a-z0-9]+)*$",
        "maxLength": TOKEN_NAME_MAX_LEN,
        "description": description,
    })
}

string_serde!(TokenName, |_| token_name_schema());

/// Référence de nœud dans une commande : id existant (`n_…`) ou référence locale (`$nom`)
/// créée plus tôt dans la transaction ou dans le run IA.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(type = "string")]
pub enum NodeRef {
    Id(NodeId),
    Local(String),
}

impl NodeRef {
    /// Vrai si `text` est un nom de référence locale valide (`$hero`, `$pricing-grid`).
    pub fn is_local_name(text: &str) -> bool {
        text.strip_prefix('$').is_some_and(|name| {
            !name.is_empty()
                && name.len() <= 64
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
    }
}

impl From<NodeId> for NodeRef {
    fn from(id: NodeId) -> Self {
        NodeRef::Id(id)
    }
}

impl From<&NodeId> for NodeRef {
    fn from(id: &NodeId) -> Self {
        NodeRef::Id(id.clone())
    }
}

impl fmt::Display for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeRef::Id(id) => f.write_str(id.as_str()),
            NodeRef::Local(name) => f.write_str(name),
        }
    }
}

impl FromStr for NodeRef {
    type Err = ValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if Self::is_local_name(s) {
            Ok(NodeRef::Local(s.to_owned()))
        } else {
            s.parse().map(NodeRef::Id).map_err(|_| ValueError::new("NodeRef", s))
        }
    }
}

string_serde!(NodeRef, |_| pattern_schema("^(n_[0-9a-z]{10}|\\$[A-Za-z0-9_-]{1,64})$"));

/// Générateur d'identifiants : SplitMix64 initialisé par une graine fournie par l'appelant
/// (`crypto.getRandomValues` côté wasm, horloge ou OS côté serveur). La crate reste ainsi pure
/// et déterministe en test.
#[derive(Debug, Clone)]
pub struct IdGen {
    state: u64,
}

impl IdGen {
    pub fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn suffix(&mut self) -> String {
        let mut value = self.next_u64();
        let mut out = [0u8; ID_SUFFIX_LEN];
        for slot in out.iter_mut().rev() {
            *slot = BASE36[(value % 36) as usize];
            value /= 36;
        }
        out.iter().map(|&b| b as char).collect()
    }

    fn raw(&mut self, prefix: &str) -> String {
        format!("{prefix}_{}", self.suffix())
    }

    /// Nouvel id de nœud absent de `taken`.
    pub fn node(&mut self, taken: impl Fn(&NodeId) -> bool) -> NodeId {
        loop {
            let id = NodeId(self.raw(NodeId::PREFIX));
            if !taken(&id) {
                return id;
            }
        }
    }

    pub fn page(&mut self, taken: impl Fn(&PageId) -> bool) -> PageId {
        loop {
            let id = PageId(self.raw(PageId::PREFIX));
            if !taken(&id) {
                return id;
            }
        }
    }

    pub fn layout(&mut self, taken: impl Fn(&LayoutId) -> bool) -> LayoutId {
        loop {
            let id = LayoutId(self.raw(LayoutId::PREFIX));
            if !taken(&id) {
                return id;
            }
        }
    }

    pub fn component(&mut self, taken: impl Fn(&ComponentId) -> bool) -> ComponentId {
        loop {
            let id = ComponentId(self.raw(ComponentId::PREFIX));
            if !taken(&id) {
                return id;
            }
        }
    }

    pub fn asset(&mut self, taken: impl Fn(&AssetId) -> bool) -> AssetId {
        loop {
            let id = AssetId(self.raw(AssetId::PREFIX));
            if !taken(&id) {
                return id;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_have_prefix_and_base36_suffix() {
        let mut ids = IdGen::from_seed(42);
        let id = ids.node(|_| false);
        assert!(is_valid_id(id.as_str(), "n"), "{id}");
        assert_eq!(id.as_str().parse::<NodeId>().unwrap(), id);
        assert!("p_abc".parse::<PageId>().is_err());
        assert!("n_ABCDEFGHIJ".parse::<NodeId>().is_err());
    }

    #[test]
    fn generator_is_deterministic_and_skips_taken_ids() {
        let first = IdGen::from_seed(7).node(|_| false);
        let again = IdGen::from_seed(7).node(|_| false);
        assert_eq!(first, again);
        let other = IdGen::from_seed(7).node(|id| *id == first);
        assert_ne!(other, first);
    }

    #[test]
    fn node_refs_parse_ids_and_locals() {
        assert!(matches!("$hero".parse::<NodeRef>(), Ok(NodeRef::Local(_))));
        assert!(matches!("n_0123456789".parse::<NodeRef>(), Ok(NodeRef::Id(_))));
        assert!("hero".parse::<NodeRef>().is_err());
        assert!("$".parse::<NodeRef>().is_err());
    }

    #[test]
    fn token_names_are_kebab_case_and_not_reserved() {
        assert!("muted-foreground".parse::<TokenName>().is_ok());
        assert!("Primary".parse::<TokenName>().is_err());
        assert!("white".parse::<TokenName>().is_err());
        assert!("red-500".parse::<TokenName>().is_err());
        assert!("brand-500".parse::<TokenName>().is_ok());
    }
}
