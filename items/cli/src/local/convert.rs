//! Converting between a server and an akt directory (`cli.local.convert`),
//! by way of a bundle (`bundle.format`).
//!
//! - [`to_dir`]: a bundle of selected akt types with their akter, written
//!   as a new akt directory: each type's latest document, each akt as it
//!   is now. Its revisions stay with the server; git takes over from here.
//! - [`bundle_of`]: an akt directory as a bundle the server imports. A
//!   directory holds no revisions, so each akt gets revisions numbered up
//!   to its file's `revision`: the first records it as it is, and the rest
//!   change nothing. Every hash then matches (`bundle.import.hashes`), and
//!   pins the files hold name revisions the server has.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use chrono::{DateTime, Utc};
use kuria_config::{AktTypeDocument, validate};
use kuria_engine::{Value, fields_from_json, fields_to_json, state_hash};
use serde_json::{Map, Value as Json, json};
use uuid::Uuid;

use super::answers;
use super::check::{check, said};
use super::dir::{AktDir, LocalAkt, LocalType, init};
use super::git::Times;
use super::layout;
use super::values::{Ids, NAMESPACE, current, file_form};
use crate::error::CliError;
use crate::file::Parsed;
use crate::profile::io_error;

/// The bundle format this client writes.
const BUNDLE_FORMAT: u32 = 1;

fn doesnt_fit(number: &str, key: &str, why: impl std::fmt::Display) -> CliError {
    CliError::refused(
        "NOT_LATEST",
        format!(
            "{number} doesn't fit the latest document of `{key}`, which an akt directory keeps: {why}; change it on the server first"
        ),
    )
}

/// The akt type a bundle's version `definition` defines.
fn local_type(definition: &Json) -> Result<LocalType, CliError> {
    let invalid = |why: String| CliError::unavailable(format!("Kuria's bundle: {why}"));
    let document: AktTypeDocument =
        serde_json::from_value(definition.clone()).map_err(|e| invalid(e.to_string()))?;
    let ty = validate(document).map_err(|p| {
        invalid(
            p.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        )
    })?;
    let document = serde_json::to_value(ty.document()).expect("serializable");
    Ok(LocalType { ty, document })
}

fn uuid(json: &Json) -> Uuid {
    json.as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_default()
}

/// Who a bundle names, by the text an akt directory names them by.
fn named(bundle: &Json) -> Ids {
    let mut ids = Ids::new(None);
    for p in bundle["people"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(bundle["contacts"].as_array().into_iter().flatten())
    {
        let id = if p["subject"].is_null() {
            uuid(&p["id"])
        } else {
            uuid(&p["subject"])
        };
        if let Some(text) = p["email"].as_str().or_else(|| p["name"].as_str()) {
            ids.name(id, text);
        }
    }
    ids
}

/// Writes `bundle`, selected akt types with their akter, as a new akt
/// directory at `root`, and answers what it wrote.
// r[impl cli.local.convert]
pub fn to_dir(bundle: &Json, root: &Path) -> Result<Json, CliError> {
    if bundle["scope"] != "types" || bundle["holdsAkter"] != true {
        return Err(CliError::unavailable(
            "Kuria's bundle doesn't hold selected akt types with their akter",
        ));
    }
    let mut types = BTreeMap::new();
    let mut version_keys = HashMap::new();
    for t in bundle["aktTypes"].as_array().into_iter().flatten() {
        let versions = t["versions"].as_array().cloned().unwrap_or_default();
        let latest = versions
            .last()
            .ok_or_else(|| CliError::unavailable("Kuria's bundle: an akt type without versions"))?;
        let key = t["key"].as_str().unwrap_or_default().to_owned();
        for v in &versions {
            version_keys.insert(uuid(&v["id"]), key.clone());
        }
        types.insert(key, local_type(&latest["definition"])?);
    }
    let akter_in = bundle["akter"].as_array().cloned().unwrap_or_default();
    let mut bundled = HashMap::new();
    for a in &akter_in {
        let number = a["number"].as_str().unwrap_or_default().to_owned();
        let type_key = version_keys
            .get(&uuid(&a["typeVersion"]))
            .cloned()
            .ok_or_else(|| {
                CliError::unavailable(format!("Kuria's bundle: {number} has no akt type"))
            })?;
        bundled.insert(
            uuid(&a["id"]),
            Bundled {
                number,
                type_key,
                rank: a["rank"].as_str().unwrap_or_default().to_owned(),
            },
        );
    }
    let ids = named(bundle);
    let mut akter = Vec::new();
    for a in &akter_in {
        akter.push(akt_of(bundle, a, root, &types, &bundled, &ids)?);
    }
    akter.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.number.cmp(&b.number)));
    let dir = AktDir {
        root: root.to_owned(),
        types,
        akter,
    };

    init(root)?;
    for (key, t) in &dir.types {
        let path = root.join("types").join(format!("{key}.json"));
        let text = serde_json::to_string_pretty(&t.document).expect("serializable") + "\n";
        std::fs::write(&path, text).map_err(|e| io_error(&path, &e))?;
    }
    let mut dir = dir;
    let every: BTreeSet<_> = dir.akter.iter().map(|a| a.path.clone()).collect();
    for path in layout::relocate(&mut dir, &every) {
        std::fs::write(&path, layout::file_text(&dir, &path)).map_err(|e| io_error(&path, &e))?;
    }
    // What was written reads back as a consistent directory.
    check(root)?;
    Ok(json!({
        "dir": root,
        "aktTypes": dir.types.len(),
        "akter": dir.akter.len(),
    }))
}

/// What a bundle says of one of its akter.
struct Bundled {
    number: String,
    type_key: String,
    rank: String,
}

/// The bundle's akt `a` as an akt of the directory.
fn akt_of(
    bundle: &Json,
    a: &Json,
    root: &Path,
    types: &BTreeMap<String, LocalType>,
    bundled: &HashMap<Uuid, Bundled>,
    ids: &Ids,
) -> Result<LocalAkt, CliError> {
    let id = uuid(&a["id"]);
    let Bundled {
        number,
        type_key: key,
        rank,
    } = &bundled[&id];
    let number = number.clone();
    let ty = &types[key].ty;
    let state = a["state"].as_str().unwrap_or_default().to_owned();
    if ty.state(&state).is_none() {
        return Err(doesnt_fit(
            &number,
            key,
            format!("it has no state `{state}`"),
        ));
    }
    let fields = fields_from_json(ty, a["fields"].as_object().unwrap_or(&Map::new()))
        .map_err(|e| doesnt_fit(&number, key, e))?;
    let mut values = Map::new();
    for (field, value) in &fields {
        if matches!(value, Value::Attachment(_)) {
            return Err(super::unsupported(
                &format!("{number}'s attachment in `{field}`"),
                "an akt directory holds no attachments",
            ));
        }
        values.insert(field.clone(), file_form(value, ids));
    }
    let containers = bundle["memberships"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| uuid(&m["member"]) == id)
        .filter_map(|m| {
            bundled
                .get(&uuid(&m["container"]))
                .map(|c| c.number.clone())
        })
        .collect();
    let (mut links, mut incoming) = (Vec::new(), Vec::new());
    for l in bundle["links"].as_array().into_iter().flatten() {
        let (source, target) = (uuid(&l["source"]), uuid(&l["target"]));
        if source != id && target != id {
            continue;
        }
        let kind = l["kind"].as_str().unwrap_or_default();
        let source_key = &bundled[&source].type_key;
        let def = types
            .get(source_key)
            .and_then(|t| t.ty.document().links.iter().find(|d| d.key == kind))
            .ok_or_else(|| doesnt_fit(&number, source_key, format!("it has no link `{kind}`")))?;
        let at = l["pinnedRevision"]
            .as_i64()
            .map(|r| format!("@{r}"))
            .unwrap_or_default();
        if source == id {
            let other = &bundled[&target];
            links.push((
                kind,
                &other.rank,
                format!("{} {}{at}", def.name, other.number),
            ));
        } else {
            let other = &bundled[&source];
            incoming.push((
                kind,
                &other.rank,
                format!("{} {}{at}", def.inverse, other.number),
            ));
        }
    }
    // Its own links first, then those to it, each by kind and the other's
    // place, as a server lists them.
    links.sort();
    incoming.sort();
    let links = links
        .into_iter()
        .chain(incoming)
        .map(|(_, _, l)| l)
        .collect();
    Ok(LocalAkt {
        path: root.join("akter").join(format!("{number}.md")),
        number,
        type_key: key.clone(),
        state,
        revision: a["revisions"]
            .as_array()
            .and_then(|r| r.last())
            .and_then(|r| r["revision"].as_i64())
            .and_then(|r| i32::try_from(r).ok())
            .unwrap_or(1),
        rank: rank.clone(),
        containers,
        links,
        values,
        parsed: Parsed {
            front: Map::new(),
            sections: Vec::new(),
        },
        nested: false,
        versions: Vec::new(),
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// A stable identifier for what `what` names in the directory.
fn id_of(what: &str) -> Uuid {
    Uuid::new_v5(&NAMESPACE, what.as_bytes())
}

fn time(text: &str, or: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text).map_or(or, |t| t.with_timezone(&Utc))
}

/// `akt`'s revisions in a bundle: numbered up to its file's, the first
/// recording `fields` as they are and the rest changing nothing, each with
/// the hash of what it results in.
fn revisions(
    akt: &LocalAkt,
    fields: &BTreeMap<String, Value>,
    author: Uuid,
    (created, updated): (DateTime<Utc>, DateTime<Utc>),
) -> Vec<Json> {
    let hash = hex(&state_hash(&akt.state, fields));
    (1..=akt.revision)
        .map(|revision| {
            let first = revision == 1;
            json!({
                "revision": revision,
                "author": author,
                "transition": null,
                "reason": first.then_some("From an akt directory, whose history git holds"),
                "changes": if first { Json::Object(fields_to_json(fields)) } else { json!({}) },
                "state": akt.state,
                "hash": hash,
                "createdAt": if first { created } else { updated },
                "membership": null,
            })
        })
        .collect()
}

/// The akt directory at `root` as a bundle of its akt types with their
/// akter, made by `me`, once `kuria check` finds it consistent.
// r[impl cli.local.convert]
pub fn bundle_of(root: &Path, me: Option<String>) -> Result<Json, CliError> {
    check(root)?;
    let dir = AktDir::open(root)?;
    let times = Times::of(root);
    let now = Utc::now();
    let mut ids = Ids::new(me);
    let author = ids.changer();

    let akt_types: Vec<Json> = dir
        .types
        .iter()
        .map(|(key, t)| {
            json!({
                "key": key,
                "versions": [{
                    "id": id_of(&format!("type:{key}")),
                    "version": 1,
                    "definition": t.document,
                    "createdBy": author,
                    "createdAt": now,
                }],
            })
        })
        .collect();
    let mut akter = Vec::new();
    let mut memberships = Vec::new();
    let mut links = Vec::new();
    for akt in &dir.akter {
        let t = dir.type_of(akt);
        let (created, updated) = times.of_akt(akt);
        let (created, updated) = (time(&created, now), time(&updated, now));
        let mut fields = current(&t.ty, akt, &mut ids)?;
        for value in fields.values_mut() {
            if let Value::Checkbox { at, .. } = value {
                *at = updated;
            }
        }
        let revisions = revisions(akt, &fields, author, (created, updated));
        let id = id_of(&format!("akt:{}", akt.number));
        akter.push(json!({
            "id": id,
            "number": akt.number,
            "typeVersion": id_of(&format!("type:{}", akt.type_key)),
            "state": akt.state,
            "fields": fields_to_json(&fields),
            "title": answers::title(&dir, akt),
            "rank": akt.rank,
            "createdAt": created,
            "updatedAt": updated,
            "revisions": revisions,
        }));
        for container in &akt.containers {
            memberships.push(json!({
                "container": id_of(&format!("akt:{container}")),
                "member": id,
                "addedAt": updated,
                "addedBy": author,
            }));
        }
        // Each link once, from the akt whose type declares it.
        for text in &akt.links {
            let Some((name, target, pin)) = said(text) else {
                continue;
            };
            if let Some(def) = t.ty.document().links.iter().find(|l| l.name == name) {
                links.push(json!({
                    "source": id,
                    "kind": def.key,
                    "target": id_of(&format!("akt:{target}")),
                    "pinnedRevision": pin,
                    "addedAt": updated,
                    "addedBy": author,
                }));
            }
        }
    }
    let people: Vec<Json> = ids
        .known()
        .map(|(subject, text)| {
            let email = text.contains('@').then_some(text);
            json!({
                "subject": subject,
                "name": if email.is_some() { None } else { Some(text) },
                "email": email,
            })
        })
        .collect();
    Ok(json!({
        "format": BUNDLE_FORMAT,
        "scope": "types",
        "types": dir.types.keys().collect::<Vec<_>>(),
        "holdsAkter": true,
        "exportedAt": now,
        "aktTypes": akt_types,
        "akter": akter,
        "memberships": memberships,
        "links": links,
        "people": people,
    }))
}
