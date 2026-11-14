//! Field values in local mode, to and from the engine's.
//!
//! An akt directory names people as text (`cli.local.people`), but the
//! engine's values name users by subject identifier. Each name or email is
//! given a stable stand-in identifier, derived from its text, on the way
//! in, and written back as that text on the way out.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use kuria_config::AktType;
use kuria_engine::verdicts::{Given, Verdict};
use kuria_engine::{Someone, Value};
use serde_json::{Map, Value as Json, json};
use uuid::Uuid;

use super::dir::LocalAkt;
use crate::error::CliError;
use crate::model::FieldDef;
use crate::values::{People, input_of};

/// The namespace stand-in identifiers are derived in.
pub(crate) const NAMESPACE: Uuid = Uuid::from_u128(0x6b75_7269_612d_6c6f_6361_6c2d_7065_6f70);

/// People as text, and their stand-in identifiers.
#[derive(Debug, Default)]
pub struct Ids {
    texts: HashMap<Uuid, String>,
    /// Who `@me` is.
    pub me: Option<String>,
}

impl Ids {
    #[must_use]
    pub fn new(me: Option<String>) -> Self {
        Self {
            texts: HashMap::new(),
            me,
        }
    }

    /// The stand-in identifier of the person `text` names.
    pub fn id(&mut self, text: &str) -> Uuid {
        let text = text.trim();
        let id = Uuid::new_v5(&NAMESPACE, text.to_lowercase().as_bytes());
        self.texts.entry(id).or_insert_with(|| text.to_owned());
        id
    }

    /// Names `id` by `text`, as a bundle records whom it names.
    pub fn name(&mut self, id: Uuid, text: &str) {
        self.texts.insert(id, text.trim().to_owned());
    }

    /// Everyone named so far, with their text.
    pub fn known(&self) -> impl Iterator<Item = (Uuid, &str)> {
        self.texts.iter().map(|(id, text)| (*id, text.as_str()))
    }

    /// The text `id` stands in for.
    #[must_use]
    pub fn text(&self, id: Uuid) -> String {
        self.texts
            .get(&id)
            .cloned()
            .unwrap_or_else(|| id.to_string())
    }

    /// Whom `text` names: `@me` is [`Ids::me`].
    pub fn person(&mut self, text: &str) -> Result<Uuid, CliError> {
        let text = text.trim();
        if text == "@me" {
            let me = self.me.clone().ok_or_else(|| {
                CliError::refused(
                    "BAD_REQUEST",
                    "`@me` is no one here: set KURIA_ME, or git's user.email",
                )
            })?;
            return Ok(self.id(&me));
        }
        Ok(self.id(text))
    }

    /// Who makes changes: `@me`, else no one in particular.
    pub fn changer(&mut self) -> Uuid {
        match self.me.clone() {
            Some(me) => self.id(&me),
            None => Uuid::nil(),
        }
    }
}

/// A value given in a file or `--set` for `field`, as the input the
/// engine takes: people by their stand-in identifiers.
// r[impl cli.local.people]
pub fn input(field: &FieldDef, raw: &Json, ids: &mut Ids) -> Result<Json, CliError> {
    let in_field = |e: CliError| CliError {
        field: Some(field.key.clone()),
        ..e
    };
    match (field.kind.as_str(), raw) {
        (_, Json::Null) | ("CHECKBOX", Json::Bool(false)) => Ok(if field.kind == "CHECKBOX" {
            false.into()
        } else {
            Json::Null
        }),
        ("USER", Json::String(text)) if !field.multiple => {
            Ok(ids.person(text).map_err(in_field)?.to_string().into())
        }
        ("USER", Json::String(_) | Json::Array(_)) => {
            let given: Vec<String> = match raw {
                Json::Array(items) => items
                    .iter()
                    .map(|i| i.as_str().map(str::to_owned))
                    .collect::<Option<_>>()
                    .ok_or_else(|| {
                        in_field(CliError::refused(
                            "BAD_REQUEST",
                            format!("{} takes a list of people", field.name),
                        ))
                    })?,
                _ => raw
                    .as_str()
                    .unwrap_or_default()
                    .split(',')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_owned)
                    .collect(),
            };
            let mut people = Vec::new();
            for text in given {
                people.push(Json::from(ids.person(&text).map_err(in_field)?.to_string()));
            }
            Ok(Json::Array(people))
        }
        ("CHECKBOX", Json::String(text)) => match text.trim().strip_prefix("for:") {
            Some(who) => Ok(json!({ "for": ids.person(who).map_err(in_field)?.to_string() })),
            None => input_of(field, raw, &People::default()),
        },
        ("ATTACHMENT", _) => Err(in_field(super::unsupported(
            "Attaching a file",
            "an akt directory holds no attachments",
        ))),
        ("TEXT", Json::String(text)) if field.multiline => Ok(text.trim_end_matches('\n').into()),
        _ => input_of(field, raw, &People::default()),
    }
}

/// An akt's values as the engine holds them.
pub fn current(
    ty: &AktType,
    akt: &LocalAkt,
    ids: &mut Ids,
) -> Result<BTreeMap<String, Value>, CliError> {
    let bad = |key: &str, why: String| {
        CliError::refused("BAD_FILE", format!("{}: `{key}` {why}", akt.path.display()))
    };
    let mut values = BTreeMap::new();
    for (key, raw) in &akt.values {
        let Some(field) = ty.field(key) else { continue };
        let kind = serde_json::to_value(field.kind).expect("serializable");
        let value = match (kind.as_str().unwrap_or_default(), raw) {
            (_, Json::Null) | ("checkbox", Json::Bool(false)) => continue,
            ("user", Json::String(text)) => Value::User(Someone::User(ids.id(text))),
            ("user", Json::Array(items)) => Value::Users(
                items
                    .iter()
                    .map(|i| {
                        i.as_str()
                            .map(|t| Someone::User(ids.id(t)))
                            .ok_or_else(|| bad(key, "lists people".to_owned()))
                    })
                    .collect::<Result<_, _>>()?,
            ),
            // An akt directory keeps each assignee's verdict, not who gave
            // it for them, nor when: git does.
            ("verdict", Json::Object(given)) => {
                let mut verdicts = Vec::new();
                for (who, verdict) in given {
                    let id = ids.id(who);
                    verdicts.push(Given {
                        assignee: Someone::User(id),
                        verdict: Verdict::read(verdict.as_str().unwrap_or_default())
                            .map_err(|e| bad(key, e))?,
                        by: id,
                        at: DateTime::<Utc>::UNIX_EPOCH,
                    });
                }
                Value::Verdicts(verdicts)
            }
            ("checkbox", Json::Bool(true)) => Value::Checkbox {
                by: ids.changer(),
                at: DateTime::<Utc>::UNIX_EPOCH,
                on_behalf_of: None,
            },
            ("checkbox", Json::String(text)) if text.starts_with("for:") => Value::Checkbox {
                by: ids.changer(),
                at: DateTime::<Utc>::UNIX_EPOCH,
                on_behalf_of: Some(Someone::User(ids.id(&text["for:".len()..]))),
            },
            ("attachment", raw) => {
                let id = raw["id"]
                    .as_str()
                    .or_else(|| raw.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok());
                Value::Attachment(
                    id.ok_or_else(|| bad(key, "isn't an attachment's id".to_owned()))?,
                )
            }
            _ => Value::from_json(field, raw).map_err(|e| bad(key, e))?,
        };
        values.insert(key.clone(), value);
    }
    Ok(values)
}

/// `value` in the akt file's form: people as their text, a checkbox as
/// `true` or `for:<someone>`.
#[must_use]
pub fn file_form(value: &Value, ids: &Ids) -> Json {
    let someone = |s: &Someone| match s {
        Someone::User(id) | Someone::Contact(id) => ids.text(*id),
    };
    match value {
        Value::User(s) => someone(s).into(),
        Value::Checkbox {
            on_behalf_of: None, ..
        } => true.into(),
        Value::Checkbox {
            on_behalf_of: Some(s),
            ..
        } => format!("for:{}", someone(s)).into(),
        Value::Attachment(id) => json!({ "id": id.to_string() }),
        Value::Users(people) => people.iter().map(|s| Json::from(someone(s))).collect(),
        Value::Verdicts(given) => given
            .iter()
            .map(|g| (someone(&g.assignee), Json::from(g.verdict.as_str())))
            .collect::<Map<_, _>>()
            .into(),
        other => other.to_json(),
    }
}

/// The values an akt has after a change: `outcome`'s, in the file's form,
/// keeping the file's own form for those the change didn't touch.
#[must_use]
pub fn after(
    akt: Option<&LocalAkt>,
    fields: &BTreeMap<String, Value>,
    changed: &[String],
    ids: &Ids,
) -> Map<String, Json> {
    fields
        .iter()
        .map(|(key, value)| {
            let kept = akt
                .filter(|_| !changed.contains(key))
                .and_then(|a| a.values.get(key))
                .cloned();
            (key.clone(), kept.unwrap_or_else(|| file_form(value, ids)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cli.local.people]
    #[test]
    fn people_keep_their_text_through_the_engine() {
        let mut ids = Ids::new(Some("me@kuria.example".to_owned()));
        let a = ids.id("Dev@shop.example");
        assert_eq!(
            ids.id("dev@shop.example"),
            a,
            "the same person, ignoring case"
        );
        assert_eq!(ids.text(a), "Dev@shop.example");
        assert_eq!(ids.person("@me").unwrap(), ids.id("me@kuria.example"));
        let value = Value::Checkbox {
            by: ids.changer(),
            at: DateTime::<Utc>::UNIX_EPOCH,
            on_behalf_of: Some(Someone::User(a)),
        };
        assert_eq!(file_form(&value, &ids), json!("for:Dev@shop.example"));
        assert_eq!(
            file_form(&Value::User(Someone::User(a)), &ids),
            json!("Dev@shop.example")
        );
        let mut nobody = Ids::new(None);
        assert_eq!(nobody.person("@me").unwrap_err().code, "BAD_REQUEST");
    }
}
