//! Style abstrait : valeurs, couleurs, responsive, propriétés.

pub mod color;
pub mod responsive;
#[allow(clippy::module_inception)]
pub mod style;
pub mod values;

pub use color::{ColorRef, ColorSource, Hex, Hue, Rgba, Shade};
pub use responsive::{Breakpoint, Responsive, ResponsivePatch};
pub use style::{
    Background, InteractionState, PropChange, ResponsiveValue, ResponsiveValuePatch, Ring, StatePatch, StateStyle,
    StateStyles, StateStylesPatch, Style, StyleError, StylePatch, StyleProp,
};
pub use values::*;
