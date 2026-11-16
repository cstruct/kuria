//! Archiving in local mode (`cli.local.archive`): an archived akt leaves
//! its file, git's history keeping it, and `akter/archived.md` lists it,
//! with when, by whom or which rule, why, and the revision git keeps it
//! at. Akt types' rules apply on the changes the CLI makes, as on a server
//! (`archive.rules.on-change`), and `kuria check --fix` applies them all
//! (`archive.sweep`).
//!
//! An akt that git doesn't hold as it is, such as one with changes not
//! committed yet or one of a directory `export --to-dir` wrote, is kept as
//! its own file in `akter/archived/` too, which `kuria unarchive` reads
//! first.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use kuria_config::{AktType, ArchiveRule};
use kuria_engine::rank_between;
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use super::dir::{AktDir, LocalAkt, read_text};
use super::git::Times;
use super::write::{Writer, text_of};
use crate::error::CliError;
use crate::output::Answer;
use crate::profile::io_error;

/// The file of `akter/` that lists the archived akter.
pub const FILE: &str = "archived.md";

/// The directory of `akter/` that keeps archived akter git doesn't hold.
pub const KEPT: &str = "archived";

/// What `akter/archived.md` says after its front matter.
const ABOUT: &str = "Akter archived from this directory (`kuria archive`, or their types' archive rules). Each left its file; git's history keeps it at the revision named, else `archived/` does. Their numbers stay taken, and references to them read as archived. `kuria unarchive <number>` puts one back. `kept` lists akter unarchived after a rule archived them, which rules leave alone until archived by hand.";

/// An archived akt, as `akter/archived.md` lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub number: String,
    #[serde(rename = "type")]
    pub type_key: String,
    /// Its revision when it was archived, at which git, or `archived/`,
    /// keeps it.
    pub revision: i32,
    /// When, in RFC 3339 UTC.
    pub at: String,
    /// Who archived it, if a person did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// The kind of rule that archived it, if one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// For `with_container`, the container it went with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Entry {
    /// As GraphQL's `Archived` answers it.
    #[must_use]
    pub fn json(&self) -> Json {
        let by = self
            .by
            .as_deref()
            .filter(|_| self.rule.is_none())
            .map(|b| json!({ "id": b, "name": b, "email": b.contains('@').then_some(b) }));
        json!({ "at": self.at, "by": by, "rule": self.rule, "reason": self.reason })
    }
}

/// `akter/archived.md`: the archived akter, and those kept from rules.
// r[impl cli.local.archive.kept]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    #[serde(default)]
    pub archived: Vec<Entry>,
    /// Live akter a person unarchived after a rule archived them
    /// (`archive.rules.kept`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kept: Vec<String>,
}

impl Archive {
    /// Where the directory at `root` lists its archived akter.
    #[must_use]
    pub fn path(root: &Path) -> PathBuf {
        root.join("akter").join(FILE)
    }

    /// The directory at `root`'s list; none if it has no file.
    pub fn read(root: &Path) -> Result<Self, CliError> {
        let path = Self::path(root);
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text)
                .map_err(|why| CliError::refused("BAD_FILE", format!("{}: {why}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(io_error(&path, &e)),
        }
    }

    /// The list `text` holds in its front matter.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        let rest = text
            .strip_prefix("---\n")
            .ok_or("doesn't begin with front matter (`---`)")?;
        let end = if rest.starts_with("---") {
            0
        } else {
            rest.find("\n---")
                .map(|e| e + 1)
                .ok_or("has no end to its front matter (`---`)")?
        };
        let archive: Self = serde_yaml_ng::from_str(&rest[..end]).map_err(|e| e.to_string())?;
        let mut seen = HashSet::new();
        for entry in &archive.archived {
            if !seen.insert(entry.number.as_str()) {
                return Err(format!("lists {} twice", entry.number));
            }
            if entry.by.is_none() && entry.rule.is_none() {
                return Err(format!(
                    "names neither who nor which rule archived {}",
                    entry.number
                ));
            }
        }
        Ok(archive)
    }

    /// The file's text.
    #[must_use]
    pub fn text(&self) -> String {
        let yaml = serde_yaml_ng::to_string(self).expect("serializable");
        format!("---\n{yaml}---\n\n{ABOUT}\n")
    }

    #[must_use]
    pub fn entry(&self, number: &str) -> Option<&Entry> {
        self.archived.iter().find(|e| e.number == number)
    }

    /// Whether `number` is archived.
    #[must_use]
    pub fn has(&self, number: &str) -> bool {
        self.entry(number).is_some()
    }

    /// Whether rules leave the live akt `number` alone.
    #[must_use]
    pub fn kept(&self, number: &str) -> bool {
        self.kept.iter().any(|k| k == number)
    }
}

/// Where the directory at `root` keeps the archived akt `number` that git
/// doesn't hold as it is.
#[must_use]
pub fn kept_path(root: &Path, number: &str) -> PathBuf {
    root.join("akter").join(KEPT).join(format!("{number}.md"))
}

/// The archived akt `entry` names, as `akter/archived/` keeps it, else as
/// git's latest commit holding it at its revision had it, at the revision
/// its archiving reached; `None` if neither has it.
// r[impl cli.local.archive.kept]
#[must_use]
pub fn read_archived(dir: &AktDir, times: &Times, entry: &Entry) -> Option<LocalAkt> {
    let kept = kept_path(&dir.root, &entry.number);
    let (path, text) = match std::fs::read_to_string(&kept) {
        Ok(text) => (kept, text),
        Err(_) => {
            let (path, text) = times.latest_at(&entry.number, entry.revision)?;
            (dir.root.join(path), text.to_owned())
        }
    };
    let mut akt = read_text(&path, &text, &dir.types)
        .into_iter()
        .filter_map(Result::ok)
        .find(|a| a.number == entry.number)?;
    akt.revision = entry.revision + 1;
    akt.nested = false;
    Some(akt)
}

/// Who archives or unarchives an akt: a person, or a rule of its type,
/// and for `with_container` the container it goes with.
#[derive(Clone, Debug)]
enum By {
    Person,
    Rule {
        kind: &'static str,
        container: Option<String>,
    },
}

/// An akt type's archive rules, owned.
enum Rule {
    Newer {
        count: u32,
        states: Vec<String>,
        within: Option<String>,
    },
    WithContainer(String),
    DaysIn {
        days: u32,
        states: Vec<String>,
    },
}

/// The rules `ty` declares (`archive.rules`).
fn rules_of(ty: &AktType) -> Vec<Rule> {
    ty.document()
        .archive
        .iter()
        .flat_map(|a| &a.rules)
        .filter_map(kuria_config::ArchiveRuleDef::rule)
        .map(|r| match r {
            ArchiveRule::Newer {
                count,
                states,
                within,
            } => Rule::Newer {
                count,
                states: states.to_vec(),
                within: within.map(str::to_owned),
            },
            ArchiveRule::WithContainer(key) => Rule::WithContainer(key.to_owned()),
            ArchiveRule::DaysIn { days, states } => Rule::DaysIn {
                days,
                states: states.to_vec(),
            },
        })
        .collect()
}

fn rfc3339(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// `kuria archive <number>…` and, with `archive` false, `kuria unarchive
/// <number>…`, in local mode: each in order, stopping at the first
/// refused, what was done before it kept.
// r[impl cli.local.archive]
// r[impl cli.archive]
pub fn command(
    mut w: Writer,
    numbers: &[String],
    reason: Option<&str>,
    archive: bool,
) -> Result<Answer, CliError> {
    let reason = crate::writes::archive::reason_of(reason)?;
    // Asked of git once, before anything here changes the directory.
    let uncommitted = !archive && super::git::uncommitted(&w.dir.root) == Some(true);
    let mut done = Vec::new();
    for number in numbers {
        let result = if archive {
            archive_by_person(&mut w, number, reason.as_deref())
        } else {
            unarchive_by_person(&mut w, number, uncommitted)
        };
        match result {
            Ok(revision) => done.push((number.clone(), i64::from(revision))),
            Err(refused) => {
                if !done.is_empty() {
                    w.save()?;
                }
                return Err(crate::writes::archive::after(&done, archive, refused));
            }
        }
    }
    w.save()?;
    let said = crate::writes::archive::answer(&done, archive);
    Ok(super::write::answer(
        said.markdown,
        said.json,
        reason.as_deref(),
    ))
}

/// The refusal of a write to the archived akt `number` (`archive.readonly`).
// r[impl cli.local.archive]
// r[impl archive.readonly]
#[must_use]
pub fn archived(number: &str) -> CliError {
    CliError::from_server(
        "ARCHIVED",
        format!(
            "{number} is archived: unarchive it first (`kuria unarchive {number}`); an archived akt is in no file of the directory, only listed in akter/archived.md"
        ),
        None,
    )
}

fn refused_live(w: &Writer, number: &str, archive: bool) -> Result<(), CliError> {
    let live = w.dir.akt(number).is_some();
    let archived = w.dir.is_archived(number);
    match (live, archived, archive) {
        (true, _, true) | (_, true, false) => Ok(()),
        (false, true, true) => Err(CliError::refused(
            "BAD_REQUEST",
            format!("{number} is archived already"),
        )),
        (true, false, false) => Err(CliError::refused(
            "BAD_REQUEST",
            format!("{number} is live already"),
        )),
        (false, false, _) => Err(CliError::from_server(
            "NOT_FOUND",
            format!("No akt `{number}`"),
            None,
        )),
    }
}

/// Archives the live akt `number` as the person writing, with `reason`,
/// answering the revision it reaches.
fn archive_by_person(w: &mut Writer, number: &str, reason: Option<&str>) -> Result<i32, CliError> {
    refused_live(w, number, true)?;
    archive_one(w, number, &By::Person, reason)
}

/// Archives the live akt `number` by `by`, and with it the akter nested in
/// its file, which go with it as `with_container` takes them; then the
/// members its archiving takes by their types' rules, and so on down
/// (`archive.rules.on-change`). Answers the revision it reaches.
fn archive_one(
    w: &mut Writer,
    number: &str,
    by: &By,
    reason: Option<&str>,
) -> Result<i32, CliError> {
    // Those nested in its file, parents first, each with its parent.
    let mut going: Vec<(String, By)> = vec![(number.to_owned(), by.clone())];
    let mut nested: Vec<(String, String)> = w
        .dir
        .akter
        .iter()
        .filter_map(|a| {
            let parent = super::layout::parent(&w.dir, a)?;
            let mut up = Some(parent);
            while let Some(p) = up {
                if p.number == number {
                    return Some((a.number.clone(), parent.number.clone()));
                }
                up = super::layout::parent(&w.dir, p);
            }
            None
        })
        .collect();
    nested.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then_with(|| a.0.cmp(&b.0)));
    going.extend(nested.into_iter().map(|(n, parent)| {
        (
            n,
            By::Rule {
                kind: "with_container",
                container: Some(parent),
            },
        )
    }));
    let mut revision = 0;
    for (i, (n, by)) in going.iter().enumerate() {
        let reached = take(w, n, by, if i == 0 { reason } else { None });
        if i == 0 {
            revision = reached;
        }
    }
    for (n, _) in &going {
        follow(w, n)?;
    }
    Ok(revision)
}

/// Takes the live akt `number` out of its file into the archive, by `by`:
/// listed in `akter/archived.md`, and kept in `akter/archived/` unless git
/// holds it as it is. Answers the revision its archiving reaches.
// r[impl cli.local.archive.kept]
fn take(w: &mut Writer, number: &str, by: &By, reason: Option<&str>) -> i32 {
    let akt = w.dir.akt(number).expect("live").clone();
    let own = text_of(&w.dir, &akt, true);
    let git_has_it = w
        .times
        .latest_at(number, akt.revision)
        .and_then(|(path, text)| {
            read_text(&w.dir.root.join(path), text, &w.dir.types)
                .into_iter()
                .filter_map(Result::ok)
                .find(|a| a.number == number)
        })
        .is_some_and(|old| text_of(&w.dir, &old, true) == own);
    if !git_has_it {
        let path = kept_path(&w.dir.root, number);
        w.keep_file(path, Some(own));
    }
    let (person, rule, container) = match by {
        By::Person => (w.ids.me.clone(), None, None),
        By::Rule { kind, container } => (None, Some((*kind).to_owned()), container.clone()),
    };
    let entry = Entry {
        number: number.to_owned(),
        type_key: akt.type_key.clone(),
        revision: akt.revision,
        at: rfc3339(w.now),
        by: person.or_else(|| rule.is_none().then(|| "someone".to_owned())),
        rule,
        container,
        reason: reason.map(str::to_owned),
    };
    let mut removed = w.remove(number);
    removed.revision += 1;
    let reached = removed.revision;
    w.dir.archived.push(removed);
    let archive = w.archive_mut();
    archive.archived.push(entry);
    if matches!(by, By::Person) {
        // r[impl archive.rules.kept]
        archive.kept.retain(|k| k != number);
    }
    reached
}

/// Whether the live akt `number`'s type archives it with containers of
/// the type `container_type`, and no person kept it from rules.
fn goes_with(w: &Writer, number: &str, container_type: &str) -> bool {
    let Some(akt) = w.dir.akt(number) else {
        return false;
    };
    !w.dir.archive.kept(number)
        && rules_of(&w.dir.type_of(akt).ty)
            .iter()
            .any(|r| matches!(r, Rule::WithContainer(k) if k == container_type))
}

/// After `container` was archived, archives the live members whose types
/// archive them with it (`archive.rules.container`).
// r[impl archive.rules.on-change]
fn follow(w: &mut Writer, container: &str) -> Result<(), CliError> {
    let Some(container_type) = w.dir.archive.entry(container).map(|e| e.type_key.clone()) else {
        return Ok(());
    };
    let members: Vec<String> = w
        .dir
        .members_of(container)
        .map(|m| m.number.clone())
        .collect();
    for member in members {
        if goes_with(w, &member, &container_type) {
            let by = By::Rule {
                kind: "with_container",
                container: Some(container.to_owned()),
            };
            archive_one(w, &member, &by, None)?;
        }
    }
    Ok(())
}

/// Unarchives the archived akt `number` as the person writing, marking it
/// kept if a rule archived it, answering the revision it reaches.
/// Refused as `UNCOMMITTED` while the directory has changes git hasn't
/// committed, as `uncommitted` says.
fn unarchive_by_person(w: &mut Writer, number: &str, uncommitted: bool) -> Result<i32, CliError> {
    refused_live(w, number, false)?;
    if uncommitted {
        return Err(CliError::refused(
            "UNCOMMITTED",
            format!(
                "{} has changes git hasn't committed; commit them first, so that unarchiving {number} puts back what git holds",
                w.dir.root.display()
            ),
        ));
    }
    unarchive_one(w, number, &By::Person)
}

/// Puts the archived akt `number` back in the directory, by `by`, as git,
/// or `akter/archived/`, keeps it, then the members `with_container` took
/// with it, and so on down (`archive.rules.container`). Answers the
/// revision it reaches.
// r[impl archive.rules.container]
fn unarchive_one(w: &mut Writer, number: &str, by: &By) -> Result<i32, CliError> {
    let entry = w
        .dir
        .archive
        .entry(number)
        .cloned()
        .expect("checked archived");
    let Some(i) = w.dir.archived.iter().position(|a| a.number == number) else {
        return Err(CliError::from_server(
            "NOT_FOUND",
            format!(
                "{number} is archived, but neither git nor akter/archived/ has it at revision {}",
                entry.revision
            ),
            None,
        ));
    };
    let mut akt = w.dir.archived.remove(i);
    akt.revision += 1;
    // Its place, unless a live akt has taken it.
    if w.dir.akter.iter().any(|a| a.rank == akt.rank) {
        let next = w
            .dir
            .akter
            .iter()
            .map(|a| a.rank.as_str())
            .filter(|r| *r > akt.rank.as_str())
            .min();
        akt.rank = rank_between(Some(&akt.rank), next)
            .map_err(|e| CliError::refused("INTERNAL", e.to_string()))?;
    }
    akt.path = w.dir.root.join("akter").join(format!("{number}.md"));
    akt.nested = false;
    let reached = akt.revision;
    w.add(akt);
    let kept = kept_path(&w.dir.root, number);
    if kept.exists() {
        w.keep_file(kept, None);
    }
    let archive = w.archive_mut();
    archive.archived.retain(|e| e.number != number);
    // r[impl archive.rules.kept]
    if matches!(by, By::Person) && entry.rule.is_some() && !archive.kept(number) {
        archive.kept.push(number.to_owned());
    }
    let took: Vec<String> = w
        .dir
        .archive
        .archived
        .iter()
        .filter(|e| {
            e.rule.as_deref() == Some("with_container") && e.container.as_deref() == Some(number)
        })
        .map(|e| e.number.clone())
        .collect();
    for member in took {
        let by = By::Rule {
            kind: "with_container",
            container: Some(number.to_owned()),
        };
        unarchive_one(w, &member, &by)?;
    }
    Ok(reached)
}

/// When the akt `akt` reached its state: by git, else when its file was
/// last written, else now.
fn since(w: &Writer, akt: &LocalAkt, moved: Option<&str>) -> DateTime<Utc> {
    if moved == Some(akt.number.as_str()) {
        return w.now;
    }
    if let Some(at) = w.times.state_since(&akt.number, &akt.state) {
        return at;
    }
    if let Some(entry) = w.dir.archive.entry(&akt.number) {
        return DateTime::parse_from_rfc3339(&entry.at).map_or(w.now, |t| t.with_timezone(&Utc));
    }
    std::fs::metadata(&akt.path)
        .and_then(|m| m.modified())
        .map_or(w.now, DateTime::from)
}

/// The type of the akt `number`, live or archived.
fn type_key_of(w: &Writer, number: &str) -> Option<String> {
    w.dir
        .any(number)
        .map(|a| a.type_key.clone())
        .or_else(|| w.dir.archive.entry(number).map(|e| e.type_key.clone()))
}

/// Archives each live akt of the type `key` in one of `states` that at
/// least `count` others of its type, in one of `states` too, live or
/// archived, and sharing a container of the type `within` if given,
/// reached after it did; `moved` reached its state now. Answers how many
/// it archived.
fn apply_newer(
    w: &mut Writer,
    key: &str,
    count: u32,
    states: &[String],
    within: Option<&str>,
    moved: Option<&str>,
) -> Result<usize, CliError> {
    struct Seen {
        number: String,
        live: bool,
        since: DateTime<Utc>,
        shared: BTreeSet<String>,
    }
    let seen: Vec<Seen> = w
        .dir
        .with(crate::reads::Archived::All)
        .filter(|a| a.type_key == key && states.contains(&a.state))
        .map(|a| Seen {
            number: a.number.clone(),
            live: w.dir.akt(&a.number).is_some(),
            since: since(w, a, moved),
            shared: a
                .containers
                .iter()
                .filter(|c| within.is_some_and(|k| type_key_of(w, c).as_deref() == Some(k)))
                .cloned()
                .collect(),
        })
        .collect();
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    let overtaken: Vec<String> = seen
        .iter()
        .filter(|a| a.live && !w.dir.archive.kept(&a.number))
        .filter(|a| {
            seen.iter()
                .filter(|b| b.number != a.number && b.since > a.since)
                .filter(|b| within.is_none() || !a.shared.is_disjoint(&b.shared))
                .count()
                >= count
        })
        .map(|a| a.number.clone())
        .collect();
    let mut archived = 0;
    for number in overtaken {
        if w.dir.akt(&number).is_some() && !w.dir.archive.kept(&number) {
            let by = By::Rule {
                kind: "newer",
                container: None,
            };
            archive_one(w, &number, &by, None)?;
            archived += 1;
        }
    }
    Ok(archived)
}

/// After the akt `number` reached its state by a transition, archives the
/// akter of its type its `newer` rules say it overtook
/// (`archive.rules.on-change`).
// r[impl cli.local.archive]
// r[impl archive.rules.on-change]
pub fn after_transition(w: &mut Writer, number: &str) -> Result<(), CliError> {
    let akt = w.akt(number)?;
    let (key, state) = (akt.type_key.clone(), akt.state.clone());
    let ty = w.dir.types[&key].ty.clone();
    for rule in rules_of(&ty) {
        if let Rule::Newer {
            count,
            states,
            within,
        } = rule
            && states.contains(&state)
        {
            apply_newer(w, &key, count, &states, within.as_deref(), Some(number))?;
        }
    }
    Ok(())
}

/// Applies every rule of every type to the directory's live akter, as
/// the sweep does (`archive.sweep`): `newer`, `with_container` for akter
/// whose container is archived, and `days_in`. Answers how many it
/// archived.
// r[impl cli.local.archive]
pub fn sweep(w: &mut Writer) -> Result<usize, CliError> {
    let mut archived = 0;
    let types: Vec<(String, AktType)> = w
        .dir
        .types
        .iter()
        .map(|(k, t)| (k.clone(), t.ty.clone()))
        .collect();
    for (key, ty) in types {
        for rule in rules_of(&ty) {
            match rule {
                Rule::Newer {
                    count,
                    states,
                    within,
                } => archived += apply_newer(w, &key, count, &states, within.as_deref(), None)?,
                Rule::WithContainer(container_type) => {
                    let going: Vec<(String, String)> = w
                        .dir
                        .akter
                        .iter()
                        .filter(|a| a.type_key == key && !w.dir.archive.kept(&a.number))
                        .filter_map(|a| {
                            let container = a.containers.iter().find(|c| {
                                w.dir
                                    .archive
                                    .entry(c)
                                    .is_some_and(|e| e.type_key == container_type)
                            })?;
                            Some((a.number.clone(), container.clone()))
                        })
                        .collect();
                    for (number, container) in going {
                        if w.dir.akt(&number).is_some() {
                            let by = By::Rule {
                                kind: "with_container",
                                container: Some(container),
                            };
                            archive_one(w, &number, &by, None)?;
                            archived += 1;
                        }
                    }
                }
                Rule::DaysIn { days, states } => {
                    let until = w.now - Duration::days(i64::from(days));
                    let going: Vec<String> = w
                        .dir
                        .akter
                        .iter()
                        .filter(|a| a.type_key == key && states.contains(&a.state))
                        .filter(|a| !w.dir.archive.kept(&a.number) && since(w, a, None) <= until)
                        .map(|a| a.number.clone())
                        .collect();
                    for number in going {
                        if w.dir.akt(&number).is_some() {
                            let by = By::Rule {
                                kind: "days_in",
                                container: None,
                            };
                            archive_one(w, &number, &by, None)?;
                            archived += 1;
                        }
                    }
                }
            }
        }
    }
    Ok(archived)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_reads_back_what_it_writes() {
        let archive = Archive {
            archived: vec![
                Entry {
                    number: "PLAN-001".to_owned(),
                    type_key: "plan".to_owned(),
                    revision: 4,
                    at: "2026-11-16T10:00:00Z".to_owned(),
                    by: Some("me@kuria.example".to_owned()),
                    rule: None,
                    container: None,
                    reason: Some("Done: with it".to_owned()),
                },
                Entry {
                    number: "PLAN-001-T01".to_owned(),
                    type_key: "task".to_owned(),
                    revision: 2,
                    at: "2026-11-16T10:00:00Z".to_owned(),
                    by: None,
                    rule: Some("with_container".to_owned()),
                    container: Some("PLAN-001".to_owned()),
                    reason: None,
                },
            ],
            kept: vec!["PLAN-002".to_owned()],
        };
        let text = archive.text();
        assert!(text.starts_with("---\narchived:\n"), "{text}");
        assert_eq!(Archive::parse(&text).unwrap(), archive);
        assert!(Archive::parse("---\nkept: [X-1]\n---\n").is_ok());
        assert!(Archive::parse("no front matter").is_err());
        assert!(
            Archive::parse("---\narchived:\n- {number: X-1, type: t, revision: 1, at: x}\n---\n")
                .is_err(),
            "neither who nor which rule"
        );
    }
}
