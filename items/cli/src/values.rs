//! Field values: as text, as the app shows them; in the akt file's form,
//! which `apply` reads back; and values given on the command line, read by
//! their field's kind (`cli.values`).

use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::model::{FieldDef, Someone, User, Value};

/// How a user or contact is shown.
#[must_use]
pub fn name_of(someone: &Someone) -> &str {
    match someone {
        Someone::User { name, .. } => name,
        Someone::Contact { display_name, .. } => display_name,
    }
}

/// How a user or contact is written in a file: by email, else name.
// r[impl cli.file]
#[must_use]
pub fn handle_of(someone: &Someone) -> &str {
    match someone {
        Someone::User { email, name, .. } => email.as_deref().unwrap_or(name),
        Someone::Contact {
            email,
            display_name,
            ..
        } => email.as_deref().unwrap_or(display_name),
    }
}

/// A value as text, as lists, boards and the akt page show it.
#[must_use]
pub fn text_of(value: &Value) -> String {
    match value {
        Value::TextValue { text, .. } => text.clone(),
        Value::ChoiceValue { option, .. } => option.name.clone(),
        Value::ChoicesValue { options, .. } => options
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        Value::DateValue { date, .. } => date.clone(),
        Value::UserValue { user, .. } => name_of(user).to_owned(),
        Value::CheckboxValue {
            by, checked_for, ..
        } => {
            let for_name = name_of(checked_for);
            // In local mode a box records whom it is for, not who checked it.
            match (for_name == by.name, by.name.is_empty()) {
                (true, true) => "Yes".to_owned(),
                (true, false) => format!("Yes, {}", by.name),
                (false, true) => format!("Yes, for {for_name}"),
                (false, false) => format!("Yes, for {for_name} (checked by {})", by.name),
            }
        }
        Value::ReferenceValue { url, label, .. } => label.clone().unwrap_or_else(|| url.clone()),
        Value::GitValue {
            repository,
            sha,
            tag,
            ..
        } => {
            let mut text = repository.clone();
            if let Some(tag) = tag {
                text.push('@');
                text.push_str(tag);
            }
            if let Some(sha) = sha {
                text.push_str(&format!(" ({})", &sha[..sha.len().min(12)]));
            }
            text
        }
        Value::DockerValue {
            repository,
            digest,
            tag,
            ..
        } => match tag {
            Some(tag) => format!("{repository}:{tag}@{digest}"),
            None => format!("{repository}@{digest}"),
        },
        Value::AttachmentValue { attachment, .. } => attachment.name.clone(),
        Value::UsersValue { users, .. } => users.iter().map(name_of).collect::<Vec<_>>().join(", "),
        Value::VerdictValue { verdicts, .. } => verdicts
            .iter()
            .map(|g| format!("{}: {}", name_of(&g.assignee), g.verdict.to_lowercase()))
            .collect::<Vec<_>>()
            .join(", "),
        Value::SectionsValue { value, .. } => {
            sections_text(&kuria_engine::sections::from_json(value).unwrap_or_default())
        }
    }
}

/// A sections value as one line: its members by number, each section's in
/// brackets after its title.
fn sections_text(items: &[kuria_engine::sections::Item]) -> String {
    use kuria_engine::sections::Item;
    items
        .iter()
        .map(|item| match item {
            Item::Akt(number) => number.clone(),
            Item::Section(s) => format!(
                "{} [{}]",
                s.title.as_deref().unwrap_or("Untitled"),
                sections_text(&s.items)
            ),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A sections value, as `editAkt` takes it, as YAML: a section's
/// keys in the order `title`, `text`, `collapsible`, `items`.
// r[impl cli.file.kinds]
#[must_use]
pub fn sections_yaml(value: &Json) -> String {
    use serde_yaml_ng::{Mapping, Value as Yaml};
    fn convert(value: &Json) -> Yaml {
        match value {
            Json::Array(items) => Yaml::Sequence(items.iter().map(convert).collect()),
            Json::Object(object) => {
                let mut mapping = Mapping::new();
                for key in ["title", "text", "collapsible", "items"] {
                    if let Some(v) = object.get(key) {
                        mapping.insert(Yaml::from(key), convert(v));
                    }
                }
                Yaml::Mapping(mapping)
            }
            other => serde_yaml_ng::to_value(other).unwrap_or(Yaml::Null),
        }
    }
    match value {
        Json::Array(items) if items.is_empty() => "[]".to_owned(),
        _ => serde_yaml_ng::to_string(&convert(value))
            .unwrap_or_default()
            .trim_end()
            .to_owned(),
    }
}

/// A sections field's section, its YAML read back as `editAkt` takes it.
// r[impl cli.file.kinds]
pub fn sections_from_text(text: &str) -> Result<Json, String> {
    if text.trim().is_empty() {
        return Ok(Json::Array(Vec::new()));
    }
    let yaml: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(text).map_err(|e| format!("isn't YAML: {e}"))?;
    let json = serde_json::to_value(yaml).map_err(|e| format!("isn't plain values: {e}"))?;
    if json.is_array() {
        Ok(json)
    } else {
        Err("is a list of akt numbers and sections".to_owned())
    }
}

/// Whether `someone` is `user`.
fn is(someone: &Someone, user: &User) -> bool {
    matches!(someone, Someone::User { id, .. } if *id == user.id)
}

/// A value as the akt file and JSON answers hold it: options by key, users
/// and contacts by email or name, a checkbox as `true` or `for:<someone>`,
/// and references, Git and Docker values and attachments as objects.
// r[impl cli.file]
#[must_use]
pub fn file_value(value: &Value) -> Json {
    match value {
        Value::TextValue { text, .. } => text.clone().into(),
        Value::ChoiceValue { option, .. } => option.key.clone().into(),
        Value::ChoicesValue { options, .. } => options.iter().map(|o| o.key.clone()).collect(),
        Value::DateValue { date, .. } => date.clone().into(),
        Value::UserValue { user, .. } => handle_of(user).into(),
        Value::CheckboxValue {
            by, checked_for, ..
        } => {
            if is(checked_for, by) {
                true.into()
            } else {
                format!("for:{}", handle_of(checked_for)).into()
            }
        }
        Value::ReferenceValue { url, label, .. } => {
            let mut object = json!({ "url": url });
            if let Some(label) = label {
                object["label"] = label.clone().into();
            }
            object
        }
        Value::GitValue {
            repository,
            sha,
            tag,
            ..
        } => {
            let mut object = json!({ "repository": repository });
            if let Some(sha) = sha {
                object["sha"] = sha.clone().into();
            }
            if let Some(tag) = tag {
                object["tag"] = tag.clone().into();
            }
            object
        }
        Value::DockerValue {
            repository,
            digest,
            tag,
            ..
        } => {
            let mut object = json!({ "repository": repository, "digest": digest });
            if let Some(tag) = tag {
                object["tag"] = tag.clone().into();
            }
            object
        }
        Value::AttachmentValue { attachment, .. } => {
            json!({ "id": attachment.id, "name": attachment.name })
        }
        Value::UsersValue { users, .. } => users.iter().map(|u| Json::from(handle_of(u))).collect(),
        Value::VerdictValue { verdicts, .. } => verdicts
            .iter()
            .map(|g| {
                (
                    handle_of(&g.assignee).to_owned(),
                    Json::from(g.verdict.to_lowercase()),
                )
            })
            .collect::<serde_json::Map<_, _>>()
            .into(),
        Value::SectionsValue { value, .. } => value.clone(),
    }
}

/// The users and contacts values can name, to read them by email or name.
#[derive(Clone, Debug, Default)]
pub struct People {
    pub users: Vec<User>,
    /// `(id, display name, name, email)`.
    pub contacts: Vec<(String, String, Option<String>, Option<String>)>,
    /// The viewer's subject, which `@me` names in a value.
    pub me: Option<String>,
}

impl People {
    /// Whom `text` names, as a value's input takes them: a subject, or
    /// `{"contact": <id>}`.
    pub fn input(&self, text: &str) -> Result<Json, CliError> {
        let resolved = self.resolve(text)?;
        if resolved == "me" {
            return self
                .me
                .clone()
                .map(Json::from)
                .ok_or_else(|| CliError::refused("UNKNOWN_PERSON", "`@me` isn't known here"));
        }
        Ok(match resolved.strip_prefix("contact:") {
            Some(contact) => json!({ "contact": contact }),
            None => resolved.into(),
        })
    }

    /// Whom `text` names: `@me`, a user by email or name, a contact by
    /// email or name, or an identifier. Answers it as `FieldCondition`
    /// takes it: `me`, a subject, or `contact:<id>`.
    pub fn resolve(&self, text: &str) -> Result<String, CliError> {
        let text = text.trim();
        if text == "@me" {
            return Ok("me".to_owned());
        }
        if text.starts_with("contact:") || uuid::Uuid::parse_str(text).is_ok() {
            return Ok(text.to_owned());
        }
        let same = |a: &str| a.eq_ignore_ascii_case(text);
        let users: Vec<&User> = self
            .users
            .iter()
            .filter(|u| u.email.as_deref().is_some_and(same) || same(&u.name))
            .collect();
        let contacts: Vec<&str> = self
            .contacts
            .iter()
            .filter(|(_, shown, name, email)| {
                email.as_deref().is_some_and(same)
                    || name.as_deref().is_some_and(same)
                    || same(shown)
            })
            .map(|(id, ..)| id.as_str())
            .collect();
        match (users.as_slice(), contacts.as_slice()) {
            ([user], []) => Ok(user.id.clone()),
            ([], [contact]) => Ok(format!("contact:{contact}")),
            ([], []) => Err(CliError::refused(
                "UNKNOWN_PERSON",
                format!("No user or contact is `{text}`; see `kuria users` and `kuria contacts`"),
            )),
            _ => Err(CliError::refused(
                "AMBIGUOUS_PERSON",
                format!("More than one user or contact is `{text}`; name them by email"),
            )),
        }
    }
}

/// `text`, given for `field` in `--where`, as `FieldCondition` takes it.
// r[impl cli.values]
pub fn condition_value(field: &FieldDef, text: &str, people: &People) -> Result<String, CliError> {
    let refused = |message: String| CliError {
        field: Some(field.key.clone()),
        ..CliError::refused("BAD_REQUEST", message)
    };
    match field.kind.as_str() {
        "CHOICE" | "CHOICES" => option_key(field, text).ok_or_else(|| {
            refused(format!(
                "`{text}` isn't an option of {}: {}",
                field.name,
                field
                    .options
                    .iter()
                    .map(|o| format!("`{}` ({})", o.key, o.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }),
        "DATE" if is_date(text) => Ok(text.to_owned()),
        "DATE" => Err(refused(format!("`{text}` isn't a date as YYYY-MM-DD"))),
        "TEXT" if !field.multiline => Ok(text.to_owned()),
        "USER" | "CHECKBOX" => {
            let who = text.strip_prefix("for:").unwrap_or(text);
            people.resolve(who).map_err(|e| CliError {
                field: Some(field.key.clone()),
                ..e
            })
        }
        _ => Err(refused(format!("{} can't be filtered by", field.name))),
    }
}

/// The key of `field`'s option `text` names, by key or name.
#[must_use]
pub fn option_key(field: &FieldDef, text: &str) -> Option<String> {
    field
        .options
        .iter()
        .find(|o| o.key == text)
        .or_else(|| {
            field
                .options
                .iter()
                .find(|o| o.name.eq_ignore_ascii_case(text))
        })
        .map(|o| o.key.clone())
}

/// Whether `text` is `YYYY-MM-DD`.
#[must_use]
pub fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
}

/// A value as the server's input takes it, for comparing the akt as it is
/// with a file or `--set`.
#[must_use]
pub fn current_input(value: &Value) -> Json {
    let someone = |who: &Someone| match who {
        Someone::User { id, .. } => Json::from(id.clone()),
        Someone::Contact { id, .. } => json!({ "contact": id }),
    };
    match value {
        Value::CheckboxValue {
            by, checked_for, ..
        } => {
            if is(checked_for, by) {
                true.into()
            } else {
                json!({ "for": someone(checked_for) })
            }
        }
        Value::UserValue { user, .. } => someone(user),
        Value::UsersValue { users, .. } => users.iter().map(someone).collect(),
        Value::AttachmentValue { attachment, .. } => attachment.id.clone().into(),
        other => file_value(other),
    }
}

/// The input `field` takes for `value`, as a file or `--set` gives it:
/// options by key or name, people by email, name or `@me`, a checkbox as
/// `true`, `false` or `for:<someone>`, references, Git and Docker values as
/// objects. `null` clears the field.
// r[impl cli.values]
pub fn input_of(field: &FieldDef, value: &Json, people: &People) -> Result<Json, CliError> {
    let refused = |message: String| CliError {
        field: Some(field.key.clone()),
        ..CliError::refused("BAD_REQUEST", message)
    };
    let in_field = |e: CliError| CliError {
        field: Some(field.key.clone()),
        ..e
    };
    let text = || match value {
        Json::String(text) => Some(text.clone()),
        Json::Number(n) => Some(n.to_string()),
        Json::Bool(b) => Some(b.to_string()),
        _ => None,
    };
    if value.is_null() {
        return Ok(if field.kind == "CHECKBOX" {
            false.into()
        } else {
            Json::Null
        });
    }
    match field.kind.as_str() {
        "TEXT" => text()
            .map(Json::from)
            .ok_or_else(|| refused(format!("{} takes text", field.name))),
        "CHOICE" => {
            let given =
                text().ok_or_else(|| refused(format!("{} takes one option", field.name)))?;
            option_key(field, &given)
                .map(Json::from)
                .ok_or_else(|| refused(not_an_option(field, &given)))
        }
        "CHOICES" => choices_input(field, value),
        "DATE" => match text() {
            Some(date) if is_date(&date) => Ok(date.into()),
            _ => Err(refused(format!(
                "{} takes a date as YYYY-MM-DD",
                field.name
            ))),
        },
        "USER" if field.multiple => users_input(field, value, people),
        "SECTIONS" => sections_input(field, value),
        "VERDICT" => Err(CliError {
            field: Some(field.key.clone()),
            ..CliError::refused(
                "READ_ONLY",
                format!("{} is given with `kuria verdict`", field.name),
            )
        }),
        "USER" => {
            match value {
                Json::Object(o) if o.contains_key("contact") => Ok(value.clone()),
                _ => people
                    .input(&text().ok_or_else(|| {
                        refused(format!("{} takes a user or contact", field.name))
                    })?)
                    .map_err(in_field),
            }
        }
        "CHECKBOX" => checkbox_input(field, value, people),
        "REFERENCE" => match value {
            Json::String(url) => Ok(json!({ "url": url })),
            Json::Object(o) if o.contains_key("url") => Ok(without_nulls(value)),
            _ => Err(refused(format!(
                "{} takes {{\"url\", \"label\"}}",
                field.name
            ))),
        },
        "GIT" | "DOCKER" => match value {
            Json::Object(o) if o.contains_key("repository") => Ok(without_nulls(value)),
            _ => Err(refused(format!(
                "{} takes an object with a `repository`",
                field.name
            ))),
        },
        "ATTACHMENT" => match value {
            Json::Object(o) => o
                .get("id")
                .cloned()
                .ok_or_else(|| refused(format!("{} takes an attachment's id", field.name))),
            _ => text()
                .map(Json::from)
                .ok_or_else(|| refused(format!("{} takes an attachment's id", field.name))),
        },
        other => Err(refused(format!(
            "{} is a {other} field, which the client can't write",
            field.name
        ))),
    }
}

/// A list of people's input, from a list or comma-separated text.
fn users_input(field: &FieldDef, value: &Json, people: &People) -> Result<Json, CliError> {
    let refused = || CliError {
        field: Some(field.key.clone()),
        ..CliError::refused(
            "BAD_REQUEST",
            format!("{} takes a list of users and contacts", field.name),
        )
    };
    let given: Vec<Json> = match value {
        Json::Array(items) => items.clone(),
        Json::String(text) => text
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(Json::from)
            .collect(),
        _ => return Err(refused()),
    };
    given
        .iter()
        .map(|one| match one {
            Json::Object(o) if o.contains_key("contact") => Ok(one.clone()),
            Json::String(text) => people.input(text).map_err(|e| CliError {
                field: Some(field.key.clone()),
                ..e
            }),
            _ => Err(refused()),
        })
        .collect()
}

/// A sections value's input: the YAML of a file's section, or a list.
fn sections_input(field: &FieldDef, value: &Json) -> Result<Json, CliError> {
    let refused = |why: String| CliError {
        field: Some(field.key.clone()),
        ..CliError::refused("BAD_REQUEST", format!("{} {why}", field.name))
    };
    match value {
        Json::String(text) => sections_from_text(text).map_err(refused),
        Json::Array(_) => Ok(value.clone()),
        _ => Err(refused(
            "takes a list of akt numbers and sections".to_owned(),
        )),
    }
}

/// A list of options' input, from a list or comma-separated text.
fn choices_input(field: &FieldDef, value: &Json) -> Result<Json, CliError> {
    let given: Vec<String> = match value {
        Json::Array(items) => items
            .iter()
            .map(|i| i.as_str().unwrap_or_default().to_owned())
            .collect(),
        other => other
            .as_str()
            .unwrap_or_default()
            .split(',')
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect(),
    };
    given
        .iter()
        .map(|g| {
            option_key(field, g)
                .map(Json::from)
                .ok_or_else(|| CliError {
                    field: Some(field.key.clone()),
                    ..CliError::refused("BAD_REQUEST", not_an_option(field, g))
                })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Json::from)
}

/// A checkbox's input: `true`, `false`, or `{"for": <someone>}`.
fn checkbox_input(field: &FieldDef, value: &Json, people: &People) -> Result<Json, CliError> {
    match value {
        Json::Bool(b) => Ok((*b).into()),
        Json::Object(o) if o.contains_key("for") => Ok(value.clone()),
        _ => {
            let given = value.as_str().unwrap_or_default();
            match given {
                "true" => Ok(true.into()),
                "false" => Ok(false.into()),
                _ => match given.strip_prefix("for:") {
                    Some(who) => Ok(json!({ "for": people.input(who).map_err(|e| CliError {
                        field: Some(field.key.clone()),
                        ..e
                    })? })),
                    None => Err(CliError {
                        field: Some(field.key.clone()),
                        ..CliError::refused(
                            "BAD_REQUEST",
                            format!("{} takes true, false or for:<someone>", field.name),
                        )
                    }),
                },
            }
        }
    }
}

fn not_an_option(field: &FieldDef, given: &str) -> String {
    format!(
        "`{given}` isn't an option of {}: {}",
        field.name,
        field
            .options
            .iter()
            .map(|o| format!("`{}` ({})", o.key, o.name))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn without_nulls(value: &Json) -> Json {
    match value {
        Json::Object(o) => Json::Object(
            o.iter()
                .filter(|(_, v)| !v.is_null() && v.as_str() != Some(""))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Whether two inputs for a field are the same value: texts alike but for
/// trailing line breaks and spaces.
#[must_use]
pub fn same_input(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::String(a), Json::String(b)) => a.trim_end() == b.trim_end(),
        (Json::Bool(false), Json::Null) | (Json::Null, Json::Bool(false)) => true,
        _ => without_nulls(a) == without_nulls(b),
    }
}

/// `text` given in `--set` for `field`, as a value: JSON for references,
/// Git and Docker values; the contents of a file for `@<file>`, or of
/// standard input for `@-`; empty to clear the field.
// r[impl cli.values]
pub fn set_value(field: &FieldDef, text: &str) -> Result<Json, CliError> {
    let read = |path: &str| -> Result<String, CliError> {
        if path == "-" {
            std::io::read_to_string(std::io::stdin())
                .map_err(|e| CliError::refused("IO", format!("standard input: {e}")))
        } else {
            std::fs::read_to_string(path)
                .map_err(|e| CliError::refused("IO", format!("{path}: {e}")))
        }
    };
    if text.is_empty() {
        return Ok(Json::Null);
    }
    match field.kind.as_str() {
        "REFERENCE" | "GIT" | "DOCKER" if text.trim_start().starts_with('{') => {
            serde_json::from_str(text).map_err(|e| CliError {
                field: Some(field.key.clone()),
                ..CliError::refused("BAD_REQUEST", format!("{} isn't JSON: {e}", field.name))
            })
        }
        "TEXT" | "SECTIONS" => match text.strip_prefix('@') {
            Some(path) => read(path).map(Json::from),
            None => Ok(text.into()),
        },
        _ => Ok(text.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Opt;

    fn user(id: &str, name: &str, email: Option<&str>) -> User {
        User {
            id: id.to_owned(),
            name: name.to_owned(),
            email: email.map(str::to_owned),
        }
    }

    fn someone(user: &User) -> Someone {
        Someone::User {
            id: user.id.clone(),
            name: user.name.clone(),
            email: user.email.clone(),
        }
    }

    fn field(kind: &str) -> FieldDef {
        FieldDef {
            key: "f".to_owned(),
            name: "F".to_owned(),
            kind: kind.to_owned(),
            description: None,
            required: false,
            multiline: false,
            options: vec![Opt {
                key: "fix".to_owned(),
                name: "Bug fix".to_owned(),
            }],
            format: None,
            multiple: false,
            of: Vec::new(),
            assignees: None,
        }
    }

    fn people() -> People {
        People {
            users: vec![user("u1", "Dev", Some("dev@shop.example"))],
            contacts: vec![(
                "c1".to_owned(),
                "Auditor".to_owned(),
                Some("Auditor".to_owned()),
                Some("audit@x.example".to_owned()),
            )],
            me: None,
        }
    }

    // r[verify cli.file]
    #[test]
    fn writes_values_as_files_read_them_back() {
        let dev = user("u1", "Dev", Some("dev@shop.example"));
        let other = user("u2", "Ops", None);
        let checked = |checked_for: &User| Value::CheckboxValue {
            field: "ok".to_owned(),
            at: String::new(),
            by: dev.clone(),
            checked_for: someone(checked_for),
        };
        assert_eq!(file_value(&checked(&dev)), json!(true));
        assert_eq!(file_value(&checked(&other)), json!("for:Ops"));
        assert_eq!(text_of(&checked(&other)), "Yes, for Ops (checked by Dev)");
        let assignee = Value::UserValue {
            field: "a".to_owned(),
            user: someone(&dev),
        };
        assert_eq!(file_value(&assignee), json!("dev@shop.example"));
        assert_eq!(text_of(&assignee), "Dev");
        let choice = Value::ChoiceValue {
            field: "k".to_owned(),
            option: Opt {
                key: "fix".to_owned(),
                name: "Bug fix".to_owned(),
            },
        };
        assert_eq!(file_value(&choice), json!("fix"));
        assert_eq!(text_of(&choice), "Bug fix");
        let git = Value::GitValue {
            field: "g".to_owned(),
            repository: "kuria".to_owned(),
            sha: Some("0123456789abcdef".to_owned()),
            tag: Some("v1".to_owned()),
        };
        assert_eq!(text_of(&git), "kuria@v1 (0123456789ab)");
        assert_eq!(
            file_value(&git),
            json!({ "repository": "kuria", "sha": "0123456789abcdef", "tag": "v1" })
        );
    }

    // r[verify cli.values]
    #[test]
    fn reads_condition_values_by_their_field_s_kind() {
        let people = people();
        assert_eq!(
            condition_value(&field("CHOICE"), "fix", &people).unwrap(),
            "fix"
        );
        assert_eq!(
            condition_value(&field("CHOICE"), "bug fix", &people).unwrap(),
            "fix"
        );
        let refused = condition_value(&field("CHOICE"), "feature", &people).unwrap_err();
        assert_eq!(refused.field.as_deref(), Some("f"));
        assert!(
            refused.message.contains("`fix` (Bug fix)"),
            "{}",
            refused.message
        );
        assert_eq!(
            condition_value(&field("DATE"), "2026-11-13", &people).unwrap(),
            "2026-11-13"
        );
        assert!(condition_value(&field("DATE"), "13/11/2026", &people).is_err());
        assert_eq!(
            condition_value(&field("USER"), "@me", &people).unwrap(),
            "me"
        );
        assert_eq!(
            condition_value(&field("USER"), "DEV@shop.example", &people).unwrap(),
            "u1"
        );
        assert_eq!(
            condition_value(&field("CHECKBOX"), "for:Auditor", &people).unwrap(),
            "contact:c1"
        );
        assert_eq!(
            condition_value(&field("USER"), "nobody", &people)
                .unwrap_err()
                .code,
            "UNKNOWN_PERSON"
        );
        assert!(condition_value(&field("REFERENCE"), "x", &people).is_err());
    }

    // r[verify cli.values]
    #[test]
    fn reads_inputs_by_their_field_s_kind() {
        let mut people = people();
        people.me = Some("u1".to_owned());
        let input = |kind: &str, value: Json| input_of(&field(kind), &value, &people);
        assert_eq!(input("CHOICE", json!("Bug fix")).unwrap(), json!("fix"));
        assert_eq!(
            input("CHOICES", json!("fix, Bug fix")).unwrap(),
            json!(["fix", "fix"])
        );
        assert_eq!(input("USER", json!("@me")).unwrap(), json!("u1"));
        assert_eq!(
            input("USER", json!("Auditor")).unwrap(),
            json!({ "contact": "c1" })
        );
        assert_eq!(
            input("CHECKBOX", json!("for:dev@shop.example")).unwrap(),
            json!({ "for": "u1" })
        );
        assert_eq!(input("CHECKBOX", Json::Null).unwrap(), json!(false));
        assert_eq!(input("TEXT", json!(42)).unwrap(), json!("42"));
        assert_eq!(
            input("REFERENCE", json!("https://x.example")).unwrap(),
            json!({ "url": "https://x.example" })
        );
        assert_eq!(
            input("DATE", json!("2026-1-1"))
                .unwrap_err()
                .field
                .as_deref(),
            Some("f")
        );
        let reference = field("REFERENCE");
        assert_eq!(
            set_value(&reference, r#"{"url": "https://x.example", "label": "X"}"#).unwrap(),
            json!({ "url": "https://x.example", "label": "X" })
        );
        assert_eq!(set_value(&field("TEXT"), "").unwrap(), Json::Null);
        assert!(same_input(&json!("a\n"), &json!("a")));
        assert!(same_input(
            &json!({ "url": "u", "label": null }),
            &json!({ "url": "u" })
        ));
    }
}
