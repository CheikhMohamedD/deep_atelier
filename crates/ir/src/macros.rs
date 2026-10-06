//! Macros internes : énumérations sérialisées en chaînes et types valeur à forme textuelle.

/// Énumération de valeurs fixes sérialisées en chaînes compactes (`"4"`, `"2xl"`, `"center"`).
///
/// Génère `ALL`, `as_str`, `Display` et `FromStr`, et dérive serde, ts-rs et schemars
/// (union de littéraux côté TypeScript, `enum` de chaînes côté JSON Schema).
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
            serde::Serialize, serde::Deserialize, ts_rs::TS, schemars::JsonSchema,
        )]
        $vis enum $name {
            $( $(#[$vmeta])* #[serde(rename = $text)] $variant ),+
        }

        impl $name {
            /// Toutes les valeurs, dans l'ordre de l'échelle.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// Forme sérialisée.
            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::error::ValueError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok($name::$variant),)+
                    _ => Err($crate::error::ValueError::new(stringify!($name), s)),
                }
            }
        }
    };
}

/// Sérialisation par chaîne d'un type qui implémente `Display` et `FromStr<Err = ValueError>`,
/// avec un JSON Schema fourni par une fonction `fn(&mut SchemaGenerator) -> Schema`.
macro_rules! string_serde {
    ($name:ident, $schema:expr) => {
        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = <std::borrow::Cow<'de, str> as serde::Deserialize>::deserialize(deserializer)?;
                text.parse().map_err(serde::de::Error::custom)
            }
        }

        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                let build: fn(&mut schemars::SchemaGenerator) -> schemars::Schema = $schema;
                build(generator)
            }
        }
    };
}

pub(crate) use string_enum;
pub(crate) use string_serde;
