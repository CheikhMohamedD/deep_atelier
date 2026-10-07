//! Style d'un nœud : une propriété = une valeur responsive optionnelle.
//!
//! La liste des propriétés est déclarée une seule fois (`style_props!`) ; la macro génère le
//! `Style`, l'énumération `StyleProp`, l'accès générique par propriété et le `StylePatch`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::TokenName;
use crate::node::PlatformScope;
use crate::style::color::ColorRef;
use crate::style::responsive::{Breakpoint, Responsive, ResponsivePatch, double_option};
use crate::style::values::*;

/// Fond : couleur ou dégradé linéaire.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "kind")]
pub enum Background {
    Color {
        color: ColorRef,
    },
    Gradient {
        direction: GradientDir,
        from: ColorRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        via: Option<ColorRef>,
        to: ColorRef,
    },
}

impl Background {
    /// Références de couleur utilisées.
    pub fn colors(&self) -> Vec<&ColorRef> {
        match self {
            Background::Color { color } => vec![color],
            Background::Gradient { from, via, to, .. } => {
                let mut out = vec![from];
                out.extend(via.as_ref());
                out.push(to);
                out
            }
        }
    }
}

/// Anneau de focus.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct Ring {
    pub width: RingWidth,
    pub color: ColorRef,
    pub offset: RingWidth,
}

/// États d'interaction (WebOnly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InteractionState {
    Hover,
    FocusVisible,
    Active,
}

impl InteractionState {
    pub const ALL: [InteractionState; 3] = [
        InteractionState::Hover,
        InteractionState::FocusVisible,
        InteractionState::Active,
    ];
}

/// Erreur d'accès générique à une propriété de style.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StyleError {
    #[error("value type does not match style property `{0}`")]
    TypeMismatch(&'static str),
    #[error("style property `{0}` is not available in interaction states")]
    NotAStateProp(&'static str),
    #[error("style property `{0}` is only available in interaction states")]
    StateOnly(&'static str),
}

/// Déclare les types de valeur responsive et leurs patchs.
macro_rules! value_types {
    ($($variant:ident($ty:ty)),+ $(,)?) => {
        /// Valeur responsive typée d'une propriété de style (forme générique, utilisée par les ops).
        // Les fonds en dégradé sont plus gros que les autres valeurs ; ces valeurs sont éphémères
        // (ops, patchs) et ne justifient pas une indirection.
        #[allow(clippy::large_enum_variant)]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
        pub enum ResponsiveValue {
            $($variant(Responsive<$ty>)),+
        }

        /// Patch responsive typé d'une propriété de style.
        #[allow(clippy::large_enum_variant)]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum ResponsiveValuePatch {
            $($variant(ResponsivePatch<$ty>)),+
        }

        impl ResponsiveValuePatch {
            /// Applique le patch ; `neutral` fournit une valeur de base (même variante) si la
            /// propriété est absente. Une propriété absente le reste (`None`) si le patch ne pose
            /// aucune valeur (patch vide ou effacements seuls).
            pub fn apply(
                &self,
                current: Option<ResponsiveValue>,
                neutral: impl FnOnce() -> ResponsiveValue,
            ) -> Result<Option<ResponsiveValue>, StyleError> {
                match self {
                    $(ResponsiveValuePatch::$variant(patch) => {
                        let current = match current {
                            None => None,
                            Some(ResponsiveValue::$variant(value)) => Some(value),
                            Some(_) => return Err(StyleError::TypeMismatch(stringify!($variant))),
                        };
                        let start = match (current, &patch.base) {
                            (Some(value), _) => value,
                            (None, Some(base)) => Responsive::new(base.clone()),
                            (None, None) if !patch.sets_value() => return Ok(None),
                            (None, None) => match neutral() {
                                ResponsiveValue::$variant(value) => Responsive::new(value.base),
                                _ => return Err(StyleError::TypeMismatch(stringify!($variant))),
                            },
                        };
                        let value = patch.merge_into(start);
                        Ok(Some(ResponsiveValue::$variant(value)))
                    })+
                }
            }

            /// Breakpoints touchés.
            pub fn touched(&self) -> Vec<Breakpoint> {
                match self {
                    $(ResponsiveValuePatch::$variant(patch) => patch.touched()),+
                }
            }
        }

        impl ResponsiveValue {
            /// Breakpoints déclarés.
            pub fn declared(&self) -> Vec<Breakpoint> {
                match self {
                    $(ResponsiveValue::$variant(value) => value.declared()),+
                }
            }

            /// Patch qui reproduit toutes les valeurs déclarées.
            pub fn to_patch(&self) -> ResponsiveValuePatch {
                match self {
                    $(ResponsiveValue::$variant(value) => {
                        ResponsiveValuePatch::$variant(ResponsivePatch::from_responsive(value))
                    })+
                }
            }

            /// Restreint la valeur à son seul `base` (valeur neutre).
            pub fn base_only(&self) -> ResponsiveValue {
                match self {
                    $(ResponsiveValue::$variant(value) => {
                        ResponsiveValue::$variant(Responsive::new(value.base.clone()))
                    })+
                }
            }

            /// Valeur effective à un breakpoint, réduite à un `base`.
            pub fn at(&self, bp: Breakpoint) -> ResponsiveValue {
                match self {
                    $(ResponsiveValue::$variant(value) => {
                        ResponsiveValue::$variant(Responsive::new(value.resolve(bp).clone()))
                    })+
                }
            }
        }
    };
}

value_types! {
    Direction(Direction),
    Bool(bool),
    GridColumns(GridColumns),
    Space(Space),
    Align(Align),
    AlignSelf(AlignSelf),
    Justify(Justify),
    GridSpan(GridSpan),
    Order(Order),
    Margin(Margin),
    Size(Size),
    AspectRatio(AspectRatio),
    Position(Position),
    Inset(Inset),
    ZIndex(ZIndex),
    Token(TokenName),
    FontSize(FontSize),
    FontWeight(FontWeight),
    LineHeight(LineHeight),
    LetterSpacing(LetterSpacing),
    TextAlign(TextAlign),
    Color(ColorRef),
    TextTransform(TextTransform),
    TextDecoration(TextDecoration),
    TextWrap(TextWrap),
    Background(Background),
    BorderWidth(BorderWidth),
    BorderStyle(BorderStyle),
    Radius(Radius),
    Shadow(Shadow),
    Opacity(Opacity),
    Overflow(Overflow),
    ObjectFit(ObjectFit),
    Transition(Transition),
    Duration(Duration),
    Scale(Scale),
    Ring(Ring),
}

macro_rules! style_props {
    (
        base { $($field:ident : $ty:ty => $prop:ident / $variant:ident, $scope:ident, $inherited:literal;)+ }
        state_only { $($sfield:ident : $sty:ty => $sprop:ident / $svariant:ident;)+ }
        state { $($stfield:ident : $stty:ty => $stprop:ident / $stvariant:ident;)+ }
    ) => {
        /// Style d'un nœud. Chaque propriété absente n'est pas définie (héritage ou défaut CSS).
        #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
        pub struct Style {
            $(
                #[serde(default, skip_serializing_if = "Option::is_none")]
                #[ts(optional)]
                pub $field: Option<Responsive<$ty>>,
            )+
        }

        /// Style d'un état d'interaction (sous-ensemble des propriétés, plus `scale` et `ring`).
        #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
        pub struct StateStyle {
            $(
                #[serde(default, skip_serializing_if = "Option::is_none")]
                #[ts(optional)]
                pub $stfield: Option<Responsive<$stty>>,
            )+
            $(
                #[serde(default, skip_serializing_if = "Option::is_none")]
                #[ts(optional)]
                pub $sfield: Option<Responsive<$sty>>,
            )+
        }

        /// Identifiant de propriété de style.
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema,
        )]
        #[serde(rename_all = "snake_case")]
        pub enum StyleProp {
            $($prop,)+
            $($sprop,)+
        }

        impl StyleProp {
            /// Toutes les propriétés, dans l'ordre de déclaration.
            pub const ALL: &'static [StyleProp] = &[$(StyleProp::$prop,)+ $(StyleProp::$sprop,)+];

            /// Nom sérialisé (snake_case).
            pub fn name(self) -> &'static str {
                match self {
                    $(StyleProp::$prop => stringify!($field),)+
                    $(StyleProp::$sprop => stringify!($sfield),)+
                }
            }

            /// Plateformes où la propriété a un sens.
            pub fn scope(self) -> PlatformScope {
                match self {
                    $(StyleProp::$prop => PlatformScope::$scope,)+
                    $(StyleProp::$sprop => PlatformScope::WebOnly,)+
                }
            }

            /// Propriété typographique héritée par les descendants.
            pub fn is_inherited(self) -> bool {
                match self {
                    $(StyleProp::$prop => $inherited,)+
                    $(StyleProp::$sprop => false,)+
                }
            }

            /// Disponible dans les états d'interaction.
            pub fn in_states(self) -> bool {
                matches!(self, $(StyleProp::$sprop)|+ $(| StyleProp::$stprop)+)
            }

            /// Disponible hors états (dans `Style`).
            pub fn in_base(self) -> bool {
                !matches!(self, $(StyleProp::$sprop)|+)
            }
        }

        impl Style {
            /// Valeur générique d'une propriété.
            pub fn get(&self, prop: StyleProp) -> Result<Option<ResponsiveValue>, StyleError> {
                match prop {
                    $(StyleProp::$prop => Ok(self.$field.clone().map(ResponsiveValue::$variant)),)+
                    $(StyleProp::$sprop => Err(StyleError::StateOnly(stringify!($sfield))),)+
                }
            }

            /// Remplace une propriété ; retourne l'ancienne valeur.
            pub fn set(
                &mut self,
                prop: StyleProp,
                value: Option<ResponsiveValue>,
            ) -> Result<Option<ResponsiveValue>, StyleError> {
                match (prop, value) {
                    $(
                        (StyleProp::$prop, None) => Ok(self.$field.take().map(ResponsiveValue::$variant)),
                        (StyleProp::$prop, Some(ResponsiveValue::$variant(v))) => {
                            Ok(self.$field.replace(v).map(ResponsiveValue::$variant))
                        }
                        (StyleProp::$prop, Some(_)) => Err(StyleError::TypeMismatch(stringify!($field))),
                    )+
                    $((StyleProp::$sprop, _) => Err(StyleError::StateOnly(stringify!($sfield))),)+
                }
            }

            /// Propriétés définies, avec leur valeur.
            pub fn entries(&self) -> Vec<(StyleProp, ResponsiveValue)> {
                let mut out = Vec::new();
                $(
                    if let Some(v) = &self.$field {
                        out.push((StyleProp::$prop, ResponsiveValue::$variant(v.clone())));
                    }
                )+
                out
            }

            pub fn is_empty(&self) -> bool {
                *self == Style::default()
            }
        }

        impl StateStyle {
            pub fn get(&self, prop: StyleProp) -> Result<Option<ResponsiveValue>, StyleError> {
                match prop {
                    $(StyleProp::$stprop => Ok(self.$stfield.clone().map(ResponsiveValue::$stvariant)),)+
                    $(StyleProp::$sprop => Ok(self.$sfield.clone().map(ResponsiveValue::$svariant)),)+
                    _ => Err(StyleError::NotAStateProp(prop.name())),
                }
            }

            pub fn set(
                &mut self,
                prop: StyleProp,
                value: Option<ResponsiveValue>,
            ) -> Result<Option<ResponsiveValue>, StyleError> {
                match (prop, value) {
                    $(
                        (StyleProp::$stprop, None) => Ok(self.$stfield.take().map(ResponsiveValue::$stvariant)),
                        (StyleProp::$stprop, Some(ResponsiveValue::$stvariant(v))) => {
                            Ok(self.$stfield.replace(v).map(ResponsiveValue::$stvariant))
                        }
                        (StyleProp::$stprop, Some(_)) => Err(StyleError::TypeMismatch(stringify!($stfield))),
                    )+
                    $(
                        (StyleProp::$sprop, None) => Ok(self.$sfield.take().map(ResponsiveValue::$svariant)),
                        (StyleProp::$sprop, Some(ResponsiveValue::$svariant(v))) => {
                            Ok(self.$sfield.replace(v).map(ResponsiveValue::$svariant))
                        }
                        (StyleProp::$sprop, Some(_)) => Err(StyleError::TypeMismatch(stringify!($sfield))),
                    )+
                    (prop, _) => Err(StyleError::NotAStateProp(prop.name())),
                }
            }

            pub fn entries(&self) -> Vec<(StyleProp, ResponsiveValue)> {
                let mut out = Vec::new();
                $(
                    if let Some(v) = &self.$stfield {
                        out.push((StyleProp::$stprop, ResponsiveValue::$stvariant(v.clone())));
                    }
                )+
                $(
                    if let Some(v) = &self.$sfield {
                        out.push((StyleProp::$sprop, ResponsiveValue::$svariant(v.clone())));
                    }
                )+
                out
            }

            pub fn is_empty(&self) -> bool {
                *self == StateStyle::default()
            }
        }

        /// Patch de style (commande `set_style`, `NodeSpec.style`).
        ///
        /// Champ absent = inchangé ; `null` = propriété supprimée (tous breakpoints) ; objet = patch
        /// par breakpoint. Raccourcis développés par le moteur : `padding`, `padding_x`,
        /// `padding_y`, `margin_x`, `margin_y`, `border_width` (un champ explicite l'emporte).
        /// `scale` et `ring` ne sont valides qu'avec un état d'interaction.
        #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
        pub struct StylePatch {
            $(
                #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
                #[ts(optional)]
                pub $field: Option<Option<ResponsivePatch<$ty>>>,
            )+
            $(
                #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
                #[ts(optional)]
                pub $sfield: Option<Option<ResponsivePatch<$sty>>>,
            )+
            #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
            #[ts(optional)]
            pub padding: Option<Option<ResponsivePatch<Space>>>,
            #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
            #[ts(optional)]
            pub padding_x: Option<Option<ResponsivePatch<Space>>>,
            #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
            #[ts(optional)]
            pub padding_y: Option<Option<ResponsivePatch<Space>>>,
            #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
            #[ts(optional)]
            pub margin_x: Option<Option<ResponsivePatch<Margin>>>,
            #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
            #[ts(optional)]
            pub margin_y: Option<Option<ResponsivePatch<Margin>>>,
            #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
            #[ts(optional)]
            pub border_width: Option<Option<ResponsivePatch<BorderWidth>>>,
        }

        /// Patch d'un état d'interaction (`NodeSpec.hover`, variantes).
        #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
        pub struct StatePatch {
            $(
                #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
                #[ts(optional)]
                pub $stfield: Option<Option<ResponsivePatch<$stty>>>,
            )+
            $(
                #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "double_option::deserialize")]
                #[ts(optional)]
                pub $sfield: Option<Option<ResponsivePatch<$sty>>>,
            )+
        }

        impl StylePatch {
            /// Entrées explicites (hors raccourcis), dans l'ordre de déclaration.
            fn explicit_entries(&self) -> Vec<(StyleProp, PropChange)> {
                let mut out = Vec::new();
                $(
                    if let Some(change) = &self.$field {
                        out.push((StyleProp::$prop, PropChange::from_patch(change, ResponsiveValuePatch::$variant)));
                    }
                )+
                $(
                    if let Some(change) = &self.$sfield {
                        out.push((StyleProp::$sprop, PropChange::from_patch(change, ResponsiveValuePatch::$svariant)));
                    }
                )+
                out
            }

            /// Patch qui reproduit un style complet.
            pub fn from_style(style: &Style) -> StylePatch {
                StylePatch {
                    $($field: style.$field.as_ref().map(|v| Some(ResponsivePatch::from_responsive(v))),)+
                    ..StylePatch::default()
                }
            }
        }

        impl StatePatch {
            /// Entrées du patch d'état.
            pub fn entries(&self) -> Vec<(StyleProp, PropChange)> {
                let mut out = Vec::new();
                $(
                    if let Some(change) = &self.$stfield {
                        out.push((StyleProp::$stprop, PropChange::from_patch(change, ResponsiveValuePatch::$stvariant)));
                    }
                )+
                $(
                    if let Some(change) = &self.$sfield {
                        out.push((StyleProp::$sprop, PropChange::from_patch(change, ResponsiveValuePatch::$svariant)));
                    }
                )+
                out
            }

            pub fn is_empty(&self) -> bool {
                *self == StatePatch::default()
            }
        }
    };
}

style_props! {
    base {
        // conteneur
        direction: Direction => Direction / Direction, All, false;
        wrap: bool => Wrap / Bool, All, false;
        columns: GridColumns => Columns / GridColumns, All, false;
        gap: Space => Gap / Space, All, false;
        align: Align => Align / Align, All, false;
        justify: Justify => Justify / Justify, All, false;
        // enfant de layout
        grow: bool => Grow / Bool, All, false;
        shrink: bool => Shrink / Bool, All, false;
        align_self: AlignSelf => AlignSelf / AlignSelf, All, false;
        col_span: GridSpan => ColSpan / GridSpan, All, false;
        order: Order => Order / Order, All, false;
        // espacement
        padding_top: Space => PaddingTop / Space, All, false;
        padding_right: Space => PaddingRight / Space, All, false;
        padding_bottom: Space => PaddingBottom / Space, All, false;
        padding_left: Space => PaddingLeft / Space, All, false;
        margin_top: Margin => MarginTop / Margin, All, false;
        margin_right: Margin => MarginRight / Margin, All, false;
        margin_bottom: Margin => MarginBottom / Margin, All, false;
        margin_left: Margin => MarginLeft / Margin, All, false;
        // dimensions
        width: Size => Width / Size, All, false;
        min_width: Size => MinWidth / Size, All, false;
        max_width: Size => MaxWidth / Size, All, false;
        height: Size => Height / Size, All, false;
        min_height: Size => MinHeight / Size, All, false;
        max_height: Size => MaxHeight / Size, All, false;
        aspect_ratio: AspectRatio => AspectRatio / AspectRatio, All, false;
        // position
        position: Position => Position / Position, All, false;
        top: Inset => Top / Inset, All, false;
        right: Inset => Right / Inset, All, false;
        bottom: Inset => Bottom / Inset, All, false;
        left: Inset => Left / Inset, All, false;
        z_index: ZIndex => ZIndex / ZIndex, All, false;
        // typographie
        font_family: TokenName => FontFamily / Token, All, true;
        font_size: FontSize => FontSize / FontSize, All, true;
        font_weight: FontWeight => FontWeight / FontWeight, All, true;
        line_height: LineHeight => LineHeight / LineHeight, All, true;
        letter_spacing: LetterSpacing => LetterSpacing / LetterSpacing, All, true;
        text_align: TextAlign => TextAlign / TextAlign, All, true;
        text_color: ColorRef => TextColor / Color, All, true;
        text_transform: TextTransform => TextTransform / TextTransform, All, true;
        text_decoration: TextDecoration => TextDecoration / TextDecoration, All, false;
        text_wrap: TextWrap => TextWrap / TextWrap, All, true;
        // visuel
        background: Background => Background / Background, All, false;
        border_top_width: BorderWidth => BorderTopWidth / BorderWidth, All, false;
        border_right_width: BorderWidth => BorderRightWidth / BorderWidth, All, false;
        border_bottom_width: BorderWidth => BorderBottomWidth / BorderWidth, All, false;
        border_left_width: BorderWidth => BorderLeftWidth / BorderWidth, All, false;
        border_color: ColorRef => BorderColor / Color, All, false;
        border_style: BorderStyle => BorderStyle / BorderStyle, All, false;
        radius: Radius => Radius / Radius, All, false;
        shadow: Shadow => Shadow / Shadow, All, false;
        opacity: Opacity => Opacity / Opacity, All, false;
        overflow: Overflow => Overflow / Overflow, All, false;
        object_fit: ObjectFit => ObjectFit / ObjectFit, All, false;
        // mouvement
        transition: Transition => Transition / Transition, WebOnly, false;
        duration: Duration => Duration / Duration, WebOnly, false;
    }
    state_only {
        scale: Scale => Scale / Scale;
        ring: Ring => Ring / Ring;
    }
    state {
        text_color: ColorRef => TextColor / Color;
        background: Background => Background / Background;
        border_color: ColorRef => BorderColor / Color;
        opacity: Opacity => Opacity / Opacity;
        shadow: Shadow => Shadow / Shadow;
        text_decoration: TextDecoration => TextDecoration / TextDecoration;
    }
}

/// Changement d'une propriété dans un patch.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PropChange {
    /// Propriété supprimée (tous breakpoints).
    Remove,
    /// Patch par breakpoint.
    Merge(ResponsiveValuePatch),
}

impl PropChange {
    fn from_patch<T: Clone>(
        change: &Option<ResponsivePatch<T>>,
        wrap: impl FnOnce(ResponsivePatch<T>) -> ResponsiveValuePatch,
    ) -> PropChange {
        match change {
            None => PropChange::Remove,
            Some(patch) => PropChange::Merge(wrap(patch.clone())),
        }
    }

    /// Breakpoints touchés (`None` : tous, suppression de la propriété).
    pub fn touched(&self) -> Option<Vec<Breakpoint>> {
        match self {
            PropChange::Remove => None,
            PropChange::Merge(patch) => Some(patch.touched()),
        }
    }
}

/// Développe un raccourci en patchs par côté (un champ explicite l'emporte sur le raccourci).
fn expand_shorthand<T: Clone>(
    out: &mut Vec<(StyleProp, PropChange)>,
    shorthand: &Option<Option<ResponsivePatch<T>>>,
    props: &[StyleProp],
    wrap: impl Fn(ResponsivePatch<T>) -> ResponsiveValuePatch,
) {
    let Some(change) = shorthand else { return };
    for prop in props {
        if out.iter().all(|(p, _)| p != prop) {
            out.push((*prop, PropChange::from_patch(change, &wrap)));
        }
    }
}

impl StylePatch {
    /// Entrées du patch, raccourcis développés.
    pub fn entries(&self) -> Vec<(StyleProp, PropChange)> {
        use StyleProp as P;
        let mut out = self.explicit_entries();
        expand_shorthand(
            &mut out,
            &self.padding_x,
            &[P::PaddingLeft, P::PaddingRight],
            ResponsiveValuePatch::Space,
        );
        expand_shorthand(
            &mut out,
            &self.padding_y,
            &[P::PaddingTop, P::PaddingBottom],
            ResponsiveValuePatch::Space,
        );
        expand_shorthand(
            &mut out,
            &self.padding,
            &[P::PaddingTop, P::PaddingRight, P::PaddingBottom, P::PaddingLeft],
            ResponsiveValuePatch::Space,
        );
        expand_shorthand(
            &mut out,
            &self.margin_x,
            &[P::MarginLeft, P::MarginRight],
            ResponsiveValuePatch::Margin,
        );
        expand_shorthand(
            &mut out,
            &self.margin_y,
            &[P::MarginTop, P::MarginBottom],
            ResponsiveValuePatch::Margin,
        );
        expand_shorthand(
            &mut out,
            &self.border_width,
            &[
                P::BorderTopWidth,
                P::BorderRightWidth,
                P::BorderBottomWidth,
                P::BorderLeftWidth,
            ],
            ResponsiveValuePatch::BorderWidth,
        );
        out
    }

    pub fn is_empty(&self) -> bool {
        *self == StylePatch::default()
    }
}

/// Styles des trois états d'interaction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct StateStyles {
    #[serde(default, skip_serializing_if = "StateStyle::is_empty")]
    #[ts(as = "Option<StateStyle>", optional)]
    pub hover: StateStyle,
    #[serde(default, skip_serializing_if = "StateStyle::is_empty")]
    #[ts(as = "Option<StateStyle>", optional)]
    pub focus_visible: StateStyle,
    #[serde(default, skip_serializing_if = "StateStyle::is_empty")]
    #[ts(as = "Option<StateStyle>", optional)]
    pub active: StateStyle,
}

impl StateStyles {
    pub fn get(&self, state: InteractionState) -> &StateStyle {
        match state {
            InteractionState::Hover => &self.hover,
            InteractionState::FocusVisible => &self.focus_visible,
            InteractionState::Active => &self.active,
        }
    }

    pub fn get_mut(&mut self, state: InteractionState) -> &mut StateStyle {
        match state {
            InteractionState::Hover => &mut self.hover,
            InteractionState::FocusVisible => &mut self.focus_visible,
            InteractionState::Active => &mut self.active,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.hover.is_empty() && self.focus_visible.is_empty() && self.active.is_empty()
    }
}

/// Patch des trois états (surcharges de variante).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
pub struct StateStylesPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hover: Option<StatePatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub focus_visible: Option<StatePatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub active: Option<StatePatch>,
}

impl StateStylesPatch {
    pub fn get(&self, state: InteractionState) -> Option<&StatePatch> {
        match state {
            InteractionState::Hover => self.hover.as_ref(),
            InteractionState::FocusVisible => self.focus_visible.as_ref(),
            InteractionState::Active => self.active.as_ref(),
        }
    }

    pub fn is_empty(&self) -> bool {
        InteractionState::ALL
            .iter()
            .all(|s| self.get(*s).is_none_or(StatePatch::is_empty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_access_round_trips() {
        let mut style = Style::default();
        let value = ResponsiveValue::Space(Responsive::new(Space::S4));
        assert_eq!(style.set(StyleProp::Gap, Some(value.clone())).unwrap(), None);
        assert_eq!(style.get(StyleProp::Gap).unwrap(), Some(value.clone()));
        assert!(
            style
                .set(StyleProp::Gap, Some(ResponsiveValue::Bool(Responsive::new(true))))
                .is_err()
        );
        assert!(style.get(StyleProp::Ring).is_err());
        assert_eq!(style.set(StyleProp::Gap, None).unwrap(), Some(value));
        assert!(style.is_empty());
    }

    #[test]
    fn state_style_accepts_only_state_props() {
        let mut state = StateStyle::default();
        let scale = ResponsiveValue::Scale(Responsive::new(Scale::S105));
        state.set(StyleProp::Scale, Some(scale.clone())).unwrap();
        assert_eq!(state.get(StyleProp::Scale).unwrap(), Some(scale));
        assert!(state.set(StyleProp::Gap, None).is_err());
        assert!(StyleProp::TextColor.in_states());
        assert!(!StyleProp::Gap.in_states());
        assert!(!StyleProp::Ring.in_base());
    }

    #[test]
    fn shorthands_expand_and_explicit_fields_win() {
        let patch: StylePatch = serde_json::from_str(
            r#"{ "padding_x": { "base": "4" }, "padding_left": { "base": "2" }, "border_width": null }"#,
        )
        .unwrap();
        let entries = patch.entries();
        let find = |prop| entries.iter().find(|(p, _)| *p == prop).map(|(_, c)| c.clone());
        assert_eq!(
            find(StyleProp::PaddingLeft),
            Some(PropChange::Merge(ResponsiveValuePatch::Space(ResponsivePatch::at(
                Breakpoint::Base,
                Space::S2
            ))))
        );
        assert_eq!(
            find(StyleProp::PaddingRight),
            Some(PropChange::Merge(ResponsiveValuePatch::Space(ResponsivePatch::at(
                Breakpoint::Base,
                Space::S4
            ))))
        );
        assert_eq!(find(StyleProp::BorderTopWidth), Some(PropChange::Remove));
        assert_eq!(find(StyleProp::PaddingTop), None);
    }

    #[test]
    fn style_serializes_compactly() {
        let style = Style {
            gap: Some(Responsive {
                md: Some(Space::S8),
                ..Responsive::new(Space::S4)
            }),
            ..Style::default()
        };
        assert_eq!(
            serde_json::to_string(&style).unwrap(),
            r#"{"gap":{"base":"4","md":"8"}}"#
        );
    }
}
