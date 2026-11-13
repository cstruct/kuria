//! What Kuria's GraphQL would answer for the CLI's reads, built from an akt
//! directory, so that every read renders as it does against a server.

use serde::Serialize;
use serde_json::{Value as Json, json};

use super::dir::{AktDir, LocalAkt, LocalType};
use super::git::Times;
use crate::error::CliError;

/// A config enum as GraphQL names it: `file-text` → `FILE_TEXT`.
fn screaming(value: &impl Serialize) -> Json {
    match serde_json::to_value(value).expect("serializable") {
        Json::String(s) => Json::String(s.replace('-', "_").to_ascii_uppercase()),
        other => other,
    }
}

/// A field's kind as GraphQL names it: `TEXT`, `USER`, ….
#[must_use]
pub fn kind_of(field: &kuria_config::FieldDef) -> String {
    screaming(&field.kind)
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// The akt types, as `aktTypes { key name version numberPattern akterCount }`.
#[must_use]
pub fn types(dir: &AktDir) -> Json {
    dir.types
        .values()
        .map(|t| {
            let count = dir
                .akter
                .iter()
                .filter(|a| a.type_key == t.ty.key())
                .count();
            json!({
                "key": t.ty.key(),
                "name": t.ty.document().name,
                "version": 1,
                "numberPattern": t.ty.document().number,
                "akterCount": count,
            })
        })
        .collect()
}

/// An akt type as the `Shape` fragment and its `document`.
#[must_use]
pub fn shape(t: &LocalType) -> Json {
    let d = t.ty.document();
    json!({
        "key": d.key,
        "version": 1,
        "name": d.name,
        "numberPattern": d.number,
        "fields": d.fields.iter().map(|f| json!({
            "key": f.key,
            "name": f.name,
            "kind": screaming(&f.kind),
            "description": f.description,
            "required": f.required,
            "multiline": f.multiline,
            "options": f.options.iter().map(|o| json!({ "key": o.key, "name": o.name })).collect::<Vec<_>>(),
            "format": f.format.as_ref().map(screaming),
        })).collect::<Vec<_>>(),
        "states": d.states.iter().map(|s| state(t, &s.key)).collect::<Vec<_>>(),
        "transitions": d.transitions.iter().map(|tr| json!({
            "key": tr.key, "name": tr.name, "from": tr.from, "to": tr.to, "requires": tr.requires,
        })).collect::<Vec<_>>(),
        "presentation": {
            "title": t.ty.title_field().map(|f| f.key.clone()),
            "card": d.presentation.card,
            "list": d.presentation.list,
            "icon": d.presentation.icon.as_ref().map(screaming),
            "colour": screaming(&d.presentation.colour.unwrap_or_default()),
        },
        "document": t.document,
    })
}

/// A state as `state { key name initial final tone }`.
fn state(t: &LocalType, key: &str) -> Json {
    let def = t.ty.state(key);
    json!({
        "key": key,
        "name": def.map_or(key, |s| s.name.as_str()),
        "initial": def.is_some_and(|s| s.initial),
        "final": def.is_some_and(|s| s.is_final),
        "tone": screaming(&t.ty.tone(key)),
    })
}

/// Someone a value names locally: text, as a `User` named by it.
// r[impl cli.local.people]
fn someone(text: &str) -> Json {
    json!({
        "__typename": "User",
        "id": text,
        "name": text,
        "email": text.contains('@').then_some(text),
    })
}

/// A field value in the akt file's form as `FieldValue`, or why it doesn't
/// fit the field.
fn value(akt: &LocalAkt, t: &LocalType, key: &str, raw: &Json) -> Result<Option<Json>, CliError> {
    let field =
        t.ty.field(key)
            .expect("checked when the directory was read");
    let bad = |what: &str| {
        CliError::refused(
            "BAD_FILE",
            format!("{}: `{key}` isn't {what}", akt.path.display()),
        )
    };
    let text = || raw.as_str().ok_or_else(|| bad("text"));
    let option = |k: &str| {
        field
            .options
            .iter()
            .find(|o| o.key == k)
            .map(|o| json!({ "key": o.key, "name": o.name }))
            .ok_or_else(|| bad("one of its options' keys"))
    };
    let kind = serde_json::to_value(field.kind).expect("serializable");
    let mut answer = match kind.as_str().unwrap_or_default() {
        "text" => json!({ "__typename": "TextValue", "text": text()? }),
        "choice" => json!({ "__typename": "ChoiceValue", "option": option(text()?)? }),
        "choices" => {
            let keys = raw.as_array().ok_or_else(|| bad("a list of option keys"))?;
            let options = keys
                .iter()
                .map(|k| option(k.as_str().unwrap_or_default()))
                .collect::<Result<Vec<_>, _>>()?;
            json!({ "__typename": "ChoicesValue", "options": options })
        }
        "date" if crate::values::is_date(text()?) => {
            json!({ "__typename": "DateValue", "date": text()? })
        }
        "date" => return Err(bad("a date as YYYY-MM-DD")),
        "user" => json!({ "__typename": "UserValue", "user": someone(text()?) }),
        "checkbox" => {
            let nobody = json!({ "id": "", "name": "", "email": null });
            let checked_for = match raw {
                Json::Bool(false) => return Ok(None),
                Json::Bool(true) => {
                    json!({ "__typename": "User", "id": "", "name": "", "email": null })
                }
                Json::String(s) if s.starts_with("for:") => someone(&s["for:".len()..]),
                _ => return Err(bad("`true`, `false` or `for:<someone>`")),
            };
            json!({ "__typename": "CheckboxValue", "at": "", "by": nobody, "for": checked_for })
        }
        "reference" => {
            let url = raw["url"]
                .as_str()
                .ok_or_else(|| bad("a reference with a `url`"))?;
            json!({ "__typename": "ReferenceValue", "url": url, "label": raw["label"] })
        }
        "git" => {
            let repository = raw["repository"]
                .as_str()
                .ok_or_else(|| bad("a Git value with a `repository`"))?;
            json!({ "__typename": "GitValue", "repository": repository, "sha": raw["sha"], "tag": raw["tag"] })
        }
        "docker" => {
            let repository = raw["repository"]
                .as_str()
                .ok_or_else(|| bad("a Docker value with a `repository`"))?;
            let digest = raw["digest"]
                .as_str()
                .ok_or_else(|| bad("a Docker value with a `digest`"))?;
            json!({ "__typename": "DockerValue", "repository": repository, "digest": digest, "tag": raw["tag"] })
        }
        "attachment" => json!({
            "__typename": "AttachmentValue",
            "attachment": {
                "id": raw["id"].as_str().or_else(|| raw.as_str()).unwrap_or_default(),
                "name": raw["name"].as_str().unwrap_or_default(),
                "mediaType": "", "size": 0, "sha256": "", "url": "",
            },
        }),
        other => return Err(bad(&format!("of a kind ({other}) this client knows"))),
    };
    answer["field"] = json!(key);
    Ok(Some(answer))
}

/// An akt's field values as `fields { ...Values }`, in its type's order.
pub fn values(dir: &AktDir, akt: &LocalAkt) -> Result<Vec<Json>, CliError> {
    let t = dir.type_of(akt);
    let mut answers = Vec::new();
    for field in &t.ty.document().fields {
        if let Some(raw) = akt.values.get(&field.key).filter(|v| !v.is_null())
            && let Some(answer) = value(akt, t, &field.key, raw)?
        {
            answers.push(answer);
        }
    }
    Ok(answers)
}

/// The akt's title: its type's title field's text.
#[must_use]
pub fn title(dir: &AktDir, akt: &LocalAkt) -> Option<String> {
    let field = dir.type_of(akt).ty.title_field()?;
    akt.values.get(&field.key)?.as_str().map(str::to_owned)
}

/// Another akt as `{ number title type { key name } state { key name } }`.
#[must_use]
pub fn other(dir: &AktDir, akt: &LocalAkt) -> Json {
    let t = dir.type_of(akt);
    json!({
        "number": akt.number,
        "title": title(dir, akt),
        "type": { "key": t.ty.key(), "name": t.ty.document().name },
        "state": { "key": akt.state, "name": t.ty.state(&akt.state).map_or(akt.state.as_str(), |s| s.name.as_str()) },
    })
}

/// An akt as the `Row` fragment.
pub fn row(dir: &AktDir, times: &Times, akt: &LocalAkt) -> Result<Json, CliError> {
    let t = dir.type_of(akt);
    let mut answer = other(dir, akt);
    answer["state"]["tone"] = screaming(&t.ty.tone(&akt.state));
    answer["updatedAt"] = json!(times.of_file(&dir.root, &akt.path).1);
    answer["fields"] = json!(values(dir, akt)?);
    Ok(answer)
}

/// A link as the file says it, read back: its name, the other akt's
/// number, and any pinned revision.
fn said(text: &str) -> Option<(&str, &str, Option<i32>)> {
    let (name, target) = text.trim().rsplit_once(' ')?;
    match target.split_once('@') {
        Some((number, revision)) => Some((name, number, revision.parse().ok())),
        None => Some((name, target, None)),
    }
}

/// An akt's links as `links { kind name outgoing pinnedRevision akt pinned }`,
/// read from its file: a name its type gives a kind is a link from it, an
/// inverse name the other akt's type gives one, a link to it.
pub fn links(dir: &AktDir, akt: &LocalAkt) -> Result<Vec<Json>, CliError> {
    let mut answers = Vec::new();
    for text in &akt.links {
        let bad = |why: &str| {
            CliError::refused(
                "BAD_FILE",
                format!("{}: the link `{text}` {why}", akt.path.display()),
            )
        };
        let (name, number, pin) = said(text).ok_or_else(|| bad("isn't `<name> <number>`"))?;
        let target = dir
            .akt(number)
            .ok_or_else(|| bad("names an akt the directory lacks"))?;
        let outgoing = dir
            .type_of(akt)
            .ty
            .document()
            .links
            .iter()
            .find(|l| l.name == name);
        let incoming = dir
            .type_of(target)
            .ty
            .document()
            .links
            .iter()
            .find(|l| l.inverse == name);
        let (kind, outgoing) = match (outgoing, incoming) {
            (Some(def), _) => (def.key.clone(), true),
            (None, Some(def)) => (def.key.clone(), false),
            (None, None) => return Err(bad("is of no kind either akt's type declares")),
        };
        answers.push(json!({
            "kind": kind,
            "name": name,
            "outgoing": outgoing,
            "pinnedRevision": pin,
            "akt": other(dir, target),
            "pinned": null,
        }));
    }
    Ok(answers)
}

/// The akt `number` as the CLI's akt query answers it, with its containers,
/// links, members (in the directory's order) and its type's board.
pub fn akt(dir: &AktDir, times: &Times, number: &str) -> Result<Option<Json>, CliError> {
    let Some(akt) = dir.akt(number) else {
        return Ok(None);
    };
    let t = dir.type_of(akt);
    let (created, updated) = times.of_file(&dir.root, &akt.path);
    let members: Vec<&LocalAkt> = dir.members_of(&akt.number).collect();
    let board = t.ty.board();
    let mut columns: Vec<Json> = board
        .map(|b| {
            b.columns
                .iter()
                .map(|c| json!({ "key": c.key, "name": c.name, "akter": [] }))
                .collect()
        })
        .unwrap_or_default();
    columns.push(json!({ "key": kuria_engine::board::UNMAPPED, "name": "Unmapped", "akter": [] }));
    for member in &members {
        let key = kuria_engine::board::column_of(board, &member.type_key, &member.state);
        if let Some(column) = columns.iter_mut().find(|c| c["key"] == key) {
            column["akter"]
                .as_array_mut()
                .expect("a list")
                .push(other(dir, member));
        }
    }
    let containers: Vec<Json> = akt
        .containers
        .iter()
        .map(|n| {
            dir.akt(n).map(|c| other(dir, c)).ok_or_else(|| {
                CliError::refused(
                    "BAD_FILE",
                    format!(
                        "{}: `in` names `{n}`, which the directory lacks",
                        akt.path.display()
                    ),
                )
            })
        })
        .collect::<Result<_, _>>()?;
    let mut shape = shape(t);
    shape["document"] = t.document.clone();
    Ok(Some(json!({
        "number": akt.number,
        "title": title(dir, akt),
        "revision": akt.revision,
        "createdAt": created,
        "updatedAt": updated,
        "type": shape,
        "state": state(t, &akt.state),
        "fields": values(dir, akt)?,
        "containers": containers,
        "links": links(dir, akt)?,
        "board": { "columns": columns },
        "members": {
            "nodes": members.iter().map(|m| other(dir, m)).collect::<Vec<_>>(),
            "pageInfo": { "endCursor": null, "hasNextPage": false },
        },
    })))
}
