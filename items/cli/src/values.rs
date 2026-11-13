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
            if for_name == by.name {
                format!("Yes, {}", by.name)
            } else {
                format!("Yes, for {for_name} (checked by {})", by.name)
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
    }
}

/// The users and contacts values can name, to read them by email or name.
#[derive(Clone, Debug, Default)]
pub struct People {
    pub users: Vec<User>,
    /// `(id, display name, name, email)`.
    pub contacts: Vec<(String, String, Option<String>, Option<String>)>,
}

impl People {
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
}
