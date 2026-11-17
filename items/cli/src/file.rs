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
pub const READ_ONLY: [&str; 11] = [
    "number", "type", "revision", "version", "state", "outcome", "archived", "in", "links", "rank",
    "versions",
];

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
    /// `PASS`, …, if its type has outcomes (`akt.outcome`).
    #[serde(default)]
    pub outcome: Option<String>,
    /// The version it is read at, if it is (`version.read`).
    #[serde(default)]
    pub version: Option<Released>,
    /// Its versions, latest first (`version.list`).
    #[serde(default)]
    pub versions: Vec<Released>,
    /// How it is archived, if it is (`archive.read`).
    #[serde(default)]
    pub archived: Option<crate::model::Archived>,
}

/// A version of an akt.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Released {
    pub number: i64,
    pub revision: i64,
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub released_by: Option<crate::model::User>,
    #[serde(default)]
    pub released_at: Option<String>,
}

/// Another akt, as an akt names it.
#[derive(Clone, Debug, Deserialize)]
pub struct Other {
    pub number: String,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub akt_type: TypeName,
    pub state: StateName,
    /// Its outcome, where it is asked for.
    #[serde(default)]
    pub outcome: Option<String>,
    /// How it is archived, where it is asked for.
    #[serde(default)]
    pub archived: Option<crate::model::Archived>,
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
    /// Whether a reference in its source's text keeps it.
    #[serde(default)]
    pub by_reference: bool,
}

/// The links an akt file says: all but those references keep, which the
/// text says and which change without a revision of their target
/// (`cli.file.kinds`).
// r[impl cli.file.kinds]
pub fn filed(links: &[Link]) -> impl Iterator<Item = &Link> {
    links.iter().filter(|l| !l.by_reference)
}

#[derive(Clone, Debug, Deserialize)]
pub struct Pinned {
    pub revision: i64,
    pub title: Option<String>,
    pub state: StateName,
}

/// What to ask for an akt's file: its fields and everything around them.
pub const AKT: &str = "number title revision createdAt updatedAt outcome
  archived { at by { id name email } rule reason }
  version { number revision label }
  versions { number revision label description releasedBy { id name email } releasedAt }
  type { ...Shape document }
  state { key name initial final tone }
  fields { ...Values }
  containers { number title type { key name } state { key name } archived { at by { id name email } rule reason } }
  links {
    kind name outgoing pinnedRevision byReference
    akt { number title type { key name } state { key name } archived { at by { id name email } rule reason } }
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

/// Whether `value` goes in a section of its own: a multiline text
/// field's, or a sections field's (`cli.file.kinds`).
fn sectioned(shape: &Shape, value: &Value) -> bool {
    match value {
        Value::TextValue { .. } => shape.field(value.field()).is_some_and(|f| f.multiline),
        Value::SectionsValue { .. } | Value::MarkdownValue { .. } => true,
        _ => false,
    }
}

/// The body of `value`'s section: a multiline text, or a sections value
/// as YAML.
fn section_body(value: &Value) -> Option<String> {
    match value {
        Value::TextValue { text, .. } => Some(text.clone()),
        Value::MarkdownValue { markdown, .. } => Some(markdown.clone()),
        Value::SectionsValue { .. } => Some(crate::values::sections_yaml(&file_value(value))),
        _ => None,
    }
}

/// The akt as its file.
// r[impl cli.file]
#[must_use]
pub fn write(akt: &Akt) -> String {
    write_numbered(akt, true)
}

/// Several akter as one multi-akt file: the first as its file, each other
/// after its line, without `number`; one akt as its file.
// r[impl cli.file.multi]
#[must_use]
pub fn write_all(akter: &[Akt]) -> String {
    let mut text = String::new();
    for (i, akt) in akter.iter().enumerate() {
        if i == 0 {
            text.push_str(&write(akt));
        } else {
            text.push_str(&format!("\n<!-- akt: {} -->\n", akt.number));
            text.push_str(&write_numbered(akt, false));
        }
    }
    text
}

/// The akt as its file, with `number` unless a multi-akt file's line
/// gives it.
fn write_numbered(akt: &Akt, numbered: bool) -> String {
    let mut front = Vec::new();
    if numbered {
        front.push(("number", Json::from(akt.number.clone())));
    }
    front.extend([
        ("type", Json::from(akt.shape.key.clone())),
        ("revision", akt.revision.into()),
    ]);
    // r[impl cli.file.kinds]
    if let Some(version) = &akt.version {
        front.push(("version", version.number.into()));
    }
    front.push(("state", akt.state.key.clone().into()));
    // r[impl cli.file.kinds]
    if let Some(outcome) = &akt.outcome {
        front.push(("outcome", outcome.to_lowercase().into()));
    }
    // r[impl cli.archive]
    if let Some(archived) = &akt.archived {
        let mut said = serde_json::Map::new();
        for (key, value) in archived.json().as_object().into_iter().flatten() {
            if !value.is_null() {
                said.insert(key.clone(), value.clone());
            }
        }
        front.push(("archived", Json::Object(said)));
    }
    if !akt.containers.is_empty() {
        front.push((
            "in",
            akt.containers.iter().map(|c| c.number.clone()).collect(),
        ));
    }
    if filed(&akt.links).next().is_some() {
        front.push(("links", filed(&akt.links).map(Link::said).collect()));
    }
    let single = akt
        .fields
        .iter()
        .filter(|v| !sectioned(&akt.shape, v))
        .map(|v| (v.field().to_owned(), file_value(v)))
        .collect::<Vec<_>>();
    let sections = akt
        .fields
        .iter()
        .filter(|v| sectioned(&akt.shape, v))
        .filter_map(|v| Some((v.field().to_owned(), section_body(v)?)))
        .collect::<Vec<_>>();
    write_parts(&front, &single, &sections)
}

/// An akt file from its parts: the read-only front matter, then each
/// single-line field by key, then each multiline field's section.
#[must_use]
pub fn write_parts(
    front: &[(&str, Json)],
    single: &[(String, Json)],
    sections: &[(String, String)],
) -> String {
    let mut text = String::from("---\n");
    for (key, value) in front {
        text.push_str(&format!("{key}: {}\n", yaml(value)));
    }
    for (key, value) in single {
        text.push_str(&format!("{key}: {}\n", yaml(value)));
    }
    text.push_str("---\n");
    for (field, body) in sections {
        text.push_str(&format!(
            "\n<!-- field: {field} -->\n{}\n",
            body.trim_end_matches('\n')
        ));
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
        "outcome": akt.outcome.as_ref().map(|o| o.to_lowercase()),
        "archived": akt.archived.as_ref().map(crate::model::Archived::json),
        "in": akt.containers.iter().map(|c| c.number.clone()).collect::<Vec<_>>(),
        "links": filed(&akt.links).map(|l| json!({
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

/// The akt a nested akt's line names: `<!-- akt: <NUMBER> -->`.
fn nested_marker(line: &str) -> Option<&str> {
    line.trim()
        .strip_prefix("<!-- akt:")
        .and_then(|l| l.strip_suffix("-->"))
        .map(str::trim)
        .filter(|n| !n.is_empty())
}

/// `text` split into its akter: the first, then each other with the
/// number its line names, as a multi-akt file or an akt directory's file
/// holds them. A line in a fenced code block is text.
// r[impl cli.local.layout]
// r[impl cli.file.multi]
#[must_use]
pub fn split(text: &str) -> Vec<(Option<&str>, String)> {
    let mut parts: Vec<(Option<&str>, Vec<&str>)> = vec![(None, Vec::new())];
    let mut fence: Option<&str> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let opens = ["```", "~~~"].into_iter().find(|f| trimmed.starts_with(f));
        match (fence, opens) {
            (None, Some(f)) => fence = Some(f),
            (Some(open), Some(_))
                if trimmed
                    .trim_end()
                    .chars()
                    .all(|c| c == open.chars().next().unwrap_or('`')) =>
            {
                fence = None;
            }
            _ => {}
        }
        match nested_marker(line).filter(|_| fence.is_none() && opens.is_none()) {
            Some(number) => parts.push((Some(number), Vec::new())),
            None => parts.last_mut().expect("never empty").1.push(line),
        }
    }
    parts
        .into_iter()
        .map(|(number, lines)| {
            let mut text = lines.join("\n").trim_matches('\n').to_owned();
            text.push('\n');
            (number, text)
        })
        .collect()
}

/// Reads a multi-akt file (`cli.file.multi`), or an akt file: each akt
/// with its number, from its line or its `number`, which must agree.
// r[impl cli.file.multi]
pub fn parse_all(text: &str) -> Result<Vec<(String, Parsed)>, CliError> {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut akter = Vec::new();
    for (line, part) in split(&text) {
        let in_akt = |e: CliError| match line {
            Some(number) => CliError {
                message: format!("{number}: {}", e.message),
                ..e
            },
            None => e,
        };
        let parsed = parse(&part).map_err(in_akt)?;
        let number = match (line, parsed.text("number")) {
            (Some(line), Some(own)) if line != own => {
                return Err(CliError::refused(
                    "BAD_FILE",
                    format!("Its line names {line}, but its `number` is {own}"),
                ));
            }
            (Some(line), _) => line.to_owned(),
            (None, Some(own)) => own,
            (None, None) => return Err(CliError::refused("BAD_FILE", "It names no `number`")),
        };
        akter.push((number, parsed));
    }
    Ok(akter)
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
    // A marker inside a fenced code block is a field's text, such as an
    // akt file shown in a plan's design.
    let mut fence: Option<&str> = None;
    for (i, line) in body.lines().enumerate() {
        let trimmed = line.trim_start();
        let opens = ["```", "~~~"].into_iter().find(|f| trimmed.starts_with(f));
        match (fence, opens) {
            (None, Some(f)) if current.is_some() => fence = Some(f),
            (Some(open), Some(f))
                if f == open
                    && trimmed
                        .trim_end()
                        .chars()
                        .all(|c| c == open.chars().next().unwrap_or('`')) =>
            {
                fence = None;
            }
            _ => {}
        }
        let marker = if fence.is_none() && opens.is_none() {
            line.trim()
                .strip_prefix("<!-- field:")
                .and_then(|l| l.strip_suffix("-->"))
                .map(str::trim)
        } else {
            None
        };
        if let Some(key) = marker {
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

/// An akt as a clone's file (`clone.tree`): its akt file without `revision`,
/// `in`, `links` and `outcome`, from its number, state and field values in
/// the file's form, a `sections` field's as its value (not its YAML).
// r[impl clone.tree]
#[must_use]
pub fn clone_text(shape: &Shape, number: &str, state: &str, values: &Map<String, Json>) -> String {
    let front = [
        ("number", Json::from(number)),
        ("type", Json::from(shape.key.clone())),
        ("state", Json::from(state)),
    ];
    let mut single = Vec::new();
    let mut sections = Vec::new();
    for field in &shape.fields {
        let Some(value) = values.get(&field.key).filter(|v| !v.is_null()) else {
            continue;
        };
        if !field.sectioned() {
            single.push((field.key.clone(), value.clone()));
        } else if field.kind == "SECTIONS" {
            sections.push((field.key.clone(), crate::values::sections_yaml(value)));
        } else if let Some(text) = value.as_str() {
            sections.push((field.key.clone(), text.to_owned()));
        }
    }
    write_parts(&front, &single, &sections)
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

    // r[verify cli.file]
    #[test]
    fn a_marker_in_a_code_block_is_text() {
        let text = "---\nnumber: X\n---\n\n<!-- field: design -->\nLike this:\n\n```markdown\n<!-- field: acceptance -->\n- a\n```\n\nAfter.\n\n<!-- field: goal -->\nG\n";
        let parsed = parse(text).unwrap();
        assert_eq!(parsed.sections.len(), 2, "{:?}", parsed.sections);
        assert!(parsed.sections[0].1.contains("<!-- field: acceptance -->"));
        assert!(parsed.sections[0].1.ends_with("After."));
        assert_eq!(parsed.sections[1], ("goal".to_owned(), "G".to_owned()));
    }
}
