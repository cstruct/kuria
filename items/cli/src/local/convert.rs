//! Converting between a server and an akt directory (`cli.local.convert`),
//! by way of a bundle (`bundle.format`).
//!
//! - [`to_dir`]: a bundle of selected akt types with their akter, written
//!   as a new akt directory: each type's latest document, each akt as it
//!   is now, with its versions. Its revisions stay with the server; git
//!   takes over from here.
//! - [`bundle_of`]: an akt directory as a bundle the server imports. A
//!   directory holds no revisions, so each akt gets revisions numbered up
//!   to its file's `revision`, each recording it as git's latest commit at
//!   that revision holds it, else as at the next revision git has. Every
//!   hash then matches (`bundle.import.hashes`), pins the files hold name
//!   revisions the server has, and each version reads as it was released.
//!
//! Attachments go both ways as files: a bundle's bytes written under
//! `attachments/<number>/`, and each file an akt holds, at any revision,
//! put in the bundle (`cli.local.attachments`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use chrono::{DateTime, Utc};
use kuria_config::{AktType, AktTypeDocument, validate};
use kuria_engine::{Value, fields_from_json, fields_to_json, state_hash};
use serde_json::{Map, Value as Json, json};
use uuid::Uuid;

use super::answers;
use super::archive::{Archive, Entry, kept_path};
use super::attachments;
use super::check::{check, said};
use super::dir::{AktDir, LocalAkt, LocalType, init, read_text};
use super::git::Times;
use super::layout;
use super::values::{Ids, NAMESPACE, current, file_form};
use crate::error::CliError;
use crate::file::Parsed;
use crate::profile::io_error;

/// The bundle format this client writes.
const BUNDLE_FORMAT: u32 = 4;

/// A bundle unpacked: its `bundle.json`, and each attachment's bytes by
/// their SHA-256 in lower-case hex, as `attachments/<sha256>` holds them.
#[derive(Debug, Default)]
pub struct Unpacked {
    pub bundle: Json,
    pub attachments: Bytes,
}

impl Unpacked {
    /// The bundle of the files a tar archive of one holds, by their names.
    #[must_use]
    pub fn of(bundle: Json, files: &[(String, Vec<u8>)]) -> Self {
        let attachments = files
            .iter()
            .filter_map(|(name, bytes)| {
                let sha = name.strip_prefix("attachments/")?;
                Some((sha.to_ascii_lowercase(), bytes.clone()))
            })
            .collect();
        Self {
            bundle,
            attachments,
        }
    }

    /// Its files as a tar archive of it holds them, but `bundle.json`.
    #[must_use]
    pub fn files(&self) -> Vec<(String, Vec<u8>)> {
        self.attachments
            .iter()
            .map(|(sha, bytes)| (format!("attachments/{sha}"), bytes.clone()))
            .collect()
    }
}

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

/// Writes `unpacked`, selected akt types with their akter, as a new akt
/// directory at `root`, each attachment as its file, and answers what it
/// wrote.
// r[impl cli.local.convert]
// r[impl cli.local.attachments]
pub fn to_dir(unpacked: &Unpacked, root: &Path) -> Result<Json, CliError> {
    let bundle = &unpacked.bundle;
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
    let mut files = Files {
        unpacked,
        written: BTreeMap::new(),
    };
    let mut akter = Vec::new();
    let mut archive = Archive::default();
    let mut archived = Vec::new();
    for a in &akter_in {
        let mut akt = akt_of(bundle, a, root, &types, &bundled, &ids, &mut files)?;
        // r[impl cli.local.archive]
        match archived_of(a, &bundled, &ids) {
            Some(entry) => {
                akt.revision = entry.revision;
                archive.archived.push(entry);
                archived.push(akt);
            }
            None => {
                if a["archiveKept"] == true {
                    archive.kept.push(akt.number.clone());
                }
                akter.push(akt);
            }
        }
    }
    akter.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.number.cmp(&b.number)));
    let dir = AktDir {
        root: root.to_owned(),
        types,
        akter,
        archive,
        archived,
    };

    init(root)?;
    for (key, t) in &dir.types {
        let path = root.join("types").join(format!("{key}.json"));
        let text = serde_json::to_string_pretty(&t.document).expect("serializable") + "\n";
        std::fs::write(&path, text).map_err(|e| io_error(&path, &e))?;
    }
    for (path, bytes) in &files.written {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_error(parent, &e))?;
        }
        std::fs::write(&path, bytes).map_err(|e| io_error(&path, &e))?;
    }
    let mut dir = dir;
    let every: BTreeSet<_> = dir.akter.iter().map(|a| a.path.clone()).collect();
    for path in layout::relocate(&mut dir, &every) {
        std::fs::write(&path, layout::file_text(&dir, &path)).map_err(|e| io_error(&path, &e))?;
    }
    write_archived(&dir, root)?;
    // What was written reads back as a consistent directory.
    check(root)?;
    Ok(json!({
        "dir": root,
        "aktTypes": dir.types.len(),
        "akter": dir.akter.len(),
        "archived": dir.archived.len(),
        "attachments": files.written.len(),
    }))
}

/// The archived akter of `dir`, which no git history holds yet, each as
/// its own file in `akter/archived/`, and `akter/archived.md`
/// (`cli.local.archive`).
// r[impl cli.local.archive.kept]
// r[impl cli.local.archive]
fn write_archived(dir: &AktDir, root: &Path) -> Result<(), CliError> {
    for akt in &dir.archived {
        let path = kept_path(root, &akt.number);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_error(parent, &e))?;
        }
        std::fs::write(&path, super::write::text_of(dir, akt, true))
            .map_err(|e| io_error(&path, &e))?;
    }
    if dir.archive != Archive::default() {
        let path = Archive::path(root);
        std::fs::write(&path, dir.archive.text()).map_err(|e| io_error(&path, &e))?;
    }
    Ok(())
}

/// How the bundle's akt `a` is archived, as `akter/archived.md` lists it:
/// at the revision before the one its archiving appended, which the akt's
/// file in `akter/archived/` holds it at.
fn archived_of(a: &Json, bundled: &HashMap<Uuid, Bundled>, ids: &Ids) -> Option<Entry> {
    let archived = a["archived"].as_object()?;
    let id = uuid(&a["id"]);
    let revisions = a["revisions"].as_array().cloned().unwrap_or_default();
    let last = revisions
        .last()
        .and_then(|r| r["revision"].as_i64())
        .and_then(|r| i32::try_from(r).ok())
        .unwrap_or(1);
    let appended = revisions
        .last()
        .is_some_and(|r| r["transition"] == "archive");
    let by = &archived["by"];
    let rule = by["rule"].as_str().map(str::to_owned);
    Some(Entry {
        number: bundled[&id].number.clone(),
        type_key: bundled[&id].type_key.clone(),
        revision: if appended && last > 1 { last - 1 } else { last },
        at: archived["at"].as_str().map_or_else(String::new, |t| {
            time(t, Utc::now()).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        }),
        by: match (&rule, by["subject"].as_str()) {
            (None, Some(_)) => Some(ids.text(uuid(&by["subject"]))),
            (None, None) => Some("someone".to_owned()),
            _ => None,
        },
        container: by["container"]
            .as_str()
            .and_then(|_| bundled.get(&uuid(&by["container"])))
            .map(|c| c.number.clone()),
        rule,
        reason: archived
            .get("reason")
            .and_then(Json::as_str)
            .map(str::to_owned),
    })
}

/// The attachments a directory is written with: a bundle's, and the files
/// they are written as, by path.
struct Files<'a> {
    unpacked: &'a Unpacked,
    written: BTreeMap<String, Vec<u8>>,
}

impl Files<'_> {
    /// The path the bundle's attachment `id`, held by `number`, is written
    /// at: `attachments/<number>/<name>`, under a name of its own if
    /// another of the akt's attachments has its name.
    fn place(&mut self, number: &str, id: Uuid) -> Result<String, CliError> {
        let missing = || {
            CliError::unavailable(format!(
                "Kuria's bundle lacks the attachment {id}, which {number} holds"
            ))
        };
        let meta = self.unpacked.bundle["attachments"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|a| uuid(&a["id"]) == id)
            .ok_or_else(missing)?;
        let bytes = meta["sha256"]
            .as_str()
            .and_then(|sha| self.unpacked.attachments.get(&sha.to_ascii_lowercase()))
            .ok_or_else(missing)?;
        let name = attachments::file_name(
            meta["name"].as_str().unwrap_or_default(),
            meta["mediaType"].as_str().unwrap_or_default(),
        );
        for n in 1.. {
            let path = format!(
                "{}/{number}/{}",
                attachments::DIRECTORY,
                attachments::numbered(&name, n)
            );
            match self.written.get(&path) {
                Some(there) if there != bytes => {}
                Some(_) => return Ok(path),
                None => {
                    self.written.insert(path.clone(), bytes.clone());
                    return Ok(path);
                }
            }
        }
        unreachable!("a name is found")
    }
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
    files: &mut Files<'_>,
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
        let value = match value {
            Value::Attachment(id) => files.place(&number, *id)?.into(),
            value => file_form(value, ids),
        };
        values.insert(field.clone(), value);
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
        versions: versions_in(a, ty, ids),
    })
}

/// The bundle's akt `a`'s versions as a directory's file keeps them
/// (`cli.local.versions`), each with the headings it had at its revision,
/// replayed from the akt's revisions.
fn versions_in(a: &Json, ty: &AktType, ids: &Ids) -> Vec<Json> {
    let versions = a["versions"].as_array().cloned().unwrap_or_default();
    let named: BTreeSet<i64> = versions
        .iter()
        .filter_map(|v| v["revision"].as_i64())
        .collect();
    let mut raw = Map::new();
    let mut headings = HashMap::new();
    let mut pictures = BTreeSet::new();
    let image = ty.document().presentation.image.as_ref();
    for r in a["revisions"].as_array().into_iter().flatten() {
        for (key, value) in r["changes"].as_object().into_iter().flatten() {
            if value.is_null() {
                raw.remove(key);
            } else {
                raw.insert(key.clone(), value.clone());
            }
        }
        let Some(revision) = r["revision"].as_i64().filter(|r| named.contains(r)) else {
            continue;
        };
        let fields = fields_from_json(ty, &raw).unwrap_or_default();
        headings.insert(revision, kuria_engine::markdown::heading_ids(ty, &fields));
        if image.is_some_and(|key| matches!(fields.get(key), Some(Value::Attachment(_)))) {
            pictures.insert(revision);
        }
    }
    versions
        .iter()
        .map(|v| {
            let revision = v["revision"].as_i64().unwrap_or_default();
            let mut version = json!({
                "number": v["number"],
                "revision": revision,
                "label": v["label"],
                "by": ids.text(uuid(&v["releasedBy"])),
                "at": v["releasedAt"]
                    .as_str()
                    .map(|t| time(t, Utc::now()).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
                "pins": v["pins"],
                "headings": headings.remove(&revision).unwrap_or_default(),
            });
            if pictures.contains(&revision) {
                version["picture"] = true.into();
            }
            if let Some(description) = v["description"].as_str() {
                version["description"] = description.into();
            }
            version
        })
        .collect()
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

/// An akt's state and fields at one revision.
type Snapshot = (String, BTreeMap<String, Value>);

/// `akt` at each of its revisions, from 1 to its file's: as git's latest
/// commit at that revision holds it, else as at the next revision git has,
/// and its file as it is now for its own. A revision a version names must
/// be one git has, so that the version reads as it was released.
fn snapshots(
    dir: &AktDir,
    times: &Times,
    akt: &LocalAkt,
    now: &BTreeMap<String, Value>,
    ids: &mut Ids,
) -> Result<Vec<Snapshot>, CliError> {
    let ty = &dir.type_of(akt).ty;
    let mut known: Vec<Option<Snapshot>> = (1..akt.revision)
        .map(|revision| {
            let (path, text) = times.text_at(&akt.number, revision)?;
            let old = read_text(&dir.root.join(path), text, &dir.types)
                .into_iter()
                .filter_map(Result::ok)
                .find(|a| a.number == akt.number)?;
            let fields = current(ty, &old, ids).ok()?;
            Some((old.state, fields))
        })
        .collect();
    known.push(Some((akt.state.clone(), now.clone())));
    for v in &akt.versions {
        let revision = v["revision"].as_i64().unwrap_or_default();
        let had = usize::try_from(revision - 1)
            .ok()
            .and_then(|i| known.get(i))
            .is_some_and(Option::is_some);
        if !had {
            return Err(CliError::refused(
                "NOT_FOUND",
                format!(
                    "git has no commit of {} at revision {revision}, which its version {} names",
                    akt.number, v["number"]
                ),
            ));
        }
    }
    let mut filled = Vec::with_capacity(known.len());
    let mut next: Option<Snapshot> = None;
    for snapshot in known.into_iter().rev() {
        let snapshot = snapshot.or_else(|| next.clone());
        next.clone_from(&snapshot);
        filled.push(snapshot.unwrap_or_default());
    }
    filled.reverse();
    Ok(filled)
}

/// `akt`'s revisions in a bundle, from its `snapshots`: the first recording
/// it as it was, each later one what changed, each with the hash of what
/// it results in.
fn revisions(snapshots: &[Snapshot], author: Uuid, at: &dyn Fn(i32) -> DateTime<Utc>) -> Vec<Json> {
    let mut before = Map::new();
    snapshots
        .iter()
        .zip(1..)
        .map(|((state, fields), revision)| {
            let after = fields_to_json(fields);
            let changes: Map<String, Json> = if revision == 1 {
                after.clone()
            } else {
                after
                    .iter()
                    .filter(|(k, v)| before.get(*k) != Some(*v))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .chain(
                        before
                            .keys()
                            .filter(|k| !after.contains_key(*k))
                            .map(|k| (k.clone(), Json::Null)),
                    )
                    .collect()
            };
            before = after;
            json!({
                "revision": revision,
                "author": author,
                "transition": null,
                "reason": (revision == 1).then_some("From an akt directory, whose history git holds"),
                "changes": changes,
                "state": state,
                "hash": hex(&state_hash(state, fields)),
                "createdAt": at(revision),
                "membership": null,
            })
        })
        .collect()
}

/// `akt`'s versions as a bundle holds them (`bundle.contents`).
fn bundled_versions(akt: &LocalAkt, ids: &mut Ids, now: DateTime<Utc>) -> Vec<Json> {
    akt.versions
        .iter()
        .map(|v| {
            let by = v["by"].as_str().unwrap_or_default();
            let mut version = json!({
                "number": v["number"],
                "revision": v["revision"],
                "releasedBy": if by.is_empty() { Uuid::nil() } else { ids.id(by) },
                "releasedAt": time(v["at"].as_str().unwrap_or_default(), now),
                "pins": if v["pins"].is_array() { v["pins"].clone() } else { json!([]) },
            });
            if let Some(label) = v["label"].as_str() {
                version["label"] = json!(label);
            }
            if let Some(description) = v["description"].as_str() {
                version["description"] = json!(description);
            }
            version
        })
        .collect()
}

/// `akt`'s fields as they are, and its revisions in a bundle, as git's
/// commits hold it at each (`snapshots`).
fn history_of(
    dir: &AktDir,
    times: &Times,
    akt: &LocalAkt,
    ids: &mut Ids,
    (created, updated, author): (DateTime<Utc>, DateTime<Utc>, Uuid),
) -> Result<(BTreeMap<String, Value>, Vec<Json>), CliError> {
    let fields = current(&dir.type_of(akt).ty, akt, ids)?;
    let mut snapshots = snapshots(dir, times, akt, &fields, ids)?;
    // A checkbox ticked in the file was ticked when it last changed.
    for (_, fields) in &mut snapshots {
        for value in fields.values_mut() {
            if let Value::Checkbox { at, .. } = value {
                *at = updated;
            }
        }
    }
    let at = |revision: i32| match revision {
        1 => created,
        _ => times
            .revision_at(&akt.number, revision)
            .map_or(updated, |t| time(&t, updated)),
    };
    let revisions = revisions(&snapshots, author, &at);
    let fields = snapshots.pop().map(|(_, f)| f).unwrap_or(fields);
    Ok((fields, revisions))
}

/// The akt directory at `root` as a bundle of its akt types with their
/// akter and the files they hold, made by `me`, once `kuria check` finds
/// it consistent.
// r[impl cli.local.convert]
pub fn bundle_of(root: &Path, me: Option<String>) -> Result<Unpacked, CliError> {
    let (dir, times) = loaded(root)?;
    let now = Utc::now();
    let mut ids = Ids::new(me);
    let author = ids.changer();
    let akt_types = types_in_bundle(&dir, author, now);
    let mut akter = Vec::new();
    let mut memberships = Vec::new();
    let mut links = Vec::new();
    for akt in dir.akter.iter().chain(&dir.archived) {
        let t = dir.type_of(akt);
        let (created, mut updated) = times.of_akt(akt);
        let entry = dir.archive.entry(&akt.number);
        if let Some(entry) = entry {
            updated.clone_from(&entry.at);
        }
        let (created, updated) = (time(&created, now), time(&updated, now));
        let (fields, mut revisions) =
            history_of(&dir, &times, akt, &mut ids, (created, updated, author))?;
        let versions = bundled_versions(akt, &mut ids, now);
        let id = id_of(&format!("akt:{}", akt.number));
        let archived = entry.map(|e| archived_in_bundle(e, &mut revisions, &mut ids, author, now));
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
            "attachments": held(&fields),
            "versions": versions,
        }));
        if let Some(archived) = archived {
            akter.last_mut().expect("pushed")["archived"] = archived;
        } else if dir.archive.kept(&akt.number) {
            akter.last_mut().expect("pushed")["archiveKept"] = true.into();
        }
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
    let (attachments, bytes) = attachments_of(root, &ids, author, now)?;
    let bundle = json!({
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
        "attachments": attachments,
    });
    Ok(Unpacked {
        bundle,
        attachments: bytes,
    })
}

/// The directory's akt types as a bundle holds them, each one version.
fn types_in_bundle(dir: &AktDir, author: Uuid, now: DateTime<Utc>) -> Vec<Json> {
    dir.types
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
        .collect()
}

/// The akt directory at `root`, once `kuria check` finds it consistent,
/// with its archived akter, each of which git or `akter/archived/` must
/// have, and what git says of it.
fn loaded(root: &Path) -> Result<(AktDir, Times), CliError> {
    check(root)?;
    let mut dir = AktDir::open(root)?;
    let times = Times::of(root);
    dir.load_archived(&times);
    if let Some(entry) = dir
        .archive
        .archived
        .iter()
        .find(|e| dir.archived.iter().all(|a| a.number != e.number))
    {
        return Err(CliError::refused(
            "NOT_FOUND",
            format!(
                "{} is archived, but neither git nor akter/archived/ has it at revision {}",
                entry.number, entry.revision
            ),
        ));
    }
    Ok((dir, times))
}

/// How the archived akt `e` lists is in a bundle, the last of its
/// `revisions` made its archive's, by whom or which rule archived it
/// (`archive.history`).
// r[impl cli.local.archive]
fn archived_in_bundle(
    e: &Entry,
    revisions: &mut [Json],
    ids: &mut Ids,
    author: Uuid,
    now: DateTime<Utc>,
) -> Json {
    if let Some(last) = revisions.last_mut() {
        last["transition"] = "archive".into();
        last["reason"] = match (&e.rule, &e.reason) {
            (Some(rule), Some(reason)) => format!("by the rule `{rule}`: {reason}").into(),
            (Some(rule), None) => format!("by the rule `{rule}`").into(),
            (None, reason) => json!(reason),
        };
        // A rule archives as Kuria, the nil subject.
        last["author"] = match (&e.rule, e.by.as_deref()) {
            (None, Some(by)) => json!(ids.id(by)),
            (None, None) => json!(author),
            (Some(_), _) => json!(Uuid::nil()),
        };
    }
    let by = match (&e.rule, &e.by) {
        (Some(rule), _) => {
            let mut by = json!({ "rule": rule });
            if let Some(container) = &e.container {
                by["container"] = json!(id_of(&format!("akt:{container}")));
            }
            by
        }
        (None, Some(who)) => json!({ "subject": ids.id(who) }),
        (None, None) => json!({ "subject": author }),
    };
    json!({ "at": time(&e.at, now), "by": by, "reason": e.reason })
}

/// The attachments `fields` hold, by field, as a bundle's akt lists them.
fn held(fields: &BTreeMap<String, Value>) -> Vec<Json> {
    fields
        .iter()
        .filter_map(|(field, value)| match value {
            Value::Attachment(a) => Some(json!({ "field": field, "attachment": a })),
            _ => None,
        })
        .collect()
}

/// Attachments' bytes, by their SHA-256 in lower-case hex.
type Bytes = BTreeMap<String, Vec<u8>>;

/// Every attachment `ids` named, the akter holding it now or at an earlier
/// revision, as a bundle holds it (`bundle.contents`), and its bytes by
/// their SHA-256: the directory's file, else, for one since removed, what
/// git last held there.
// r[impl cli.local.attachments]
fn attachments_of(
    root: &Path,
    ids: &Ids,
    author: Uuid,
    now: DateTime<Utc>,
) -> Result<(Vec<Json>, Bytes), CliError> {
    use sha2::{Digest, Sha256};
    let mut listed = Vec::new();
    let mut bytes = BTreeMap::new();
    let mut held: Vec<(Uuid, &str)> = ids.attachments().collect();
    held.sort_by_key(|(_, path)| *path);
    for (id, path) in held {
        let read = std::fs::read(root.join(path))
            .ok()
            .or_else(|| super::git::last_bytes(root, path))
            .ok_or_else(|| {
                CliError::refused(
                    "NOT_FOUND",
                    format!("neither the directory nor git has `{path}`, which an akt holds"),
                )
            })?;
        let sha = hex(&Sha256::digest(&read));
        listed.push(json!({
            "id": id,
            "name": path.rsplit('/').next(),
            "mediaType": attachments::media_type(path),
            "size": read.len(),
            "sha256": sha,
            "uploadedBy": author,
            "uploadedAt": now,
            "sbom": kuria_engine::sbom::detect(&read),
        }));
        bytes.insert(sha, read);
    }
    Ok((listed, bytes))
}
