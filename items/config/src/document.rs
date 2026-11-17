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
    /// The sequence, by key, that akt types numbered by the same pattern
    /// share (`akt.number.shared`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<String>,
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
    /// The kinds of link this type's akter may have to other akter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<LinkDef>,
    /// Whether this type's akter release versions, and what a release
    /// needs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<VersionsDef>,
    /// Whether this type's akter have a live draft, edited together, that
    /// only releases make revisions of (`live.declare`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub live_drafts: bool,
    /// The rules by which this type's akter archive themselves
    /// (`archive.rules`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<ArchiveDef>,
}

/// How an akt type's akter are archived (`archive.rules`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveDef {
    #[serde(default)]
    pub rules: Vec<ArchiveRuleDef>,
}

/// One archive rule, as written: exactly one of `newer`, `with_container`
/// and `days_in`, with the members its kind takes.
///
/// ```json
/// { "newer": 2, "in": ["released", "closed"] }
/// { "with_container": "release" }
/// { "days_in": 90, "in": ["done"] }
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveRuleDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newer: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with_container: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_in: Option<u32>,
    #[serde(default, rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    /// For `newer`: only akter sharing a container of this type count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<String>,
}

/// An archive rule, by its kind (`archive.rules`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveRule<'a> {
    /// Archive once at least `count` other akter of the type reached one
    /// of `states` after it did, sharing a container of the type `within`
    /// if given.
    Newer {
        count: u32,
        states: &'a [String],
        within: Option<&'a str>,
    },
    /// Archive with a container of this type that holds it.
    WithContainer(&'a str),
    /// Archive after `days` days in one of `states`.
    DaysIn { days: u32, states: &'a [String] },
}

impl ArchiveRule<'_> {
    /// The rule's kind, as an archive by it names it.
    #[must_use]
    pub fn kind(self) -> &'static str {
        match self {
            Self::Newer { .. } => "newer",
            Self::WithContainer(_) => "with_container",
            Self::DaysIn { .. } => "days_in",
        }
    }
}

impl ArchiveRuleDef {
    /// The rule, if it is exactly one kind with what that kind takes.
    #[must_use]
    pub fn rule(&self) -> Option<ArchiveRule<'_>> {
        match (self.newer, &self.with_container, self.days_in) {
            (Some(count), None, None) => Some(ArchiveRule::Newer {
                count,
                states: &self.states,
                within: self.within.as_deref(),
            }),
            (None, Some(container), None) if self.states.is_empty() && self.within.is_none() => {
                Some(ArchiveRule::WithContainer(container))
            }
            (None, None, Some(days)) if self.within.is_none() => Some(ArchiveRule::DaysIn {
                days,
                states: &self.states,
            }),
            _ => None,
        }
    }
}

/// That an akt type releases versions (`version.declare`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionsDef {
    /// A Konklav action a release requires, besides `kuria:akter:edit`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// The `attachment` field each version is released with a PDF in, for
    /// akter written elsewhere, such as external documents
    /// (`version.file`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// A kind of link an akt type's akter may have:
///
/// ```json
/// { "key": "blocks", "name": "Blocks", "inverse": "Blocked by", "to": ["task"] }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkDef {
    pub key: String,
    /// The link as its source reads it.
    pub name: String,
    /// The link as its target reads it.
    pub inverse: String,
    /// The akt types its targets may have, by key.
    pub to: Vec<String>,
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
    /// The icon shown with the type's name; none if not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
    /// The colour the type is shown in; `neutral` if not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Colour>,
    /// Where the sidebar lists the type; `main` if not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<Navigation>,
    /// The `attachment` field whose value is the akt's picture, making
    /// its akter images (`akt.field.markdown.images`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Whether references to its akter are shown with the akt's number
    /// first (`app.references`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub numbered_references: bool,
}

/// The media types an image's picture may have.
pub const IMAGE_MEDIA_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/svg+xml", "image/webp"];

/// Where the app's sidebar lists an akt type (`app.navigation`).
// r[impl akt.type.presentation]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Navigation {
    /// With the others.
    #[default]
    Main,
    /// Under "Show more".
    More,
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

/// The icons an akt type may be shown with, by their names in documents.
// r[impl akt.type.presentation]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Icon {
    Archive,
    BookOpen,
    Box,
    Bug,
    Calendar,
    CircleCheck,
    ClipboardCheck,
    FileText,
    Flag,
    FlaskConical,
    GitBranch,
    Layers,
    ListChecks,
    Milestone,
    NotebookPen,
    Package,
    Rocket,
    ShieldCheck,
    Tag,
    Target,
    TriangleAlert,
    Users,
    Wrench,
    Zap,
}

/// The colours an akt type may be shown in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Colour {
    #[default]
    Neutral,
    Blue,
    Green,
    Amber,
    Red,
    Purple,
    Teal,
    Pink,
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a member of the field's document, as its author writes it"
)]
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
    /// For `attachment`: the media types it takes, any if empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media_types: Vec<String>,
    /// For `user`: whether it holds a list of users and contacts.
    #[serde(default, skip_serializing_if = "is_false")]
    pub multiple: bool,
    /// For `sections`: the akt types of the members it places; any of the
    /// types the akt type contains if empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub of: Vec<String>,
    /// For `verdict`: the key of the `multiple` `user` field naming who
    /// gives verdicts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignees: Option<String>,
    /// For `markdown`: the key of the link kind its references are kept
    /// as, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<String>,
    /// For `markdown`: whether its `diagram` code blocks are diagrams,
    /// rendered when it is written (`akt.field.markdown.diagrams`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub diagrams: bool,
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
    /// The akt's members laid out in nested sections.
    Sections,
    /// Each assignee's verdict.
    Verdict,
    /// Markdown whose headings keep ids and whose links may name akter.
    Markdown,
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
    /// The outcomes the akt must have for it to be performed (`pass`,
    /// `fail`, `skip` or `pending`); any, or none, if empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<String>,
    /// A Konklav action performing it requires, besides
    /// `kuria:akter:transition`, such as `capa:approve_closure`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
fn is_false(value: &bool) -> bool {
    !*value
}
