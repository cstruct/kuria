//! Kuria's answers as the client reads them, and the GraphQL that asks for
//! them.

use serde::Deserialize;
use serde_json::Value as Json;

/// An akt type's shape: its fields, states, transitions and presentation.
pub const SHAPE: &str = "fragment Shape on AktType {
  key version name numberPattern
  fields { key name kind description required multiline multiple of assignees references options { key name } format }
  states { key name initial final tone }
  transitions { key name from to requires outcomes }
  presentation { title card list icon colour }
}";

/// A user or contact, as values name them.
pub const SOMEONE: &str = "fragment Someone on Someone {
  __typename
  ... on User { id name email }
  ... on Contact { id displayName name email }
}";

/// Field values, by kind.
pub const VALUES: &str = "fragment Values on FieldValue {
  __typename
  ... on TextValue { field text }
  ... on ChoiceValue { field option { key name } }
  ... on ChoicesValue { field options { key name } }
  ... on DateValue { field date }
  ... on UserValue { field user { ...Someone } }
  ... on CheckboxValue { field at by { id name email } for { ...Someone } }
  ... on ReferenceValue { field url label }
  ... on GitValue { field repository sha tag }
  ... on DockerValue { field repository digest tag }
  ... on AttachmentValue { field attachment { id name mediaType size sha256 url } }
  ... on UsersValue { field users { ...Someone } }
  ... on VerdictValue { field verdicts { assignee { ...Someone } verdict by { id name email } at } }
  ... on SectionsValue { field value }
  ... on MarkdownValue { field markdown references { destination broken } }
}";

/// An akt as lists show it.
pub const ROW: &str = "fragment Row on Akt {
  number title updatedAt outcome
  type { key name }
  state { key name tone }
  fields { ...Values }
}";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shape {
    pub key: String,
    pub version: i64,
    pub name: String,
    pub number_pattern: String,
    pub fields: Vec<FieldDef>,
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
    pub presentation: Presentation,
    /// The document the version was defined by, for what the shape
    /// doesn't say: what it contains and links to, and its board.
    #[serde(default)]
    pub document: Json,
}

impl Shape {
    #[must_use]
    pub fn field(&self, key: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.key == key)
    }

    #[must_use]
    pub fn state(&self, key: &str) -> Option<&State> {
        self.states.iter().find(|s| s.key == key)
    }

    /// A field's name, else its key.
    #[must_use]
    pub fn field_name<'a>(&'a self, key: &'a str) -> &'a str {
        self.field(key).map_or(key, |f| f.name.as_str())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct FieldDef {
    pub key: String,
    pub name: String,
    /// `TEXT`, `CHOICE`, …, as GraphQL names it.
    pub kind: String,
    pub description: Option<String>,
    pub required: bool,
    pub multiline: bool,
    pub options: Vec<Opt>,
    pub format: Option<String>,
    /// For user fields: whether it names any number of people.
    #[serde(default)]
    pub multiple: bool,
    /// For sections fields: the akt types of the members it places.
    #[serde(default)]
    pub of: Vec<String>,
    /// For verdict fields: the user field naming the assignees.
    #[serde(default)]
    pub assignees: Option<String>,
    /// For markdown fields: the link kind its references are kept as.
    #[serde(default)]
    pub references: Option<String>,
}

impl FieldDef {
    /// Whether its value is a section of an akt file: a multiline text, a
    /// `markdown` field or a `sections` field (`cli.file.kinds`).
    #[must_use]
    pub fn sectioned(&self) -> bool {
        (self.kind == "TEXT" && self.multiline)
            || self.kind == "MARKDOWN"
            || self.kind == "SECTIONS"
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Opt {
    pub key: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct State {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub initial: bool,
    #[serde(default, rename = "final")]
    pub is_final: bool,
    #[serde(default)]
    pub tone: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Transition {
    pub key: String,
    pub name: String,
    pub from: Vec<String>,
    pub to: String,
    pub requires: Vec<String>,
    /// As GraphQL names them: `PASS`, …
    #[serde(default)]
    pub outcomes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Presentation {
    pub title: Option<String>,
    pub card: Vec<String>,
    pub list: Vec<String>,
    pub icon: Option<String>,
    pub colour: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "__typename", rename_all_fields = "camelCase")]
pub enum Someone {
    User {
        id: String,
        name: String,
        email: Option<String>,
    },
    Contact {
        id: String,
        display_name: String,
        name: Option<String>,
        email: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub media_type: String,
    pub size: i64,
    pub sha256: String,
    pub url: String,
}

/// A field's value, by its kind.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "__typename")]
pub enum Value {
    TextValue {
        field: String,
        text: String,
    },
    ChoiceValue {
        field: String,
        option: Opt,
    },
    ChoicesValue {
        field: String,
        options: Vec<Opt>,
    },
    DateValue {
        field: String,
        date: String,
    },
    UserValue {
        field: String,
        user: Someone,
    },
    CheckboxValue {
        field: String,
        at: String,
        by: User,
        #[serde(rename = "for")]
        checked_for: Someone,
    },
    ReferenceValue {
        field: String,
        url: String,
        label: Option<String>,
    },
    GitValue {
        field: String,
        repository: String,
        sha: Option<String>,
        tag: Option<String>,
    },
    DockerValue {
        field: String,
        repository: String,
        digest: String,
        tag: Option<String>,
    },
    AttachmentValue {
        field: String,
        attachment: Attachment,
    },
    UsersValue {
        field: String,
        users: Vec<Someone>,
    },
    VerdictValue {
        field: String,
        verdicts: Vec<GivenVerdict>,
    },
    SectionsValue {
        field: String,
        /// As `editAkt` takes it: akt numbers and `{title, text,
        /// collapsible, items}`.
        value: Json,
    },
    MarkdownValue {
        field: String,
        markdown: String,
        #[serde(default)]
        references: Vec<MarkdownReference>,
    },
}

/// A reference in a `markdown` field, and why it is broken, if it is.
#[derive(Clone, Debug, Deserialize)]
pub struct MarkdownReference {
    pub destination: String,
    pub broken: Option<String>,
}

/// One assignee's verdict.
#[derive(Clone, Debug, Deserialize)]
pub struct GivenVerdict {
    pub assignee: Someone,
    /// `PASS`, `FAIL` or `SKIP`.
    pub verdict: String,
    pub by: User,
    pub at: String,
}

impl Value {
    /// The key of the field it is the value of.
    #[must_use]
    pub fn field(&self) -> &str {
        match self {
            Self::TextValue { field, .. }
            | Self::ChoiceValue { field, .. }
            | Self::ChoicesValue { field, .. }
            | Self::DateValue { field, .. }
            | Self::UserValue { field, .. }
            | Self::CheckboxValue { field, .. }
            | Self::ReferenceValue { field, .. }
            | Self::GitValue { field, .. }
            | Self::DockerValue { field, .. }
            | Self::AttachmentValue { field, .. }
            | Self::UsersValue { field, .. }
            | Self::VerdictValue { field, .. }
            | Self::SectionsValue { field, .. }
            | Self::MarkdownValue { field, .. } => field,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TypeName {
    pub key: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StateName {
    pub key: Option<String>,
    pub name: String,
}

/// An akt as lists show it.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub number: String,
    pub title: Option<String>,
    pub updated_at: String,
    #[serde(rename = "type")]
    pub akt_type: TypeName,
    pub state: StateName,
    pub fields: Vec<Value>,
    /// `PASS`, …, if its type has outcomes.
    #[serde(default)]
    pub outcome: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub nodes: Vec<Row>,
    pub page_info: PageInfo,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
}

/// Reads a GraphQL answer's part into `T`.
pub fn read<T: serde::de::DeserializeOwned>(
    value: Json,
    what: &str,
) -> Result<T, crate::error::CliError> {
    serde_json::from_value(value).map_err(|e| {
        crate::error::CliError::unavailable(format!(
            "Kuria's answer for {what} wasn't as expected: {e}"
        ))
    })
}
