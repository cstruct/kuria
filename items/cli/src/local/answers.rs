//! What Kuria's GraphQL would answer for the CLI's reads, built from an akt
//! directory, so that every read renders as it does against a server.

use std::collections::BTreeMap;

use kuria_config::FieldKind;
use kuria_engine::outcomes::{self, Outcome};
use serde::Serialize;
use serde_json::{Value as Json, json};

use super::dir::{AktDir, LocalAkt, LocalType};
use super::git::Times;
use super::values::{Ids, current};
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
            "multiple": f.multiple,
            "of": f.of,
            "assignees": f.assignees,
            "references": f.references,
        })).collect::<Vec<_>>(),
        "states": d.states.iter().map(|s| state(t, &s.key)).collect::<Vec<_>>(),
        "transitions": d.transitions.iter().map(|tr| json!({
            "key": tr.key, "name": tr.name, "from": tr.from, "to": tr.to, "requires": tr.requires,
            "outcomes": tr.outcomes.iter().map(|o| o.to_ascii_uppercase()).collect::<Vec<_>>(),
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
        "user" if field.multiple => users(raw).ok_or_else(|| bad("a list of people"))?,
        "user" => json!({ "__typename": "UserValue", "user": someone(text()?) }),
        "verdict" => {
            let given = raw
                .as_object()
                .ok_or_else(|| bad("a mapping of assignees to verdicts"))?;
            match verdicts(given) {
                Some(answer) => answer,
                None => return Ok(None),
            }
        }
        "sections" if kuria_engine::sections::from_json(raw).is_ok() => {
            json!({ "__typename": "SectionsValue", "value": raw })
        }
        "sections" => return Err(bad("a list of akt numbers and sections")),
        "markdown" => {
            json!({ "__typename": "MarkdownValue", "markdown": text()?, "references": [] })
        }
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
        // A file of the directory, named by its path (`cli.local.attachments`).
        "attachment" => text()
            .ok()
            .and_then(attachment)
            .ok_or_else(|| bad("the path of a file under attachments/<number>/"))?,
        other => return Err(bad(&format!("of a kind ({other}) this client knows"))),
    };
    answer["field"] = json!(key);
    Ok(Some(answer))
}

/// An attachment, the file of the directory at `path`, as
/// `AttachmentValue`, or `None` if `path` isn't one under
/// `attachments/<number>/` (`cli.local.attachments`).
fn attachment(path: &str) -> Option<Json> {
    super::attachments::holder(path)?;
    Some(json!({
        "__typename": "AttachmentValue",
        "attachment": {
            "id": path,
            "name": path.rsplit('/').next(),
            "mediaType": super::attachments::media_type(path),
            "size": 0, "sha256": "", "url": path,
        },
    }))
}

/// A multiple user field's value in the file's form as `UsersValue`, or
/// `None` if it isn't a list of people.
fn users(raw: &Json) -> Option<Json> {
    let people = raw
        .as_array()?
        .iter()
        .map(|p| p.as_str().map(someone))
        .collect::<Option<Vec<_>>>()?;
    Some(json!({ "__typename": "UsersValue", "users": people }))
}

/// A verdict field's value in the file's form as `VerdictValue`, `None`
/// if it holds none.
fn verdicts(given: &serde_json::Map<String, Json>) -> Option<Json> {
    if given.is_empty() {
        return None;
    }
    let verdicts: Vec<Json> = given
        .iter()
        .map(|(who, verdict)| {
            let by = someone(who);
            // `pass`, or `{verdict, description}`.
            let (verdict, description) = match verdict {
                Json::Object(o) => (&o["verdict"], o.get("description")),
                other => (other, None),
            };
            json!({
                "assignee": by,
                "verdict": verdict.as_str().unwrap_or_default().to_ascii_uppercase(),
                "description": description,
                "by": { "id": by["id"], "name": by["name"], "email": by["email"] },
                "at": "",
            })
        })
        .collect();
    Some(json!({ "__typename": "VerdictValue", "verdicts": verdicts }))
}

/// `akt`'s outcome, `None` if its type has none (`akt.outcome`): from its
/// own verdicts if it has a `verdict` field, else from its members'.
// r[impl akt.outcome]
// r[impl akt.outcome.derived]
pub fn outcome(dir: &AktDir, akt: &LocalAkt) -> Result<Option<Outcome>, CliError> {
    fn walk(
        dir: &AktDir,
        akt: &LocalAkt,
        within: &mut Vec<String>,
    ) -> Result<Option<Outcome>, CliError> {
        let lookup = |key: &str| dir.types.get(key).map(|t| &t.ty);
        // A directory with a cycle is `check`'s to name.
        if within.contains(&akt.number) || !outcomes::has_outcomes(&akt.type_key, &lookup) {
            return Ok(None);
        }
        let ty = &dir.type_of(akt).ty;
        if ty
            .document()
            .fields
            .iter()
            .any(|f| f.kind == FieldKind::Verdict)
        {
            let fields = current(ty, akt, &mut Ids::default())?;
            return Ok(outcomes::of_fields(ty, &fields));
        }
        within.push(akt.number.clone());
        let mut held = Vec::new();
        for member in dir.members_of(&akt.number) {
            held.extend(walk(dir, member, within)?);
        }
        within.pop();
        Ok(Some(outcomes::of_all(held)))
    }
    walk(dir, akt, &mut Vec::new())
}

/// An outcome as GraphQL names it, if there is one.
fn outcome_json(dir: &AktDir, akt: &LocalAkt) -> Result<Json, CliError> {
    Ok(outcome(dir, akt)?.map_or(Json::Null, |o| screaming(&o)))
}

/// An akt's field values as `fields { ...Values }`, in its type's order.
pub fn values(dir: &AktDir, akt: &LocalAkt) -> Result<Vec<Json>, CliError> {
    let t = dir.type_of(akt);
    let mut answers = Vec::new();
    for field in &t.ty.document().fields {
        if let Some(raw) = akt.values.get(&field.key).filter(|v| !v.is_null())
            && let Some(mut answer) = value(akt, t, &field.key, raw)?
        {
            if answer["__typename"] == "MarkdownValue" {
                answer["references"] = references(dir, raw.as_str().unwrap_or_default()).into();
            }
            answers.push(answer);
        }
    }
    Ok(answers)
}

/// An akt's `markdown` fields as the engine holds them, read from its file,
/// leaving out any other field.
#[must_use]
pub fn markdown_of(dir: &AktDir, akt: &LocalAkt) -> BTreeMap<String, kuria_engine::Value> {
    dir.type_of(akt)
        .ty
        .document()
        .fields
        .iter()
        .filter(|f| f.kind == FieldKind::Markdown)
        .filter_map(|f| {
            let text = akt.values.get(&f.key)?.as_str()?;
            Some((
                f.key.clone(),
                kuria_engine::Value::Markdown(text.to_owned()),
            ))
        })
        .collect()
}

/// The references in `text`, each with why it is broken, if it is, among
/// the directory's akter (`akt.field.markdown.references`).
// r[impl akt.field.markdown.references]
fn references(dir: &AktDir, text: &str) -> Vec<Json> {
    use kuria_engine::markdown;
    markdown::references(text)
        .iter()
        .map(|reference| {
            let target = super::versions::target(dir, &reference.number);
            json!({
                "destination": reference.to_string(),
                "broken": markdown::broken(reference, target.as_ref()),
            })
        })
        .collect()
}

/// The links references keep, to and from `akt`, worked out from the
/// directory's `markdown` fields: they are never stored
/// (`cli.local.references`).
// r[impl cli.local.references]
// r[impl link.referenced]
fn reference_links(dir: &AktDir, akt: &LocalAkt) -> Vec<Json> {
    let kept_by = |source: &LocalAkt| {
        let ty = &dir.type_of(source).ty;
        kuria_engine::markdown::referenced(ty, &markdown_of(dir, source))
            .into_iter()
            .filter_map(|(kind, numbers)| {
                let def = ty.link(&kind)?.clone();
                Some((def, numbers))
            })
            .collect::<Vec<_>>()
    };
    let fits = |def: &kuria_config::LinkDef, source: &LocalAkt, target: &LocalAkt| {
        source.number != target.number && def.to.contains(&target.type_key)
    };
    let link = |def: &kuria_config::LinkDef, outgoing: bool, other_akt: &LocalAkt| {
        json!({
            "kind": def.key,
            "name": if outgoing { &def.name } else { &def.inverse },
            "outgoing": outgoing,
            "pinnedRevision": null,
            "byReference": true,
            "akt": other(dir, other_akt),
            "pinned": null,
        })
    };
    let mut answers = Vec::new();
    for (def, numbers) in kept_by(akt) {
        for target in numbers.iter().filter_map(|n| dir.any(n)) {
            if fits(&def, akt, target) {
                answers.push(link(&def, true, target));
            }
        }
    }
    for source in &dir.akter {
        for (def, numbers) in kept_by(source) {
            if numbers.contains(&akt.number) && fits(&def, source, akt) {
                answers.push(link(&def, false, source));
            }
        }
    }
    answers
}

/// The akt's title: its type's title field's text.
#[must_use]
pub fn title(dir: &AktDir, akt: &LocalAkt) -> Option<String> {
    let field = dir.type_of(akt).ty.title_field()?;
    akt.values.get(&field.key)?.as_str().map(str::to_owned)
}

/// Another akt as `{ number title type { key name } state { key name }
/// archived }`.
#[must_use]
pub fn other(dir: &AktDir, akt: &LocalAkt) -> Json {
    let t = dir.type_of(akt);
    json!({
        "number": akt.number,
        "title": title(dir, akt),
        "type": { "key": t.ty.key(), "name": t.ty.document().name },
        "state": { "key": akt.state, "name": t.ty.state(&akt.state).map_or(akt.state.as_str(), |s| s.name.as_str()) },
        "archived": archived(dir, &akt.number),
    })
}

/// How the akt `number` is archived, as `archived { at by rule reason }`;
/// `null` while it is live (`cli.local.archive`).
// r[impl cli.local.archive]
#[must_use]
pub fn archived(dir: &AktDir, number: &str) -> Json {
    dir.archive
        .entry(number)
        .map_or(Json::Null, super::archive::Entry::json)
}

/// The akt `number` as another names it, live or archived; an archived
/// one neither git nor `akter/archived/` has is named by its number and
/// type only.
#[must_use]
pub fn named(dir: &AktDir, number: &str) -> Option<Json> {
    if let Some(akt) = dir.any(number) {
        return Some(other(dir, akt));
    }
    let entry = dir.archive.entry(number)?;
    let name = dir
        .types
        .get(&entry.type_key)
        .map_or(entry.type_key.as_str(), |t| t.ty.document().name.as_str());
    Some(json!({
        "number": number,
        "title": null,
        "type": { "key": entry.type_key, "name": name },
        "state": { "key": null, "name": "archived" },
        "archived": entry.json(),
    }))
}

/// When `akt` last changed: when it was archived, if it is, else as git
/// says.
#[must_use]
pub fn updated(dir: &AktDir, times: &Times, akt: &LocalAkt) -> String {
    match dir.archive.entry(&akt.number) {
        Some(entry) => entry.at.clone(),
        None => times.of_akt(akt).1,
    }
}

/// An akt as the `Row` fragment.
pub fn row(dir: &AktDir, times: &Times, akt: &LocalAkt) -> Result<Json, CliError> {
    let t = dir.type_of(akt);
    let mut answer = other(dir, akt);
    answer["state"]["tone"] = screaming(&t.ty.tone(&akt.state));
    answer["updatedAt"] = json!(updated(dir, times, akt));
    answer["fields"] = json!(values(dir, akt)?);
    answer["outcome"] = outcome_json(dir, akt)?;
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
        let Some(target) = dir.any(number) else {
            // An archived akt neither git nor `akter/archived/` has, named
            // by its own type's link only.
            let def = dir
                .type_of(akt)
                .ty
                .document()
                .links
                .iter()
                .find(|l| l.name == name);
            match (def, named(dir, number)) {
                (Some(def), Some(other)) => answers.push(json!({
                    "kind": def.key, "name": name, "outgoing": true,
                    "pinnedRevision": pin, "akt": other, "pinned": null,
                })),
                (None, Some(_)) => {}
                _ => return Err(bad("names an akt the directory lacks")),
            }
            continue;
        };
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
    answers.extend(reference_links(dir, akt));
    Ok(answers)
}

/// The akt `number`, live or archived, as the CLI's akt query answers it,
/// with its containers, links, members `shown` allows (in the
/// directory's order) and its type's board; an archived container's board
/// shows every member unless asked otherwise (`archive.board`).
// r[impl cli.local.archive]
pub fn akt(
    dir: &AktDir,
    times: &Times,
    number: &str,
    shown: crate::reads::Archived,
) -> Result<Option<Json>, CliError> {
    let Some(akt) = dir.any(number) else {
        return Ok(None);
    };
    let t = dir.type_of(akt);
    let gone = dir.archive.entry(number);
    let (created, updated) = match gone {
        Some(entry) => (
            times
                .history(akt)
                .and_then(|(commits, _)| commits.first().map(|(c, _)| c.at.clone()))
                .unwrap_or_else(|| entry.at.clone()),
            entry.at.clone(),
        ),
        None => times.of_akt(akt),
    };
    let shown = match shown {
        crate::reads::Archived::Live if gone.is_some() => crate::reads::Archived::All,
        asked => asked,
    };
    let members: Vec<&LocalAkt> = dir.members_with(&akt.number, shown).collect();
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
            let mut shown = other(dir, member);
            shown["outcome"] = outcome_json(dir, member)?;
            column["akter"].as_array_mut().expect("a list").push(shown);
        }
    }
    let containers: Vec<Json> = akt
        .containers
        .iter()
        .map(|n| {
            named(dir, n).ok_or_else(|| {
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
        "outcome": outcome_json(dir, akt)?,
        "archived": archived(dir, number),
        "fields": values(dir, akt)?,
        "containers": containers,
        "links": links(dir, akt)?,
        "version": null,
        "versions": super::versions::listed(akt),
        "board": { "columns": columns },
        "members": {
            "nodes": members
                .iter()
                .map(|m| {
                    let mut shown = other(dir, m);
                    shown["outcome"] = outcome_json(dir, m)?;
                    Ok(shown)
                })
                .collect::<Result<Vec<_>, CliError>>()?,
            "pageInfo": { "endCursor": null, "hasNextPage": false },
        },
    })))
}

/// The akt `number` as it was at its version `n`, as the CLI's akt query
/// answers it with `version:`, its references following the version's
/// pins; `None` if it has no such version (`version.read`).
// r[impl version.read]
pub fn akt_at(dir: &AktDir, times: &Times, number: &str, n: i64) -> Result<Option<Json>, CliError> {
    let Some((read, version)) = super::versions::at(dir, times, number, n)? else {
        return Ok(None);
    };
    let pins = super::versions::pins_of(&version);
    let mut at = AktDir {
        root: dir.root.clone(),
        types: dir.types.clone(),
        akter: dir.akter.clone(),
        archive: dir.archive.clone(),
        archived: dir.archived.clone(),
    };
    if let Some(slot) = at
        .akter
        .iter_mut()
        .chain(at.archived.iter_mut())
        .find(|a| a.number == number)
    {
        *slot = read;
    }
    let Some(mut answer) = akt(&at, times, number, crate::reads::Archived::Live)? else {
        return Ok(None);
    };
    answer["version"] = super::versions::shown(&version);
    // Each reference naming no version, as its pin says.
    if let Some(fields) = answer["fields"].as_array_mut() {
        for field in fields
            .iter_mut()
            .filter(|f| f["__typename"] == "MarkdownValue")
        {
            let text = field["markdown"].as_str().unwrap_or_default().to_owned();
            field["references"] = pinned_references(dir, &text, &pins).into();
        }
    }
    Ok(Some(answer))
}

/// The references in `text` as a version reads them, following `pins`.
fn pinned_references(dir: &AktDir, text: &str, pins: &[kuria_engine::versions::Pin]) -> Vec<Json> {
    use kuria_engine::markdown;
    markdown::references(text)
        .iter()
        .map(|reference| {
            let (version, revision) = kuria_engine::versions::as_pinned(reference, pins);
            let target = super::versions::target(dir, &reference.number);
            let broken = if revision.is_some() {
                target
                    .is_none()
                    .then(|| format!("there is no akt `{}`", reference.number))
            } else {
                let as_read = markdown::Reference {
                    version: reference.version.or(version),
                    ..reference.clone()
                };
                markdown::broken(&as_read, target.as_ref())
            };
            json!({
                "destination": reference.to_string(),
                "broken": broken,
                "pinnedVersion": version.filter(|_| reference.version.is_none()),
                "pinnedRevision": revision,
            })
        })
        .collect()
}
