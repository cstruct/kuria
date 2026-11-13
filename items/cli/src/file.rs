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

use crate::error::CliError;
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

/// An akt file as read: its front matter, and its sections by field key.
#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    pub front: Map<String, Json>,
    pub sections: Vec<(String, String)>,
}

impl Parsed {
    /// A front-matter key as text.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<String> {
        match self.front.get(key)? {
            Json::String(s) => Some(s.clone()),
            Json::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }

    /// The field values the file gives, by key: the front matter's other
    /// keys, and the sections. A field given twice is refused.
    pub fn values(&self) -> Result<Map<String, Json>, CliError> {
        let mut values: Map<String, Json> = self
            .front
            .iter()
            .filter(|(k, _)| !READ_ONLY.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (key, text) in &self.sections {
            if values.insert(key.clone(), text.clone().into()).is_some() {
                return Err(CliError {
                    field: Some(key.clone()),
                    ..CliError::refused("BAD_FILE", format!("`{key}` is given twice"))
                });
            }
        }
        Ok(values)
    }
}

/// Reads an akt file.
// r[impl cli.file]
pub fn parse(text: &str) -> Result<Parsed, CliError> {
    let bad = |message: String| CliError::refused("BAD_FILE", message);
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| bad("An akt file starts with `---` and its front matter".to_owned()))?;
    let (front, body) = match rest.find("\n---\n") {
        Some(end) => (&rest[..end], &rest[end + 5..]),
        None => match rest.strip_suffix("\n---") {
            Some(front) => (front, ""),
            None => return Err(bad("The front matter doesn't end with `---`".to_owned())),
        },
    };
    let front: Json = if front.trim().is_empty() {
        Json::Object(Map::new())
    } else {
        // As YAML's own value first, which refuses a key given twice.
        let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(front)
            .map_err(|e| bad(format!("The front matter isn't YAML: {e}")))?;
        serde_json::to_value(yaml)
            .map_err(|e| bad(format!("The front matter isn't plain values: {e}")))?
    };
    let Json::Object(front) = front else {
        return Err(bad("The front matter is `key: value` lines".to_owned()));
    };
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for (i, line) in body.lines().enumerate() {
        if let Some(key) = line
            .trim()
            .strip_prefix("<!-- field:")
            .and_then(|l| l.strip_suffix("-->"))
            .map(str::trim)
        {
            if let Some((key, lines)) = current.take() {
                sections.push((key, lines.join("\n").trim_matches('\n').to_owned()));
            }
            if sections.iter().any(|(k, _)| k == key) {
                return Err(bad(format!("The section for `{key}` is there twice")));
            }
            current = Some((key.to_owned(), Vec::new()));
        } else if let Some((_, lines)) = current.as_mut() {
            lines.push(line);
        } else if !line.trim().is_empty() {
            return Err(bad(format!(
                "Line {} is outside a section: each multiline field starts with `<!-- field: <key> -->`",
                i + 1
            )));
        }
    }
    if let Some((key, lines)) = current {
        sections.push((key, lines.join("\n").trim_matches('\n').to_owned()));
    }
    Ok(Parsed { front, sections })
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

    // r[verify cli.file]
    #[test]
    fn reads_back_what_it_writes() {
        let akt = akt();
        let parsed = parse(&write(&akt)).unwrap();
        assert_eq!(parsed.text("number").as_deref(), Some("ITEM-0001"));
        assert_eq!(parsed.front["revision"], json!(3));
        assert_eq!(parsed.front["in"], json!(["BACKLOG-1"]));
        let values = parsed.values().unwrap();
        assert_eq!(values["title"], json!("Export: and import"));
        assert_eq!(values["kind"], json!("fix"));
        assert_eq!(
            values["change"],
            json!({ "url": "https://git.example/pr/1" })
        );
        assert_eq!(values["description"], json!("Line one\n\nLine two"));
    }

    #[test]
    fn refuses_what_isn_t_an_akt_file() {
        for (text, why) in [
            ("number: X\n", "no front matter"),
            ("---\nnumber: X\n", "unended front matter"),
            ("---\n- a\n---\n", "not a mapping"),
            (
                "---\nnumber: X\n---\nloose text\n",
                "text outside a section",
            ),
            (
                "---\n---\n<!-- field: a -->\nx\n<!-- field: a -->\ny\n",
                "a section twice",
            ),
            ("---\na: 1\na: 2\n---\n", "a key twice"),
        ] {
            assert_eq!(
                parse(text).map(|_| ()).unwrap_err().code,
                "BAD_FILE",
                "{why}"
            );
        }
        let twice = parse("---\na: x\n---\n<!-- field: a -->\ny\n").unwrap();
        assert_eq!(twice.values().unwrap_err().field.as_deref(), Some("a"));
        assert!(parse("---\r\nnumber: X\r\n---\r\n").is_ok());
    }
}
