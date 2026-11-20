//! Field values and their JSON form.
//!
//! A value is read either from a request's input ([`Value::from_input`]) or
//! from what was stored ([`Value::from_json`]). They differ only where the
//! server supplies part of a value: a checkbox's input is `true` or `false`,
//! but what is stored is who checked it and when.

use std::collections::BTreeMap;
use std::fmt::Write;

use chrono::{DateTime, NaiveDate, Utc};
use kuria_config::{AktType, FieldDef, FieldKind};
use serde_json::{Map, Value as Json, json};
use uuid::Uuid;

/// A person or piece of software a value names: a user, by their Konklav
/// subject identifier, or a contact of the tenant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Someone {
    User(Uuid),
    Contact(Uuid),
}

impl Someone {
    /// Reads a subject identifier, or `{"contact": <identifier>}`.
    // r[impl contact.reference]
    pub fn from_json(json: &Json) -> Result<Self, String> {
        if let Some(text) = json.as_str() {
            return Uuid::parse_str(text)
                .map(Self::User)
                .map_err(|_| format!("`{text}` is not a subject identifier"));
        }
        let object = members(json, &["contact"])
            .map_err(|_| "expected a subject identifier or {\"contact\": <identifier>}")?;
        object
            .get("contact")
            .and_then(Json::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(Self::Contact)
            .ok_or_else(|| "a contact is named by its identifier".to_owned())
    }

    #[must_use]
    pub fn to_json(&self) -> Json {
        match self {
            Self::User(subject) => json!(subject.to_string()),
            Self::Contact(id) => json!({ "contact": id.to_string() }),
        }
    }

    /// The contact it names, if it names one.
    #[must_use]
    pub fn contact(&self) -> Option<Uuid> {
        match self {
            Self::Contact(id) => Some(*id),
            Self::User(_) => None,
        }
    }
}

/// A field value, of its field's kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Choice(String),
    Choices(Vec<String>),
    Date(NaiveDate),
    User(Someone),
    /// A checked box: who checked it and when, and whom for if not for
    /// themselves.
    Checkbox {
        by: Uuid,
        at: DateTime<Utc>,
        on_behalf_of: Option<Someone>,
    },
    Reference {
        url: String,
        label: Option<String>,
    },
    Git {
        repository: String,
        sha: Option<String>,
        tag: Option<String>,
    },
    Docker {
        repository: String,
        digest: String,
        tag: Option<String>,
    },
    /// An attachment of the akt's tenant, by its identifier.
    Attachment(Uuid),
    /// A `multiple` `user` field's users and contacts, distinct, in order.
    Users(Vec<Someone>),
    /// The akt's members laid out in sections.
    Sections(Vec<crate::sections::Item>),
    /// Each assignee's verdict.
    Verdicts(Vec<crate::verdicts::Given>),
    /// A `markdown` field's text, its headings with ids once written.
    Markdown(String),
}

/// Who makes a change, and when: what the server supplies to values such as
/// a checkbox's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub by: Uuid,
    pub at: DateTime<Utc>,
}

/// What a request's input does to a field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// Set it to this value.
    Set(Value),
    /// Clear it.
    Clear,
}

impl Value {
    /// Reads a request's input for `field`: like [`Value::from_json`],
    /// except that a checkbox takes `true` to check it, recording `change`,
    /// `{"for": <user>}` to check it for someone else, and `false` to clear
    /// it.
    // r[impl akt.field.checkbox]
    pub fn from_input(field: &FieldDef, json: &Json, change: Change) -> Result<Input, String> {
        if json.is_null() {
            return Ok(Input::Clear);
        }
        // r[impl akt.field.multiple]
        if field.multiple && json.as_array().is_some_and(Vec::is_empty) {
            return Ok(Input::Clear);
        }
        // r[impl akt.field.markdown]
        if field.kind == FieldKind::Markdown && json.as_str().is_some_and(crate::markdown::is_empty)
        {
            return Ok(Input::Clear);
        }
        if field.kind == FieldKind::Checkbox {
            let checked = |on_behalf_of: Option<Someone>| {
                Input::Set(Self::Checkbox {
                    by: change.by,
                    at: change.at,
                    // Checking it for oneself is checking it.
                    on_behalf_of: on_behalf_of.filter(|s| *s != Someone::User(change.by)),
                })
            };
            if let Some(checked_now) = json.as_bool() {
                return Ok(if checked_now {
                    checked(None)
                } else {
                    Input::Clear
                });
            }
            let object =
                members(json, &["for"]).map_err(|_| "expected true, false or {\"for\": <user>}")?;
            let on_behalf_of = object.get("for").ok_or("say whom it is checked for")?;
            return Ok(checked(Some(Someone::from_json(on_behalf_of)?)));
        }
        Self::from_json(field, json).map(Input::Set)
    }

    /// Reads a stored value for `field`, or says why it doesn't fit.
    ///
    /// - `text`: a string, without line breaks unless the field is multiline
    /// - `choice`: an option key; `choices`: a list of distinct option keys
    /// - `date`: `YYYY-MM-DD`
    /// - `user`: a subject identifier, or `{"contact": <identifier>}`
    /// - `checkbox`: `{"checked_by": …, "checked_at": …, "for": <user>}`,
    ///   `for` only when checked for someone else
    /// - `reference`: `{"url": "https://…", "label": "…"}`, the label optional
    /// - `git`: `{"repository": …, "sha": …, "tag": …}`, at least one of the
    ///   last two
    /// - `docker`: `{"repository": …, "digest": "sha256:…", "tag": …}`, the
    ///   tag optional
    /// - `attachment`: an attachment identifier; that it exists in the
    ///   tenant is checked by the server
    // r[impl akt.field.kinds]
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
            // r[impl akt.field.multiple]
            FieldKind::User if field.multiple => {
                let items = json
                    .as_array()
                    .ok_or("expected a list of users and contacts")?;
                let mut people = Vec::new();
                for item in items {
                    let someone = Someone::from_json(item)?;
                    if people.contains(&someone) {
                        return Err("someone is named twice".to_owned());
                    }
                    people.push(someone);
                }
                if people.is_empty() {
                    return Err("a list of no one is no value".to_owned());
                }
                Ok(Self::Users(people))
            }
            FieldKind::User => Someone::from_json(json).map(Self::User),
            FieldKind::Sections => crate::sections::from_json(json).map(Self::Sections),
            FieldKind::Verdict => crate::verdicts::all_from_json(json).map(Self::Verdicts),
            // r[impl akt.field.markdown]
            FieldKind::Markdown => {
                let text = json.as_str().ok_or("expected Markdown, as a string")?;
                if crate::markdown::is_empty(text) {
                    return Err("empty Markdown is no value".to_owned());
                }
                Ok(Self::Markdown(text.to_owned()))
            }
            FieldKind::Checkbox => checkbox(json),
            FieldKind::Reference => reference(json),
            FieldKind::Git => git(json),
            FieldKind::Docker => docker(json),
            FieldKind::Attachment => {
                let text = json.as_str().ok_or("expected an attachment identifier")?;
                Uuid::parse_str(text)
                    .map(Self::Attachment)
                    .map_err(|_| format!("`{text}` is not an attachment identifier"))
            }
        }
    }

    /// Everyone the value names: a user field's user or contact, and a
    /// checkbox's checker and whom it was checked for.
    #[must_use]
    pub fn named(&self) -> Vec<Someone> {
        match self {
            Self::User(someone) => vec![*someone],
            Self::Users(people) => people.clone(),
            Self::Verdicts(given) => given
                .iter()
                .flat_map(|g| [g.assignee, Someone::User(g.by)])
                .collect(),
            Self::Checkbox {
                by, on_behalf_of, ..
            } => std::iter::once(Someone::User(*by))
                .chain(*on_behalf_of)
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The value as text, for drafts, showing the people it names by
    /// `person`.
    // r[impl draft.render]
    #[must_use]
    pub fn display(&self, field: &FieldDef, person: &dyn Fn(&Someone) -> String) -> String {
        let name = |key: &str| {
            field
                .options
                .iter()
                .find(|o| o.key == key)
                .map_or_else(|| key.to_owned(), |o| o.name.clone())
        };
        match self {
            Self::Text(text) | Self::Markdown(text) => text.clone(),
            Self::Choice(key) => name(key),
            Self::Choices(keys) => keys.iter().map(|k| name(k)).collect::<Vec<_>>().join(", "),
            Self::Date(date) => date.format("%Y-%m-%d").to_string(),
            Self::User(someone) => person(someone),
            Self::Users(people) => people.iter().map(person).collect::<Vec<_>>().join(", "),
            Self::Sections(items) => {
                let n = crate::sections::numbers(items).len();
                format!("{n} akter in sections")
            }
            Self::Verdicts(given) => given
                .iter()
                .map(|g| match &g.description {
                    Some(why) => format!("{}: {} ({why})", person(&g.assignee), g.verdict.as_str()),
                    None => format!("{}: {}", person(&g.assignee), g.verdict.as_str()),
                })
                .collect::<Vec<_>>()
                .join(", "),
            Self::Attachment(id) => id.to_string(),
            Self::Checkbox {
                on_behalf_of: None, ..
            } => "yes".to_owned(),
            Self::Checkbox {
                by,
                on_behalf_of: Some(someone),
                ..
            } => format!(
                "yes, for {} (checked by {})",
                person(someone),
                person(&Someone::User(*by))
            ),
            Self::Reference { url, label } => label.clone().unwrap_or_else(|| url.clone()),
            Self::Git {
                repository,
                sha,
                tag,
            } => {
                let mut text = repository.clone();
                if let Some(tag) = tag {
                    text.push('@');
                    text.push_str(tag);
                }
                if let Some(sha) = sha {
                    let _ = write!(text, " ({})", &sha[..12]);
                }
                text
            }
            Self::Docker {
                repository,
                digest,
                tag,
            } => match tag {
                Some(tag) => format!("{repository}:{tag}@{digest}"),
                None => format!("{repository}@{digest}"),
            },
        }
    }

    /// The value's JSON form, as [`Value::from_json`] reads it.
    #[must_use]
    pub fn to_json(&self) -> Json {
        let without_nulls = |object: Json| match object {
            Json::Object(map) => {
                Json::Object(map.into_iter().filter(|(_, v)| !v.is_null()).collect())
            }
            other => other,
        };
        match self {
            Self::Text(text) | Self::Choice(text) | Self::Markdown(text) => json!(text),
            Self::Choices(keys) => json!(keys),
            Self::Date(date) => json!(date.format("%Y-%m-%d").to_string()),
            Self::User(someone) => someone.to_json(),
            Self::Users(people) => Json::Array(people.iter().map(Someone::to_json).collect()),
            Self::Sections(items) => crate::sections::to_json(items),
            Self::Verdicts(given) => {
                Json::Array(given.iter().map(crate::verdicts::Given::to_json).collect())
            }
            Self::Attachment(id) => json!(id.to_string()),
            Self::Checkbox {
                by,
                at,
                on_behalf_of,
            } => without_nulls(json!({
                "checked_by": by.to_string(),
                "checked_at": at.to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
                "for": on_behalf_of.as_ref().map(Someone::to_json),
            })),
            Self::Reference { url, label } => without_nulls(json!({ "url": url, "label": label })),
            Self::Git {
                repository,
                sha,
                tag,
            } => without_nulls(json!({ "repository": repository, "sha": sha, "tag": tag })),
            Self::Docker {
                repository,
                digest,
                tag,
            } => without_nulls(json!({ "repository": repository, "digest": digest, "tag": tag })),
        }
    }
}

fn checkbox(json: &Json) -> Result<Value, String> {
    let object = members(json, &["checked_by", "checked_at", "for"])?;
    let by = object
        .get("checked_by")
        .and_then(Json::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or("a checked box needs who checked it")?;
    let at = object
        .get("checked_at")
        .and_then(Json::as_str)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .ok_or("a checked box needs when it was checked")?;
    let on_behalf_of = object.get("for").map(Someone::from_json).transpose()?;
    Ok(Value::Checkbox {
        by,
        at: at.with_timezone(&Utc),
        on_behalf_of,
    })
}

fn reference(json: &Json) -> Result<Value, String> {
    let object = members(json, &["url", "label"])?;
    let url = object
        .get("url")
        .and_then(Json::as_str)
        .ok_or("a reference needs a url")?;
    let parsed = url::Url::parse(url).map_err(|_| format!("`{url}` is not a URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("a reference's url must be http or https".to_owned());
    }
    Ok(Value::Reference {
        url: url.to_owned(),
        label: optional_string(object, "label")?,
    })
}

fn git(json: &Json) -> Result<Value, String> {
    let object = members(json, &["repository", "sha", "tag"])?;
    let repository = object
        .get("repository")
        .and_then(Json::as_str)
        .ok_or("a git reference needs a repository")?;
    check_git_repository(repository)?;
    let sha = optional_string(object, "sha")?;
    if let Some(sha) = &sha
        && !is_hex(sha, &[40, 64])
    {
        return Err(format!(
            "`{sha}` is not a commit SHA: 40 or 64 lowercase hexadecimal digits"
        ));
    }
    let tag = optional_string(object, "tag")?;
    if let Some(tag) = &tag {
        check_git_tag(tag)?;
    }
    if sha.is_none() && tag.is_none() {
        return Err("a git reference needs a sha, a tag, or both".to_owned());
    }
    Ok(Value::Git {
        repository: repository.to_owned(),
        sha,
        tag,
    })
}

fn docker(json: &Json) -> Result<Value, String> {
    let object = members(json, &["repository", "digest", "tag"])?;
    let repository = object
        .get("repository")
        .and_then(Json::as_str)
        .ok_or("an image needs a repository")?;
    check_image_repository(repository)?;
    let digest = object
        .get("digest")
        .and_then(Json::as_str)
        .ok_or("an image needs a digest")?;
    if !digest
        .strip_prefix("sha256:")
        .is_some_and(|hex| is_hex(hex, &[64]))
    {
        return Err(format!(
            "`{digest}` is not a digest: sha256: and 64 lowercase hexadecimal digits"
        ));
    }
    let tag = optional_string(object, "tag")?;
    if let Some(tag) = &tag {
        check_image_tag(tag)?;
    }
    Ok(Value::Docker {
        repository: repository.to_owned(),
        digest: digest.to_owned(),
        tag,
    })
}

/// `json` as an object with only the members `allowed`.
pub(crate) fn members<'a>(
    json: &'a Json,
    allowed: &[&str],
) -> Result<&'a Map<String, Json>, String> {
    let object = json
        .as_object()
        .ok_or_else(|| format!("expected an object with {}", allowed.join(", ")))?;
    if let Some(extra) = object.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(format!("`{extra}` is not one of {}", allowed.join(", ")));
    }
    Ok(object)
}

fn optional_string(object: &Map<String, Json>, key: &str) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| Some(s.to_owned()))
            .ok_or_else(|| format!("`{key}` is a non-empty string")),
    }
}

fn is_hex(text: &str, lengths: &[usize]) -> bool {
    lengths.contains(&text.len())
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// An `https` URL, an `ssh://` URL, or an scp-like `user@host:path`.
fn check_git_repository(repository: &str) -> Result<(), String> {
    let invalid = || {
        format!(
            "`{repository}` is not a repository: an https URL or an ssh remote such as git@host:team/app.git"
        )
    };
    if let Ok(url) = url::Url::parse(repository) {
        return if matches!(url.scheme(), "https" | "ssh") && url.host().is_some() {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    let (user_host, path) = repository.split_once(':').ok_or_else(invalid)?;
    let (user, host) = user_host.split_once('@').ok_or_else(invalid)?;
    let plain = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    if plain(user) && plain(host) && !path.is_empty() && !path.contains(char::is_whitespace) {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// A tag git would accept, simplified: no whitespace, control characters or
/// `~^:?*[\`, no `..`, and not starting with `-` or ending with `.` or `/`.
fn check_git_tag(tag: &str) -> Result<(), String> {
    let bad = tag.chars().any(|c| {
        c.is_whitespace() || c.is_control() || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
    }) || tag.contains("..")
        || tag.starts_with('-')
        || tag.ends_with('.')
        || tag.ends_with('/');
    if bad {
        Err(format!("`{tag}` is not a git tag"))
    } else {
        Ok(())
    }
}

/// An image repository such as `registry.example:5000/team/backend`:
/// lowercase path components of letters, digits and `._-`, after an
/// optional registry host.
fn check_image_repository(repository: &str) -> Result<(), String> {
    let invalid = || format!("`{repository}` is not an image repository");
    let mut parts: Vec<&str> = repository.split('/').collect();
    // A first part with a dot, a colon or `localhost` is a registry host.
    if parts.len() > 1 && (parts[0].contains(['.', ':']) || parts[0] == "localhost") {
        let host = parts.remove(0);
        if host.is_empty() || host.contains(char::is_whitespace) || host.contains('@') {
            return Err(invalid());
        }
    }
    let component = |part: &&str| {
        !part.is_empty()
            && part.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
            })
            && part.starts_with(|c: char| c.is_ascii_alphanumeric())
    };
    if parts.iter().all(component) {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// A tag: up to 128 letters, digits and `_.-`, not starting with `.` or `-`.
fn check_image_tag(tag: &str) -> Result<(), String> {
    let valid = tag.len() <= 128
        && tag.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if valid {
        Ok(())
    } else {
        Err(format!("`{tag}` is not an image tag"))
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
