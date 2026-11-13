//! The akt file (`cli.file`): Markdown with YAML front matter, as `cat`
//! writes it and `apply` reads it.
//!
//! ```markdown
//! ---
//! number: ITEM-0001
//! type: backlog_item
//! revision: 3
//! state: in_progress
//! in: [BACKLOG-1]
//! links: ["Blocks ITEM-0002"]
//! title: Export and import
//! assignee: dev@shop.example
//! ---
//!
//! <!-- field: description -->
//! What to build…
//! ```

use serde::Deserialize;
use serde_json::{Map, Value as Json, json};

use crate::model::{Shape, State, StateName, TypeName, Value};
use crate::values::file_value;

/// The keys every file has before its fields, which `apply` doesn't edit.
pub const READ_ONLY: [&str; 6] = ["number", "type", "revision", "state", "in", "links"];

/// An akt with everything its file and `show` say.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Akt {
    pub number: String,
    pub title: Option<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(rename = "type")]
    pub shape: Shape,
    pub state: State,
    pub fields: Vec<Value>,
    pub containers: Vec<Other>,
    pub links: Vec<Link>,
}

/// Another akt, as an akt names it.
#[derive(Clone, Debug, Deserialize)]
pub struct Other {
    pub number: String,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub akt_type: TypeName,
    pub state: StateName,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub kind: String,
    pub name: String,
    pub outgoing: bool,
    pub pinned_revision: Option<i64>,
    pub akt: Other,
    pub pinned: Option<Pinned>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Pinned {
    pub revision: i64,
    pub title: Option<String>,
    pub state: StateName,
}

/// What to ask for an akt's file: its fields and everything around them.
pub const AKT: &str = "number title revision createdAt updatedAt
  type { ...Shape document }
  state { key name initial final tone }
  fields { ...Values }
  containers { number title type { key name } state { key name } }
  links {
    kind name outgoing pinnedRevision
    akt { number title type { key name } state { key name } }
    pinned { revision title state { key name } }
  }";

impl Link {
    /// The link as the file's `links` says it: "Blocks ITEM-0002".
    #[must_use]
    pub fn said(&self) -> String {
        match self.pinned_revision {
            Some(revision) => format!("{} {}@{revision}", self.name, self.akt.number),
            None => format!("{} {}", self.name, self.akt.number),
        }
    }
}

/// Whether `value` goes in a section of its own: a multiline text field's.
fn sectioned(shape: &Shape, value: &Value) -> bool {
    matches!(value, Value::TextValue { .. })
        && shape.field(value.field()).is_some_and(|f| f.multiline)
}

/// The akt as its file.
// r[impl cli.file]
#[must_use]
pub fn write(akt: &Akt) -> String {
    let mut front = vec![
        ("number", Json::from(akt.number.clone())),
        ("type", akt.shape.key.clone().into()),
        ("revision", akt.revision.into()),
        ("state", akt.state.key.clone().into()),
    ];
    if !akt.containers.is_empty() {
        front.push((
            "in",
            akt.containers.iter().map(|c| c.number.clone()).collect(),
        ));
    }
    if !akt.links.is_empty() {
        front.push(("links", akt.links.iter().map(Link::said).collect()));
    }
    let mut text = String::from("---\n");
    for (key, value) in front {
        text.push_str(&format!("{key}: {}\n", yaml(&value)));
    }
    for value in akt.fields.iter().filter(|v| !sectioned(&akt.shape, v)) {
        text.push_str(&format!(
            "{}: {}\n",
            value.field(),
            yaml(&file_value(value))
        ));
    }
    text.push_str("---\n");
    for value in akt.fields.iter().filter(|v| sectioned(&akt.shape, v)) {
        if let Value::TextValue { field, text: body } = value {
            text.push_str(&format!(
                "\n<!-- field: {field} -->\n{}\n",
                body.trim_end_matches('\n')
            ));
        }
    }
    text
}

/// The akt as `cat --format json` answers it: the file's keys, with links
/// and fields as objects.
#[must_use]
pub fn json(akt: &Akt) -> Json {
    let fields: Map<String, Json> = akt
        .fields
        .iter()
        .map(|v| (v.field().to_owned(), file_value(v)))
        .collect();
    json!({
        "number": akt.number,
        "type": akt.shape.key,
        "revision": akt.revision,
        "state": akt.state.key,
        "in": akt.containers.iter().map(|c| c.number.clone()).collect::<Vec<_>>(),
        "links": akt.links.iter().map(|l| json!({
            "kind": l.kind,
            "name": l.name,
            "outgoing": l.outgoing,
            "number": l.akt.number,
            "pinnedRevision": l.pinned_revision,
        })).collect::<Vec<_>>(),
        "fields": fields,
    })
}

/// `value` as YAML: plain scalars where they read the same, JSON's quoting
/// where they wouldn't, and flow style for lists and objects.
#[must_use]
pub fn yaml(value: &Json) -> String {
    match value {
        Json::String(text) if plain(text) => text.clone(),
        Json::Array(items) => format!(
            "[{}]",
            items.iter().map(yaml).collect::<Vec<_>>().join(", ")
        ),
        Json::Object(object) => format!(
            "{{{}}}",
            object
                .iter()
                .map(|(k, v)| format!("{}: {}", yaml(&Json::String(k.clone())), yaml(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.to_string(),
    }
}

/// Whether `text` reads back as itself, a string, unquoted, in block and
/// flow context alike.
fn plain(text: &str) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    let words = ["true", "false", "null", "yes", "no", "on", "off", "y", "n"];
    let starts =
        first.is_ascii_alphabetic() || (first.is_ascii_digit() && crate::values::is_date(text));
    starts
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " ._/+@-".contains(c))
        && !text.ends_with(' ')
        && !words.iter().any(|w| w.eq_ignore_ascii_case(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn akt() -> Akt {
        serde_json::from_value(json!({
            "number": "ITEM-0001", "title": "Export: and import", "revision": 3,
            "createdAt": "2026-11-13T14:00:00Z", "updatedAt": "2026-11-13T15:00:00Z",
            "type": {
                "key": "backlog_item", "version": 1, "name": "Backlog item", "numberPattern": "ITEM-{seq:4}",
                "fields": [
                    { "key": "title", "name": "Title", "kind": "TEXT", "description": null, "required": true, "multiline": false, "options": [], "format": null },
                    { "key": "description", "name": "Description", "kind": "TEXT", "description": null, "required": false, "multiline": true, "options": [], "format": null },
                    { "key": "kind", "name": "Kind", "kind": "CHOICE", "description": null, "required": false, "multiline": false, "options": [{ "key": "fix", "name": "Fix" }], "format": null },
                    { "key": "change", "name": "Pull request", "kind": "REFERENCE", "description": null, "required": false, "multiline": false, "options": [], "format": null }
                ],
                "states": [{ "key": "open", "name": "Open", "initial": true, "final": false, "tone": "NEUTRAL" }],
                "transitions": [],
                "presentation": { "title": "title", "card": [], "list": [], "icon": null, "colour": "NEUTRAL" }
            },
            "state": { "key": "open", "name": "Open", "initial": true, "final": false, "tone": "NEUTRAL" },
            "fields": [
                { "__typename": "TextValue", "field": "title", "text": "Export: and import" },
                { "__typename": "TextValue", "field": "description", "text": "Line one\n\nLine two\n" },
                { "__typename": "ChoiceValue", "field": "kind", "option": { "key": "fix", "name": "Fix" } },
                { "__typename": "ReferenceValue", "field": "change", "url": "https://git.example/pr/1", "label": null }
            ],
            "containers": [{ "number": "BACKLOG-1", "title": "Backlog", "type": { "key": "backlog", "name": "Backlog" }, "state": { "key": "open", "name": "Open" } }],
            "links": [{
                "kind": "blocks", "name": "Blocks", "outgoing": true, "pinnedRevision": null, "pinned": null,
                "akt": { "number": "ITEM-0002", "title": null, "type": { "key": "backlog_item", "name": "Backlog item" }, "state": { "key": "open", "name": "Open" } }
            }]
        }))
        .unwrap()
    }

    // r[verify cli.file]
    #[test]
    fn writes_an_akt_as_its_file() {
        assert_eq!(
            write(&akt()),
            "---
number: ITEM-0001
type: backlog_item
revision: 3
state: open
in: [BACKLOG-1]
links: [Blocks ITEM-0002]
title: \"Export: and import\"
kind: fix
change: {url: \"https://git.example/pr/1\"}
---

<!-- field: description -->
Line one

Line two
"
        );
    }

    #[test]
    fn quotes_what_yaml_would_read_otherwise() {
        for (text, written) in [
            ("ITEM-0001", "ITEM-0001"),
            ("Ship it", "Ship it"),
            ("2026-11-13", "2026-11-13"),
            ("true", "\"true\""),
            ("42", "\"42\""),
            ("@me", "\"@me\""),
            ("a: b", "\"a: b\""),
            ("x, y", "\"x, y\""),
            ("", "\"\""),
            ("#tag", "\"#tag\""),
        ] {
            assert_eq!(yaml(&Json::from(text)), written, "{text}");
        }
    }
}
