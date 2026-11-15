//! Akt type documents: what a tenant submits to define an akt type, and its
//! validation into an [`AktType`] the engine can use.
//!
//! No I/O: documents come in as values and leave as values, so every rule
//! here is tested without a database.

mod document;
mod draft;
mod number;
mod references;
mod validate;

pub use document::{
    AktTypeDocument, AttachmentFormat, BoardDef, ChoiceOption, Colour, ColumnDef, ContainsDef,
    FieldDef, FieldKind, Icon, LinkDef, MAX_CARD_FIELDS, Navigation, PresentationDef, StateDef,
    Tone, TransitionDef, VersionsDef,
};
pub use draft::{DraftDef, GroupBy, LineTemplate, LineValues, MAX_DEPTH};
pub use number::{NumberContext, NumberPattern, PatternError};
pub use references::{StateReference, check_field_references, check_state_references};
pub use validate::{AktType, Problem, validate};
