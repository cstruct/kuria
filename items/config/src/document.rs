//! The akt type document, as submitted.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::draft::DraftDef;

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
    /// The akt types whose akter this type's akter may contain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contains: Vec<ContainsDef>,
    /// The akt types an akt of this type must be created in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub created_in: Vec<String>,
    /// How this type's akter are shown as boards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<BoardDef>,
    /// How this type's akter are shown: title, cards, lists and tones.
    #[serde(default, skip_serializing_if = "PresentationDef::is_empty")]
    pub presentation: PresentationDef,
}

/// How an akt type's akter are shown:
///
/// ```json
/// { "title": "title", "card": ["kind", "assignee"], "list": ["kind"],
///   "tones": { "rejected": "danger", "accepted": "success" } }
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationDef {
    /// The single-line text field shown as each akt's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The fields shown on board cards, after the title; at most four.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card: Vec<String>,
    /// The fields shown as columns where the type's akter are listed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<String>,
    /// Per state key, how it is shown; `neutral` for states not named.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tones: BTreeMap<String, Tone>,
}

impl PresentationDef {
    fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// How strongly, and in which sense, a state is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Neutral,
    Active,
    Success,
    Warning,
    Danger,
}

/// The most fields a board card shows after the title.
pub const MAX_CARD_FIELDS: usize = 4;

/// One akt type this type's akter may contain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainsDef {
    /// The contained akt type's key.
    #[serde(rename = "type")]
    pub akt_type: String,
    /// An akt of that type may be in at most one container of this type.
    #[serde(default, skip_serializing_if = "is_false")]
    pub exclusive: bool,
    /// The states of this type in which it accepts new members of that
    /// type; any state if empty.
    #[serde(default, rename = "while", skip_serializing_if = "Vec::is_empty")]
    pub accepting: Vec<String>,
}

impl ContainsDef {
    /// Whether a container in `state` accepts new members of this type.
    // r[impl container.while]
    #[must_use]
    pub fn accepts_in(&self, state: &str) -> bool {
        self.accepting.is_empty() || self.accepting.iter().any(|s| s == state)
    }
}

/// A container's board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardDef {
    pub columns: Vec<ColumnDef>,
}

/// One column of a board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnDef {
    pub key: String,
    pub name: String,
    /// Per contained akt type, by key, the states that fall in this column.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub states: BTreeMap<String, Vec<String>>,
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
    /// For multiline `text`: how it is drafted from the akter around it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<DraftDef>,
    /// For `attachment`: what files it holds, any file by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<AttachmentFormat>,
}

/// What files an `attachment` field holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentFormat {
    /// Any file.
    Any,
    /// Only a file Kuria recognised as an SBOM.
    Sbom,
}

/// What values a field takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Choice,
    Choices,
    Date,
    User,
    Checkbox,
    Reference,
    Git,
    Docker,
    Attachment,
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
