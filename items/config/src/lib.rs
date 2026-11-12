//! Akt type documents: what a tenant submits to define an akt type, and its
//! validation into an [`AktType`] the engine can use.
//!
//! No I/O: documents come in as values and leave as values, so every rule
//! here is tested without a database.

mod document;
mod number;
mod validate;

pub use document::{AktTypeDocument, ChoiceOption, FieldDef, FieldKind, StateDef, TransitionDef};
pub use number::{NumberPattern, PatternError};
pub use validate::{AktType, Problem, validate};
