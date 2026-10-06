//! Valeurs par breakpoint, cascade mobile-first et patchs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Breakpoints alignés sur Tailwind (largeur minimale).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Breakpoint {
    Base,
    Sm,
    Md,
    Lg,
    Xl,
    #[serde(rename = "2xl")]
    #[ts(rename = "2xl")]
    Xxl,
}

impl Breakpoint {
    /// Du plus petit au plus grand.
    pub const ALL: [Breakpoint; 6] = [
        Breakpoint::Base,
        Breakpoint::Sm,
        Breakpoint::Md,
        Breakpoint::Lg,
        Breakpoint::Xl,
        Breakpoint::Xxl,
    ];

    /// Largeur minimale en px.
    pub fn min_width(self) -> u16 {
        match self {
            Breakpoint::Base => 0,
            Breakpoint::Sm => 640,
            Breakpoint::Md => 768,
            Breakpoint::Lg => 1024,
            Breakpoint::Xl => 1280,
            Breakpoint::Xxl => 1536,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Breakpoint::Base => "base",
            Breakpoint::Sm => "sm",
            Breakpoint::Md => "md",
            Breakpoint::Lg => "lg",
            Breakpoint::Xl => "xl",
            Breakpoint::Xxl => "2xl",
        }
    }
}

/// Valeur par breakpoint : `base` obligatoire, surcharges optionnelles au-dessus.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
#[schemars(bound = "T: JsonSchema", rename = "Responsive_{T}")]
pub struct Responsive<T> {
    pub base: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sm: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub md: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lg: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub xl: Option<T>,
    #[serde(rename = "2xl", default, skip_serializing_if = "Option::is_none")]
    #[ts(rename = "2xl", optional)]
    pub xxl: Option<T>,
}

impl<T> Responsive<T> {
    /// Valeur seulement en `base`.
    pub fn new(base: T) -> Self {
        Self {
            base,
            sm: None,
            md: None,
            lg: None,
            xl: None,
            xxl: None,
        }
    }

    fn slot(&self, bp: Breakpoint) -> Option<&T> {
        match bp {
            Breakpoint::Base => Some(&self.base),
            Breakpoint::Sm => self.sm.as_ref(),
            Breakpoint::Md => self.md.as_ref(),
            Breakpoint::Lg => self.lg.as_ref(),
            Breakpoint::Xl => self.xl.as_ref(),
            Breakpoint::Xxl => self.xxl.as_ref(),
        }
    }

    fn slot_mut(&mut self, bp: Breakpoint) -> Option<&mut Option<T>> {
        match bp {
            Breakpoint::Base => None,
            Breakpoint::Sm => Some(&mut self.sm),
            Breakpoint::Md => Some(&mut self.md),
            Breakpoint::Lg => Some(&mut self.lg),
            Breakpoint::Xl => Some(&mut self.xl),
            Breakpoint::Xxl => Some(&mut self.xxl),
        }
    }

    /// Valeur déclarée exactement à ce breakpoint.
    pub fn get(&self, bp: Breakpoint) -> Option<&T> {
        self.slot(bp)
    }

    /// Valeur effective à ce breakpoint (cascade mobile-first).
    pub fn resolve(&self, bp: Breakpoint) -> &T {
        let origin = self.origin(bp);
        self.slot(origin).unwrap_or(&self.base)
    }

    /// Breakpoint d'où provient la valeur effective (affiché par l'inspecteur).
    pub fn origin(&self, bp: Breakpoint) -> Breakpoint {
        Breakpoint::ALL
            .iter()
            .copied()
            .filter(|b| *b <= bp)
            .rev()
            .find(|b| self.slot(*b).is_some())
            .unwrap_or(Breakpoint::Base)
    }

    /// Pose une valeur à un breakpoint.
    pub fn set(&mut self, bp: Breakpoint, value: T) {
        match self.slot_mut(bp) {
            Some(slot) => *slot = Some(value),
            None => self.base = value,
        }
    }

    /// Efface la surcharge d'un breakpoint (> base) ; retourne l'ancienne valeur.
    pub fn clear(&mut self, bp: Breakpoint) -> Option<T> {
        self.slot_mut(bp).and_then(Option::take)
    }

    /// Breakpoints déclarés, du plus petit au plus grand.
    pub fn declared(&self) -> Vec<Breakpoint> {
        Breakpoint::ALL
            .into_iter()
            .filter(|bp| self.slot(*bp).is_some())
            .collect()
    }

    /// Toutes les valeurs déclarées.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        Breakpoint::ALL.into_iter().filter_map(|bp| self.slot(bp))
    }

    pub fn map<U>(&self, mut f: impl FnMut(&T) -> U) -> Responsive<U> {
        Responsive {
            base: f(&self.base),
            sm: self.sm.as_ref().map(&mut f),
            md: self.md.as_ref().map(&mut f),
            lg: self.lg.as_ref().map(&mut f),
            xl: self.xl.as_ref().map(&mut f),
            xxl: self.xxl.as_ref().map(&mut f),
        }
    }
}

/// Désérialise un champ à trois états : absent (`None`), `null` (`Some(None)`), valeur.
pub(crate) mod double_option {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Some)
    }
}

/// Patch d'une valeur responsive : breakpoint absent = inchangé, `null` = surcharge effacée
/// (« réinitialiser à l'héritage »), valeur = posée. `base` ne peut pas être effacé : pour
/// supprimer la propriété, le patch de style la met à `null`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
#[schemars(bound = "T: JsonSchema", rename = "ResponsivePatch_{T}")]
pub struct ResponsivePatch<T> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub base: Option<T>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option::deserialize"
    )]
    #[ts(optional)]
    pub sm: Option<Option<T>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option::deserialize"
    )]
    #[ts(optional)]
    pub md: Option<Option<T>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option::deserialize"
    )]
    #[ts(optional)]
    pub lg: Option<Option<T>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option::deserialize"
    )]
    #[ts(optional)]
    pub xl: Option<Option<T>>,
    #[serde(
        rename = "2xl",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "double_option::deserialize"
    )]
    #[ts(rename = "2xl", optional)]
    pub xxl: Option<Option<T>>,
}

impl<T> Default for ResponsivePatch<T> {
    fn default() -> Self {
        Self {
            base: None,
            sm: None,
            md: None,
            lg: None,
            xl: None,
            xxl: None,
        }
    }
}

impl<T: Clone> ResponsivePatch<T> {
    /// Patch qui pose une valeur à un breakpoint.
    pub fn at(bp: Breakpoint, value: T) -> Self {
        let mut patch = Self::default();
        patch.set(bp, Some(value));
        patch
    }

    /// Patch qui pose toutes les valeurs déclarées de `value`.
    pub fn from_responsive(value: &Responsive<T>) -> Self {
        let mut patch = Self::default();
        for bp in value.declared() {
            patch.set(bp, value.get(bp).cloned());
        }
        patch
    }

    fn set(&mut self, bp: Breakpoint, value: Option<T>) {
        match bp {
            Breakpoint::Base => self.base = value,
            Breakpoint::Sm => self.sm = Some(value),
            Breakpoint::Md => self.md = Some(value),
            Breakpoint::Lg => self.lg = Some(value),
            Breakpoint::Xl => self.xl = Some(value),
            Breakpoint::Xxl => self.xxl = Some(value),
        }
    }

    fn entry(&self, bp: Breakpoint) -> Option<Option<&T>> {
        match bp {
            Breakpoint::Base => self.base.as_ref().map(Some),
            Breakpoint::Sm => self.sm.as_ref().map(Option::as_ref),
            Breakpoint::Md => self.md.as_ref().map(Option::as_ref),
            Breakpoint::Lg => self.lg.as_ref().map(Option::as_ref),
            Breakpoint::Xl => self.xl.as_ref().map(Option::as_ref),
            Breakpoint::Xxl => self.xxl.as_ref().map(Option::as_ref),
        }
    }

    /// Valeurs posées par le patch.
    pub fn values(&self) -> Vec<&T> {
        Breakpoint::ALL
            .into_iter()
            .filter_map(|bp| self.entry(bp).flatten())
            .collect()
    }

    /// Breakpoints touchés par le patch.
    pub fn touched(&self) -> Vec<Breakpoint> {
        Breakpoint::ALL
            .into_iter()
            .filter(|bp| self.entry(*bp).is_some())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.touched().is_empty()
    }

    /// Applique le patch à une valeur existante, ou à `neutral` si la propriété est absente
    /// (la base neutre ne change pas le rendu en `base`).
    pub fn apply(&self, current: Option<Responsive<T>>, neutral: impl FnOnce() -> T) -> Responsive<T> {
        let start = match (current, &self.base) {
            (Some(value), _) => value,
            (None, Some(base)) => Responsive::new(base.clone()),
            (None, None) => Responsive::new(neutral()),
        };
        self.merge_into(start)
    }

    /// Applique le patch à une valeur de départ.
    pub fn merge_into(&self, mut value: Responsive<T>) -> Responsive<T> {
        for bp in Breakpoint::ALL {
            match self.entry(bp) {
                Some(Some(v)) => value.set(bp, v.clone()),
                Some(None) => {
                    value.clear(bp);
                }
                None => {}
            }
        }
        value
    }

    pub fn map<U>(&self, mut f: impl FnMut(&T) -> U) -> ResponsivePatch<U> {
        let mut map_entry = |entry: &Option<Option<T>>| entry.as_ref().map(|v| v.as_ref().map(&mut f));
        ResponsivePatch {
            sm: map_entry(&self.sm),
            md: map_entry(&self.md),
            lg: map_entry(&self.lg),
            xl: map_entry(&self.xl),
            xxl: map_entry(&self.xxl),
            base: self.base.as_ref().map(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cascade_is_mobile_first() {
        let mut value = Responsive::new(1);
        value.set(Breakpoint::Md, 2);
        value.set(Breakpoint::Xl, 3);
        assert_eq!(*value.resolve(Breakpoint::Sm), 1);
        assert_eq!(*value.resolve(Breakpoint::Lg), 2);
        assert_eq!(value.origin(Breakpoint::Lg), Breakpoint::Md);
        assert_eq!(*value.resolve(Breakpoint::Xxl), 3);
        assert_eq!(value.get(Breakpoint::Lg), None);
        assert_eq!(value.declared(), vec![Breakpoint::Base, Breakpoint::Md, Breakpoint::Xl]);
    }

    #[test]
    fn patch_distinguishes_absent_null_and_value() {
        let patch: ResponsivePatch<u8> = serde_json::from_str(r#"{ "md": null, "lg": 4 }"#).unwrap();
        assert_eq!(patch.md, Some(None));
        assert_eq!(patch.lg, Some(Some(4)));
        assert_eq!(patch.sm, None);
        let mut current = Responsive::new(1);
        current.set(Breakpoint::Md, 2);
        let next = patch.apply(Some(current), || 0);
        assert_eq!(next.declared(), vec![Breakpoint::Base, Breakpoint::Lg]);
        assert_eq!(serde_json::to_string(&patch).unwrap(), r#"{"md":null,"lg":4}"#);
    }

    #[test]
    fn override_on_absent_property_keeps_base_neutral() {
        let patch = ResponsivePatch::at(Breakpoint::Lg, 8);
        let next = patch.apply(None, || 0);
        assert_eq!(next.base, 0);
        assert_eq!(next.lg, Some(8));
    }

    #[test]
    fn breakpoint_serializes_2xl() {
        assert_eq!(serde_json::to_string(&Breakpoint::Xxl).unwrap(), "\"2xl\"");
        let value: Responsive<u8> = serde_json::from_str(r#"{ "base": 1, "2xl": 3 }"#).unwrap();
        assert_eq!(value.xxl, Some(3));
    }
}
