//! A clone synced with Kuria: `kuria docs clone`, `kuria docs pull` and
//! `kuria docs push` (`clone.clone`, `clone.pull`, `clone.push`).
//!
//! A push sends, as one `pushClone` (`clone.server`), for each staged file
//! of an akt with a live draft, a Yjs update made against the base's state:
//! the live draft loaded from it, with the fields the staged file changed
//! from the base written to it, a text merged from the base into the live
//! draft's text by characters. For any other akt, an `editAkt` of the
//! fields the staged file changed, at the base's revision.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use kuria_engine::live::{LiveDraft, is_text};
use kuria_engine::merge::{has_conflict, merge_marked, merge_text};
use kuria_engine::{Change, Input, Value as FieldValue};
use serde_json::{Map, Value as Json, json};

use super::base::{AktBase, Bases, Local, Recorded};
use super::layout::{normal, read_file};
use super::{CloneOf, Now, pictures};
use crate::error::{CliError, Kind};
use crate::file::Parsed;
use crate::local::layout::file_name;
use crate::model::{SHAPE, Shape, read};
use crate::output::Answer;
use crate::session::Session;
use crate::values::{input_of, same_input};

/// The base `now` makes for each of its akter.
fn bases_of(now: &Now) -> Bases {
    Bases {
        akter: now
            .akter
            .iter()
            .map(|(number, akt)| {
                (
                    number.clone(),
                    AktBase {
                        path: akt.path.clone(),
                        revision: akt.revision,
                        live: akt.live.clone(),
                    },
                )
            })
            .collect(),
    }
}

/// `kuria docs clone <number> [<dir>]`: the container `number` and its members
/// as files in `dir`, by default a directory named as its file is in
/// `here`, with the base in `<dir>/.kuria/` (`clone.clone`).
// r[impl clone.clone]
pub async fn clone(
    session: &Session,
    number: &str,
    dir: Option<&Path>,
    here: &Path,
) -> Result<Answer, CliError> {
    let mut of = CloneOf {
        session,
        number: number.to_owned(),
        root: here.to_path_buf(),
    };
    let title = of.container().await?;
    let root = match dir {
        Some(dir) => here.join(dir),
        None => {
            let name = file_name(number, title.as_deref());
            here.join(name.trim_end_matches(".md"))
        }
    };
    let taken = std::fs::read_dir(&root).is_ok_and(|mut entries| entries.next().is_some());
    if taken {
        return Err(CliError::refused(
            "BAD_REQUEST",
            format!(
                "{} already holds files: clone into a new or empty directory",
                root.display()
            ),
        ));
    }
    let made = !root.exists();
    of.root.clone_from(&root);
    let cloned = write_clone(&of, &root).await;
    if cloned.is_err() && made {
        drop(std::fs::remove_dir_all(&root));
    }
    let (files, members) = cloned?;
    Ok(Answer {
        markdown: format!(
            "Cloned {number} and its {members} members into {}: {files} files. Edit them, stage with `kuria docs add`, push with `kuria docs push -m <message>`; keep `.kuria/` out of version control (it ignores itself).",
            root.display()
        ),
        json: json!({ "dir": root, "number": number, "files": files, "members": members }),
    })
}

/// Writes the clone `of` is of at `root`: its files and pictures, and the
/// same as its base. Answers how many files and members it wrote.
async fn write_clone(of: &CloneOf<'_>, root: &Path) -> Result<(usize, usize), CliError> {
    let mut local = Local::start(
        root,
        Recorded {
            url: of.session.url.clone(),
            number: of.number.clone(),
        },
    )?;
    let now = of.now().await?;
    for (path, text) in &now.files {
        local.set_working(path, Some(text.as_bytes()))?;
        local.set_base(path, Some(text.as_bytes()))?;
    }
    for (path, bytes) in &now.pictures {
        local.set_working(path, Some(bytes))?;
        local.set_base(path, Some(bytes))?;
    }
    local.bases = bases_of(&now);
    local.save()?;
    Ok((now.files.len(), now.akter.len().saturating_sub(1)))
}

/// The clone `here` is in, refused if `session` speaks to another Kuria.
fn open_for(session: &Session, here: &Path) -> Result<Local, CliError> {
    let local = Local::open(here)?;
    if local.recorded.url.trim_end_matches('/') != session.url.trim_end_matches('/') {
        return Err(CliError::refused(
            "BAD_REQUEST",
            format!(
                "The clone is of {}, not {}: run it with `--url {}`",
                local.recorded.url, session.url, local.recorded.url
            ),
        ));
    }
    Ok(local)
}

/// What a pull did, file by file.
#[derive(Debug, Default)]
struct Pulled {
    merged: Vec<String>,
    conflicted: Vec<String>,
    added: Vec<String>,
    removed: Vec<String>,
    /// Changed here but gone from Kuria, or a picture changed here: kept.
    kept: Vec<String>,
    /// Staged, but conflicting with Kuria's change: unstaged.
    unstaged: Vec<String>,
}

/// The working file at `path` as text, if there is one.
fn text_at(local: &Local, path: &str) -> Option<String> {
    local
        .working(path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// The akt `number`'s file, from the base's `old` to Kuria's `new`, merged
/// into the working file and what is staged (`clone.pull`).
// r[impl clone.pull]
fn pull_akt(
    local: &Local,
    old: Option<&AktBase>,
    new: Option<(&str, &str)>,
    pulled: &mut Pulled,
) -> Result<(), CliError> {
    let base = old.and_then(|o| local.base_text(&o.path));
    let staged = old.and_then(|o| local.staged(&o.path));
    if let Some(o) = old {
        local.set_staged(&o.path, None)?;
    }
    let Some((path, kuria)) = new else {
        // Removed from the container: gone, unless changed here.
        let Some(o) = old else { return Ok(()) };
        match text_at(local, &o.path) {
            Some(w) if Some(&w) != base.as_ref() => pulled.kept.push(o.path.clone()),
            Some(_) => {
                local.set_working(&o.path, None)?;
                pulled.removed.push(o.path.clone());
            }
            None => {}
        }
        return Ok(());
    };
    let at = old.map_or(path, |o| o.path.as_str());
    let working = text_at(local, at);
    let Some(working) = working else {
        // Removed here: a new member appears; one removed stays so.
        if old.is_none() {
            local.set_working(path, Some(kuria.as_bytes()))?;
            pulled.added.push(path.to_owned());
        }
        return Ok(());
    };
    let base = base.unwrap_or_default();
    let merged = merge_marked(&base, &working, kuria);
    if at != path {
        local.set_working(at, None)?;
    }
    if merged.text != working || at != path {
        local.set_working(path, Some(merged.text.as_bytes()))?;
    }
    if merged.conflicts > 0 {
        pulled.conflicted.push(path.to_owned());
    } else if old.is_none() {
        pulled.added.push(path.to_owned());
    } else if kuria != base || at != path {
        pulled.merged.push(path.to_owned());
    }
    if let Some(staged) = staged {
        let restaged = merge_marked(&base, &staged, kuria);
        if restaged.conflicts > 0 {
            pulled.unstaged.push(path.to_owned());
        } else if restaged.text != kuria {
            local.set_staged(path, Some(&restaged.text))?;
        }
    }
    Ok(())
}

/// The pictures, from the base's to Kuria's: each one not changed here
/// becomes Kuria's, and the base becomes Kuria's (`clone.pictures`).
// r[impl clone.pictures]
fn pull_pictures(local: &Local, now: &Now, pulled: &mut Pulled) -> Result<(), CliError> {
    let mut paths: BTreeSet<String> = local.base_pictures()?.into_iter().collect();
    paths.extend(now.pictures.keys().cloned());
    for path in &paths {
        let (base, kuria) = (local.base(path), now.pictures.get(path));
        let working = local.working(path);
        if working == base {
            local.set_working(path, kuria.map(Vec::as_slice))?;
        } else if working.as_ref() != kuria {
            pulled.kept.push(path.clone());
        }
        local.set_base(path, kuria.map(Vec::as_slice))?;
    }
    Ok(())
}

fn said(title: &str, paths: &[String]) -> String {
    if paths.is_empty() {
        String::new()
    } else {
        format!("\n{title}: {}.", paths.join(", "))
    }
}

/// `kuria docs pull`: Kuria's current state merged three ways into the working
/// files, from the base, with conflict markers where both changed the same
/// text; members added appear and members removed go unless changed here;
/// the base becomes Kuria's, and what was staged is staged again against
/// it, or unstaged where it conflicts (`clone.pull`).
// r[impl clone.pull]
pub async fn pull(session: &Session, here: &Path) -> Result<Answer, CliError> {
    let mut local = open_for(session, here)?;
    let of = CloneOf {
        session,
        number: local.recorded.number.clone(),
        root: local.root.clone(),
    };
    let now = of.now().await?;
    let mut pulled = Pulled::default();
    let numbers: BTreeSet<&String> = local.bases.akter.keys().chain(now.akter.keys()).collect();
    for number in &numbers {
        let new = now
            .akter
            .get(*number)
            .map(|a| (a.path.as_str(), now.files[&a.path].as_str()));
        pull_akt(&local, local.bases.akter.get(*number), new, &mut pulled)?;
    }
    for akt in local.bases.akter.values() {
        local.set_base(&akt.path, None)?;
    }
    for (path, text) in &now.files {
        local.set_base(path, Some(text.as_bytes()))?;
    }
    pull_pictures(&local, &now, &mut pulled)?;
    local.bases = bases_of(&now);
    local.save()?;
    let mut markdown = if pulled.merged.is_empty()
        && pulled.added.is_empty()
        && pulled.removed.is_empty()
        && pulled.conflicted.is_empty()
    {
        "Already up to date with Kuria.".to_owned()
    } else {
        "Pulled from Kuria.".to_owned()
    };
    markdown.push_str(&said("Merged", &pulled.merged));
    markdown.push_str(&said("New", &pulled.added));
    markdown.push_str(&said("Removed", &pulled.removed));
    markdown.push_str(&said(
        "Kept, changed here, though Kuria no longer has them so",
        &pulled.kept,
    ));
    markdown.push_str(&said(
        "Conflicts, marked `<<<<<<< ours` … `>>>>>>> kuria`; resolve them, then `kuria docs add`",
        &pulled.conflicted,
    ));
    markdown.push_str(&said(
        "Unstaged, as what was staged conflicts with Kuria's change",
        &pulled.unstaged,
    ));
    Ok(Answer {
        markdown,
        json: json!({
            "merged": pulled.merged,
            "added": pulled.added,
            "removed": pulled.removed,
            "kept": pulled.kept,
            "conflicted": pulled.conflicted,
            "unstaged": pulled.unstaged,
        }),
    })
}

/// What `akt { … }` selects for a push's fetch.
const CURRENT: &str = "number revision liveState { base } type { ...Shape document }";

/// A staged file, to push: its path, its akt, and its text.
struct Staged {
    path: String,
    number: String,
    text: String,
}

/// What a push sends for one akt.
enum Sent {
    /// A live draft's update, and the draft after it.
    Draft {
        number: String,
        base: String,
        update: Vec<u8>,
        after: LiveDraft,
    },
    /// An edit of an akt without one.
    Edit {
        number: String,
        revision: i64,
        fields: Map<String, Json>,
    },
}

fn decode(text: &str, what: &str) -> Result<Vec<u8>, CliError> {
    BASE64
        .decode(text)
        .map_err(|e| CliError::refused("BAD_FILE", format!("{what} isn't base64: {e}")))
}

/// What is staged, checked: each an akt file of the base, without conflict
/// markers; refused for a new file or a picture (`clone.push`).
// r[impl clone.push]
fn staged_of(local: &Local) -> Result<Vec<Staged>, CliError> {
    let mut staged = Vec::new();
    for path in local.staged_paths()? {
        let Some((number, _)) = local.bases.at(&path) else {
            return Err(CliError::refused(
                "BAD_REQUEST",
                format!(
                    "{path} is a new file: akter are created and removed in Kuria; nothing was pushed"
                ),
            ));
        };
        let text = local.staged(&path).unwrap_or_default();
        if has_conflict(&text) {
            return Err(CliError::refused(
                "CONFLICTED",
                format!("{path} holds conflict markers; nothing was pushed"),
            ));
        }
        staged.push(Staged {
            path,
            number: number.clone(),
            text,
        });
    }
    if staged.is_empty() {
        return Err(CliError::refused(
            "BAD_REQUEST",
            "Nothing is staged: stage files with `kuria docs add`, then push",
        ));
    }
    // r[impl clone.pictures]
    for path in local.base_pictures()? {
        if let Some(working) = local.working(&path)
            && Some(&working) != local.base(&path).as_ref()
        {
            return Err(CliError::refused(
                "READ_ONLY",
                format!(
                    "The picture {path} changed: pictures are uploaded in Kuria; restore it, or remove it and `kuria docs pull`. Nothing was pushed"
                ),
            ));
        }
    }
    Ok(staged)
}

/// The akter's base is still Kuria's current state: each live draft at the
/// base's, each other akt at the base's revision. Refused, saying to pull,
/// if not (`clone.push`).
// r[impl clone.push]
fn check_current(
    local: &Local,
    staged: &[Staged],
    current: &BTreeMap<String, Json>,
) -> Result<(), CliError> {
    let mut stale = Vec::new();
    for s in staged {
        let akt = &current[&s.number];
        let base = &local.bases.akter[&s.number];
        let is_current = match (&base.live, akt["liveState"]["base"].as_str()) {
            (Some(live), Some(now)) => {
                let draft = LiveDraft::load(&decode(&live.state, "a live draft's state")?, [])
                    .map_err(|e| CliError::refused("BAD_FILE", e))?;
                draft
                    .is_at(&decode(now, "a live draft's base")?)
                    .unwrap_or(false)
            }
            (None, None) => akt["revision"].as_i64() == Some(base.revision),
            _ => false,
        };
        if !is_current && !stale.contains(&s.number) {
            stale.push(s.number.clone());
        }
    }
    if stale.is_empty() {
        return Ok(());
    }
    Err(CliError::new(
        Kind::Stale,
        "STALE",
        format!(
            "{} changed in Kuria since the clone's base: `kuria docs pull`, then push again. Nothing was pushed",
            stale.join(", ")
        ),
    ))
}

/// The fields the staged file `s` changes from its base's file: each with
/// its input in the base and in the staged file. Refused for a file not in
/// normal form, or one changing what a file can't.
// r[impl clone.push]
async fn changed_fields(
    session: &Session,
    local: &Local,
    s: &Staged,
    shape: &Shape,
) -> Result<Vec<(String, Json, Json)>, CliError> {
    let kept = |text: &str| pictures::linked_to_kuria(text, &s.path);
    let base_text = local.base_text(&s.path).unwrap_or_default();
    let in_file = |e: CliError| CliError {
        message: format!("{}: {}", s.path, e.message),
        ..e
    };
    let (_, base) = read_file(&kept(&base_text)).map_err(in_file)?;
    let staged_text = kept(&s.text);
    let (number, staged) = read_file(&staged_text).map_err(in_file)?;
    for (key, now) in [
        ("number", Some(s.number.clone())),
        ("type", base.text("type")),
        ("state", base.text("state")),
    ] {
        let given = if key == "number" {
            Some(number.clone())
        } else {
            staged.text(key)
        };
        if given != now {
            return Err(CliError {
                field: Some(key.to_owned()),
                ..CliError::refused(
                    "READ_ONLY",
                    format!(
                        "{}: `{key}` can't be changed in a clone's file; nothing was pushed",
                        s.path
                    ),
                )
            });
        }
    }
    // r[impl clone.normal]
    if normal(shape, &staged_text).map_err(in_file)? != staged_text {
        return Err(CliError::refused(
            "NOT_NORMAL",
            format!(
                "{} isn't in normal form: run `kuria docs fmt`, `kuria docs add` it again and push. Nothing was pushed",
                s.path
            ),
        ));
    }
    inputs_changed(session, shape, &s.number, &base, &staged).await
}

/// Each field whose input differs between the files `base` and `staged`
/// of `number`.
async fn inputs_changed(
    session: &Session,
    shape: &Shape,
    number: &str,
    base: &Parsed,
    staged: &Parsed,
) -> Result<Vec<(String, Json, Json)>, CliError> {
    let (before, after) = (base.values()?, staged.values()?);
    for key in after.keys() {
        crate::writes::known_field(shape, key)?;
    }
    let people =
        crate::writes::people_for(session, shape, after.keys().chain(before.keys())).await?;
    let mut changed = Vec::new();
    for field in &shape.fields {
        if field.kind == "VERDICT" {
            let was = before.get(&field.key).cloned().unwrap_or_else(|| json!({}));
            crate::writes::edit::verdicts_unchanged(number, field, after.get(&field.key), &was)?;
            continue;
        }
        let was = input_of(
            field,
            before.get(&field.key).unwrap_or(&Json::Null),
            &people,
        )?;
        let now = input_of(field, after.get(&field.key).unwrap_or(&Json::Null), &people)?;
        if !same_input(&was, &now) {
            changed.push((field.key.clone(), was, now));
        }
    }
    Ok(changed)
}

/// The update that makes the live draft `state` hold the `changed` fields
/// of `ty`, each text merged into the draft's from the base's by
/// characters, and the draft after it; `None` if nothing differs
/// (`clone.push`).
// r[impl clone.push]
fn live_update(
    ty: &kuria_config::AktType,
    state: &[u8],
    changed: &[(String, Json, Json)],
) -> Result<Option<(Vec<u8>, LiveDraft)>, CliError> {
    let bad = |e: String| CliError::refused("BAD_REQUEST", e);
    let mut draft = LiveDraft::load(state, []).map_err(bad)?;
    let mut fields = draft.fields(ty).map_err(bad)?;
    // A checkbox's checker is the subject; Kuria records who pushed.
    let change = Change {
        by: uuid::Uuid::nil(),
        at: chrono::Utc::now(),
    };
    for (key, was, now) in changed {
        let Some(field) = ty.field(key) else { continue };
        let input = if is_text(field) {
            let current = match fields.get(key) {
                Some(FieldValue::Text(t) | FieldValue::Markdown(t)) => t.clone(),
                _ => String::new(),
            };
            let text = |v: &Json| v.as_str().unwrap_or_default().to_owned();
            Json::String(merge_text(&text(was), &current, &text(now)))
        } else {
            now.clone()
        };
        let refused = |reason: String| CliError {
            field: Some(key.clone()),
            ..CliError::refused("BAD_REQUEST", format!("`{key}`: {reason}"))
        };
        match FieldValue::from_input(field, &input, change).map_err(refused)? {
            Input::Set(value) => {
                fields.insert(key.clone(), value);
            }
            Input::Clear => {
                fields.remove(key);
            }
        }
    }
    Ok(draft.write(ty, &fields).map(|update| (update, draft)))
}

/// What to send for the staged file `s` of an akt as Kuria has it,
/// `current`; `None` if it changes nothing.
async fn sent_for(
    session: &Session,
    local: &Local,
    s: &Staged,
    current: &Json,
) -> Result<Option<Sent>, CliError> {
    let shape: Shape = read(current["type"].clone(), "the akt type")?;
    let changed = changed_fields(session, local, s, &shape).await?;
    if changed.is_empty() {
        return Ok(None);
    }
    let base = &local.bases.akter[&s.number];
    let Some(live) = &base.live else {
        return Ok(Some(Sent::Edit {
            number: s.number.clone(),
            revision: base.revision,
            fields: changed.into_iter().map(|(k, _, now)| (k, now)).collect(),
        }));
    };
    let document = serde_json::from_value(current["type"]["document"].clone())
        .map_err(|e| CliError::unavailable(format!("Kuria's akt type document: {e}")))?;
    let ty = kuria_config::validate(document).map_err(|problems| {
        CliError::unavailable(format!("Kuria's akt type document: {problems:?}"))
    })?;
    let state = decode(&live.state, "a live draft's state")?;
    Ok(
        live_update(&ty, &state, &changed)?.map(|(update, after)| Sent::Draft {
            number: s.number.clone(),
            base: live.base.clone(),
            update,
            after,
        }),
    )
}

const PUSH: &str = "mutation($i: PushCloneInput!) { pushClone(input: $i) {
    drafts { number revision } operations { number revision } } }";

/// `kuria docs push -m <message>`: what is staged, sent as one push after
/// checking the base is still Kuria's current state; then the base becomes
/// what was pushed, and what wasn't staged stays in the working files
/// (`clone.push`).
// r[impl clone.push]
pub async fn push(session: &Session, here: &Path, message: &str) -> Result<Answer, CliError> {
    let mut local = open_for(session, here)?;
    let staged = staged_of(&local)?;
    let of = CloneOf {
        session,
        number: local.recorded.number.clone(),
        root: local.root.clone(),
    };
    let asked: Vec<(String, Option<i64>)> = staged
        .iter()
        .map(|s| (s.number.clone(), None))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let found = of
        .found(&asked, CURRENT, SHAPE, super::FILES_PER_REQUEST)
        .await?;
    let mut current = BTreeMap::new();
    for ((number, _), akt) in asked.iter().zip(found) {
        let akt = akt.ok_or_else(|| {
            CliError::from_server(
                "NOT_FOUND",
                format!(
                    "{number} is gone from Kuria, or you may no longer read it: `kuria docs pull`"
                ),
                None,
            )
        })?;
        current.insert(number.clone(), akt);
    }
    check_current(&local, &staged, &current)?;
    let mut sent = Vec::new();
    for s in &staged {
        if let Some(one) = sent_for(session, &local, s, &current[&s.number]).await? {
            sent.push((s, one));
        }
    }
    let pushed: Vec<String> = sent.iter().map(|(s, _)| s.path.clone()).collect();
    if !sent.is_empty() {
        let answer = send(session, message, &sent).await?;
        record(&mut local, &sent, &answer)?;
    }
    for s in &staged {
        local.set_staged(&s.path, None)?;
    }
    local.save()?;
    Ok(Answer {
        markdown: if pushed.is_empty() {
            "Nothing to push: what was staged is the base.".to_owned()
        } else {
            format!("Pushed {}: “{message}”.", pushed.join(", "))
        },
        json: json!({ "pushed": pushed, "message": message }),
    })
}

/// Sends `sent` as one `pushClone`, answering its `drafts` and
/// `operations`; a stale base is said as a pull to make.
async fn send(
    session: &Session,
    message: &str,
    sent: &[(&Staged, Sent)],
) -> Result<Json, CliError> {
    let mut drafts = Vec::new();
    let mut operations = Vec::new();
    for (_, one) in sent {
        match one {
            Sent::Draft {
                number,
                base,
                update,
                ..
            } => drafts
                .push(json!({ "number": number, "base": base, "update": BASE64.encode(update) })),
            Sent::Edit {
                number,
                revision,
                fields,
            } => operations.push(
                json!({ "editAkt": { "number": number, "revision": revision, "fields": fields } }),
            ),
        }
    }
    session
        .graphql(
            PUSH,
            json!({ "i": { "message": message, "drafts": drafts, "operations": operations } }),
        )
        .await
        .map(|data| data["pushClone"].clone())
        .map_err(|e| {
            if e.code == "STALE" {
                CliError {
                    message: format!(
                        "{}: `kuria docs pull`, then push again. Nothing was pushed",
                        e.message
                    ),
                    ..e
                }
            } else {
                CliError {
                    message: format!("{}; nothing was pushed", e.message),
                    ..e
                }
            }
        })
}

/// The base after `sent` was pushed, as `answer` says: each pushed file as
/// staged, with its new revision, and each live draft as the update left
/// it (`clone.push`).
// r[impl clone.push]
fn record(local: &mut Local, sent: &[(&Staged, Sent)], answer: &Json) -> Result<(), CliError> {
    let revisions: BTreeMap<String, i64> = ["drafts", "operations"]
        .iter()
        .flat_map(|k| answer[*k].as_array().cloned().unwrap_or_default())
        .filter_map(|a| Some((a["number"].as_str()?.to_owned(), a["revision"].as_i64()?)))
        .collect();
    for (s, one) in sent {
        local.set_base(&s.path, Some(s.text.as_bytes()))?;
        let Some(base) = local.bases.akter.get_mut(&s.number) else {
            continue;
        };
        if let Some(revision) = revisions.get(&s.number) {
            base.revision = *revision;
        }
        if let Sent::Draft { after, .. } = one {
            base.live = Some(super::base::LiveBase {
                state: BASE64.encode(after.state()),
                base: BASE64.encode(after.base()),
            });
        }
    }
    Ok(())
}

/// The Kuria the clone `dir` is in was cloned from, if it is in one.
#[must_use]
pub fn url_of(dir: &Path) -> Option<String> {
    Local::open(dir).ok().map(|l| l.recorded.url)
}
