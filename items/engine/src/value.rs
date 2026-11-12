//! Field values and their JSON form.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use kuria_config::{AktType, FieldDef, FieldKind};
use serde_json::{Map, Value as Json, json};
use uuid::Uuid;

/// A field value, of its field's kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Choice(String),
    Choices(Vec<String>),
    Date(NaiveDate),
    User(Uuid),
    Reference { url: String, label: Option<String> },
}

impl Value {
    /// Reads a value for `field` from JSON, or says why it doesn't fit.
    ///
    /// - `text`: a string, without line breaks unless the field is multiline
    /// - `choice`: an option key; `choices`: a list of distinct option keys
    /// - `date`: `YYYY-MM-DD`
    /// - `subject`: a UUID
    /// - `reference`: `{"url": "https://…", "label": "…"}`, the label optional
    pub fn from_json(field: &FieldDef, json: &Json) -> Result<Self, String> {
        let option = |key: &str| {
            if field.options.iter().any(|o| o.key == key) {
                Ok(key.to_owned())
            } else {
                Err(format!("`{key}` is not one of its options"))
            }
        };
        match field.kind {
            FieldKind::Text => {
                let text = json.as_str().ok_or("expected a string")?;
                if !field.multiline && text.contains(['\n', '\r']) {
                    return Err("a single-line text can't hold line breaks".to_owned());
                }
                Ok(Self::Text(text.to_owned()))
            }
            FieldKind::Choice => Ok(Self::Choice(option(
                json.as_str().ok_or("expected an option key")?,
            )?)),
            FieldKind::Choices => {
                let items = json.as_array().ok_or("expected a list of option keys")?;
                let mut keys = Vec::new();
                for item in items {
                    let key = option(item.as_str().ok_or("expected option keys")?)?;
                    if keys.contains(&key) {
                        return Err(format!("`{key}` is chosen twice"));
                    }
                    keys.push(key);
                }
                Ok(Self::Choices(keys))
            }
            FieldKind::Date => {
                let text = json.as_str().ok_or("expected a date, YYYY-MM-DD")?;
                NaiveDate::parse_from_str(text, "%Y-%m-%d")
                    .ok()
                    .filter(|date| date.format("%Y-%m-%d").to_string() == text)
                    .map(Self::Date)
                    .ok_or_else(|| format!("`{text}` is not a date, YYYY-MM-DD"))
            }
            FieldKind::User => {
                let text = json.as_str().ok_or("expected a subject identifier")?;
                Uuid::parse_str(text)
                    .map(Self::User)
                    .map_err(|_| format!("`{text}` is not a subject identifier"))
            }
            FieldKind::Reference => {
                let object = json
                    .as_object()
                    .ok_or("expected {\"url\": …, \"label\": …}")?;
                if let Some(extra) = object.keys().find(|k| *k != "url" && *k != "label") {
                    return Err(format!("a reference has no `{extra}`"));
                }
                let url = object
                    .get("url")
                    .and_then(Json::as_str)
                    .ok_or("a reference needs a url")?;
                let parsed = url::Url::parse(url).map_err(|_| format!("`{url}` is not a URL"))?;
                if !matches!(parsed.scheme(), "http" | "https") {
                    return Err("a reference's url must be http or https".to_owned());
                }
                let label = match object.get("label") {
                    None | Some(Json::Null) => None,
                    Some(label) => Some(label.as_str().ok_or("a label is a string")?.to_owned()),
                };
                Ok(Self::Reference {
                    url: url.to_owned(),
                    label,
                })
            }
        }
    }

    /// The value as text, for drafts: a text as itself, an option's name
    /// (several joined with `, `), a date as `YYYY-MM-DD`, a subject's
    /// identifier, a reference's label or else its URL.
    // r[impl draft.render]
    #[must_use]
    pub fn display(&self, field: &FieldDef) -> String {
        let name = |key: &str| {
            field
                .options
                .iter()
                .find(|o| o.key == key)
                .map_or_else(|| key.to_owned(), |o| o.name.clone())
        };
        match self {
            Self::Text(text) => text.clone(),
            Self::Choice(key) => name(key),
            Self::Choices(keys) => keys.iter().map(|k| name(k)).collect::<Vec<_>>().join(", "),
            Self::Date(date) => date.format("%Y-%m-%d").to_string(),
            Self::User(subject) => subject.to_string(),
            Self::Reference { url, label } => label.clone().unwrap_or_else(|| url.clone()),
        }
    }

    /// The value's JSON form, as [`Value::from_json`] reads it.
    #[must_use]
    pub fn to_json(&self) -> Json {
        match self {
            Self::Text(text) | Self::Choice(text) => json!(text),
            Self::Choices(keys) => json!(keys),
            Self::Date(date) => json!(date.format("%Y-%m-%d").to_string()),
            Self::User(subject) => json!(subject.to_string()),
            Self::Reference { url, label } => match label {
                Some(label) => json!({ "url": url, "label": label }),
                None => json!({ "url": url }),
            },
        }
    }
}

/// Field values as a JSON object.
#[must_use]
pub fn fields_to_json(fields: &BTreeMap<String, Value>) -> Map<String, Json> {
    fields
        .iter()
        .map(|(key, value)| (key.clone(), value.to_json()))
        .collect()
}

/// Field values read back from a JSON object for `ty`.
pub fn fields_from_json(
    ty: &AktType,
    json: &Map<String, Json>,
) -> Result<BTreeMap<String, Value>, String> {
    json.iter()
        .map(|(key, raw)| {
            let field = ty.field(key).ok_or_else(|| format!("no field `{key}`"))?;
            Value::from_json(field, raw)
                .map(|value| (key.clone(), value))
                .map_err(|reason| format!("`{key}`: {reason}"))
        })
        .collect()
}
