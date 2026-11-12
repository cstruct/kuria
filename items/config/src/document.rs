//! The akt type document, as submitted.

use serde::{Deserialize, Serialize};

/// An akt type, as a tenant writes it:
///
/// ```json
/// {
///   "key": "capa",
///   "name": "CAPA",
///   "number": "CAPA-{year}-{seq:4}",
///   "fields": [
///     { "key": "title", "name": "Title", "kind": "text", "required": true },
///     { "key": "severity", "name": "Severity", "kind": "choice",
///       "options": [{ "key": "major", "name": "Major" }] }
///   ],
///   "states": [
///     { "key": "open", "name": "Open", "initial": true },
///     { "key": "closed", "name": "Closed", "final": true }
///   ],
///   "transitions": [
///     { "key": "close", "name": "Close", "from": ["open"], "to": "closed",
///       "requires": ["severity"] }
///   ]
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AktTypeDocument {
    pub key: String,
    pub name: String,
    /// The number pattern, such as `CAPA-{year}-{seq:4}`.
    pub number: String,
    #[serde(default)]
    pub fields: Vec<FieldDef>,
    pub states: Vec<StateDef>,
    #[serde(default)]
    pub transitions: Vec<TransitionDef>,
}

/// One field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDef {
    pub key: String,
    pub name: String,
    pub kind: FieldKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Holds a value from creation on, and can't be cleared.
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
    /// For `text`: whether line breaks are allowed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub multiline: bool,
    /// For `choice` and `choices`: the options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ChoiceOption>,
}

/// What values a field takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Choice,
    Choices,
    Date,
    Subject,
    Reference,
}

impl FieldKind {
    /// Whether fields of this kind take their values from options.
    #[must_use]
    pub fn has_options(self) -> bool {
        matches!(self, Self::Choice | Self::Choices)
    }
}

/// One option of a `choice` or `choices` field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    pub key: String,
    pub name: String,
}

/// One state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateDef {
    pub key: String,
    pub name: String,
    /// The state akter are created in. Exactly one state is initial.
    #[serde(default, skip_serializing_if = "is_false")]
    pub initial: bool,
    /// An akt in a final state can't be edited or moved on.
    #[serde(default, rename = "final", skip_serializing_if = "is_false")]
    pub is_final: bool,
}

/// One transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionDef {
    pub key: String,
    pub name: String,
    /// The states it may be performed from.
    pub from: Vec<String>,
    /// The state it moves the akt to.
    pub to: String,
    /// Fields that must hold a value once it has been performed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    /// A Konklav action performing it requires, besides
    /// `kuria:akter:transition`, such as `capa:approve_closure`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
fn is_false(value: &bool) -> bool {
    !*value
}
