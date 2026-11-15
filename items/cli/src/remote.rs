//! `git-remote-kuria` (`git.*`): a git remote helper that makes
//! `kuria::<url>/<number>` a remote, so that a container and its members
//! can be cloned, fetched, diffed, rebased and pushed with git. Kuria
//! serves no git protocol: the helper builds commits from revisions, the
//! same way for every clone, and applies pushed commits as batches.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value as Json, json};

use crate::error::CliError;
use crate::file::{self, Parsed};
use crate::local::layout::{file_name, slug};
use crate::model::{SHAPE, SOMEONE, Shape, VALUES, Value, read};
use crate::session::Session;
use crate::values::file_value;

/// A clone's remote: the container, on the Kuria `session` speaks to.
pub struct Remote {
    pub session: Session,
    pub number: String,
    /// The remote's name, for its private refs.
    pub name: String,
}

/// `kuria::https://kuria.example/DOCS-1` as the Kuria URL and the number.
pub fn split_url(url: &str) -> Result<(String, String), CliError> {
    let url = url
        .strip_prefix("kuria::")
        .unwrap_or(url)
        .trim_end_matches('/');
    match url.rsplit_once('/') {
        Some((base, number)) if base.starts_with("http") && !number.is_empty() => {
            Ok((base.to_owned(), number.to_owned()))
        }
        _ => Err(CliError::refused(
            "BAD_REQUEST",
            format!(
                "`{url}` isn't a Kuria URL and a container's number, such as https://kuria.example/DOCS-1"
            ),
        )),
    }
}

/// One revision of an akt, as the history needs it.
#[derive(Clone, Debug)]
struct Event {
    at: chrono::DateTime<chrono::Utc>,
    number: String,
    revision: i64,
    by: String,
    reason: Option<String>,
    commit: Option<Json>,
    /// For the container: the members it added and removed.
    added: Vec<String>,
    removed: Vec<String>,
}

/// One commit of the history: its metadata and full tree.
#[derive(Clone, Debug)]
pub struct Built {
    pub author: String,
    pub committer: String,
    pub message: String,
    /// Path to text.
    pub tree: BTreeMap<String, String>,
    /// The akter's revisions as of it, to place versions' tags.
    revisions: HashMap<String, i64>,
}

/// The history of a container: its commits, oldest first, and its versions'
/// tags, `(tag, commit index)`.
pub struct History {
    pub commits: Vec<Built>,
    pub tags: Vec<(String, usize)>,
    /// Each akt's latest revision: what a push checks is unchanged.
    pub seen: BTreeMap<String, i64>,
}

/// An akt at a revision: its shape, its clone file, its title, and its
/// value of a `sections` field, if any.
type AtRevision = (Shape, String, Option<String>, Option<Json>);

const REVISIONS: &str =
    "revisions { revision createdAt reason commit membership by { id name email } }
  versions { number revision }";

impl Remote {
    async fn revisions_of(&self, number: &str) -> Result<(Vec<Event>, Vec<(i64, i64)>), CliError> {
        let data = self
            .session
            .graphql(
                &format!("query($n: String!) {{ akt(number: $n) {{ number {REVISIONS} }} }}"),
                json!({ "n": number }),
            )
            .await?;
        if data["akt"].is_null() {
            return Err(CliError::from_server(
                "NOT_FOUND",
                format!("No akt `{number}`"),
                None,
            ));
        }
        let entries = |r: &Json, key: &str| -> Vec<String> {
            r["membership"][key]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e["number"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let events = data["akt"]["revisions"]
            .as_array()
            .map(|revisions| {
                revisions
                    .iter()
                    .map(|r| Event {
                        at: r["createdAt"]
                            .as_str()
                            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                            .map(|t| t.with_timezone(&chrono::Utc))
                            .unwrap_or_default(),
                        number: number.to_owned(),
                        revision: r["revision"].as_i64().unwrap_or_default(),
                        by: format!(
                            "{} <{}>",
                            r["by"]["name"].as_str().unwrap_or("Kuria"),
                            r["by"]["email"].as_str().map_or_else(
                                || format!("{}@kuria", r["by"]["id"].as_str().unwrap_or("someone")),
                                str::to_owned
                            )
                        ),
                        reason: r["reason"].as_str().map(str::to_owned),
                        commit: r.get("commit").filter(|c| !c.is_null()).cloned(),
                        added: entries(r, "added_member"),
                        removed: entries(r, "removed_member"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let versions = data["akt"]["versions"]
            .as_array()
            .map(|v| {
                v.iter()
                    .map(|v| {
                        (
                            v["number"].as_i64().unwrap_or_default(),
                            v["revision"].as_i64().unwrap_or_default(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok((events, versions))
    }

    /// The akt `number` at `revision`, as its clone file says it, with its
    /// shape, and its title and its value of its `sections` field, if any.
    async fn file_at(&self, number: &str, revision: i64) -> Result<AtRevision, CliError> {
        let data = self
            .session
            .graphql(
                &format!(
                    "query($n: String!, $r: Int) {{ akt(number: $n, revision: $r) {{ number title state {{ key }} type {{ ...Shape }} fields {{ ...Values }} }} }} {SHAPE} {VALUES} {SOMEONE}"
                ),
                json!({ "n": number, "r": revision }),
            )
            .await?;
        let akt = &data["akt"];
        let shape: Shape = read(akt["type"].clone(), "the akt type")?;
        let fields: Vec<Value> = read(akt["fields"].clone(), "the fields")?;
        let values: Map<String, Json> = fields
            .iter()
            .map(|v| (v.field().to_owned(), file_value(v)))
            .collect();
        let sections = fields.iter().find_map(|v| match v {
            Value::SectionsValue { value, .. } => Some(value.clone()),
            _ => None,
        });
        let text = file::clone_text(
            &shape,
            number,
            akt["state"]["key"].as_str().unwrap_or_default(),
            &values,
        );
        Ok((
            shape,
            text,
            akt["title"].as_str().map(str::to_owned),
            sections,
        ))
    }

    /// The container's history, built from Kuria's revisions, the same way
    /// for every clone (`git.history`, `git.identity`).
    // r[impl git.history]
    // r[impl git.identity]
    pub async fn history(&self) -> Result<History, CliError> {
        let (container_events, container_versions) = self.revisions_of(&self.number).await?;
        let mut members: Vec<String> = Vec::new();
        for e in &container_events {
            for n in &e.added {
                if !members.contains(n) {
                    members.push(n.clone());
                }
            }
        }
        let mut events = container_events;
        let mut versions = vec![(self.number.clone(), container_versions)];
        for member in &members {
            let (e, v) = self.revisions_of(member).await?;
            events.extend(e);
            versions.push((member.clone(), v));
        }
        events.sort_by(|a, b| (a.at, &a.number, a.revision).cmp(&(b.at, &b.number, b.revision)));
        let groups = grouped(events);
        let mut cache: HashMap<(String, i64), AtRevision> = HashMap::new();
        let mut revisions: HashMap<String, i64> = HashMap::new();
        let mut present: Vec<String> = Vec::new();
        let mut commits = Vec::new();
        for group in groups {
            for e in &group {
                revisions.insert(e.number.clone(), e.revision);
                if e.number == self.number {
                    for n in &e.added {
                        if !present.contains(n) {
                            present.push(n.clone());
                        }
                    }
                    present.retain(|n| !e.removed.contains(n));
                }
            }
            let mut wanted = vec![self.number.clone()];
            wanted.extend(present.iter().cloned());
            for number in &wanted {
                if let Some(&r) = revisions.get(number)
                    && !cache.contains_key(&(number.clone(), r))
                {
                    let read = self.file_at(number, r).await?;
                    cache.insert((number.clone(), r), read);
                }
            }
            let file = |n: &str| {
                revisions
                    .get(n)
                    .and_then(|r| cache.get(&(n.to_owned(), *r)))
            };
            let tree = match file(&self.number) {
                Some((_, text, title, sections)) => {
                    let placed: Vec<(String, String, Option<String>)> = present
                        .iter()
                        .filter_map(|n| {
                            file(n)
                                .map(|(_, text, title, _)| (n.clone(), text.clone(), title.clone()))
                        })
                        .collect();
                    tree_of(
                        &self.number,
                        text,
                        title.as_deref(),
                        sections.as_ref(),
                        &placed,
                    )
                }
                None => BTreeMap::new(),
            };
            let (author, committer, message) = metadata(&group);
            commits.push(Built {
                author,
                committer,
                message,
                tree,
                revisions: revisions.clone(),
            });
        }
        let tags = tags_of(&versions, &commits);
        let seen = revisions.into_iter().collect();
        Ok(History {
            commits,
            tags,
            seen,
        })
    }
}

/// Revisions as commits: one per pushed commit, else per change, what one
/// person did in the same second for the same reason.
fn grouped(events: Vec<Event>) -> Vec<Vec<Event>> {
    let mut groups: Vec<Vec<Event>> = Vec::new();
    for e in events {
        let joins = groups.last().and_then(|g| g.last()).is_some_and(|last| {
            match (&last.commit, &e.commit) {
                (Some(a), Some(b)) => a == b,
                (None, None) => {
                    last.at.timestamp() == e.at.timestamp()
                        && last.by == e.by
                        && last.reason == e.reason
                }
                _ => false,
            }
        });
        if joins {
            groups.last_mut().expect("joined").push(e);
        } else {
            groups.push(vec![e]);
        }
    }
    groups
}

/// A commit's author, committer and message: the pushed commit's, if it was
/// pushed, else its first revision's author, time and reason.
fn metadata(group: &[Event]) -> (String, String, String) {
    let first = &group[0];
    if let Some(c) = &first.commit {
        return (
            c["author"].as_str().unwrap_or_default().to_owned(),
            c["committer"].as_str().unwrap_or_default().to_owned(),
            c["message"].as_str().unwrap_or_default().to_owned(),
        );
    }
    let who = format!("{} {} +0000", first.by, first.at.timestamp());
    let mut numbers: Vec<&str> = group.iter().map(|e| e.number.as_str()).collect();
    numbers.dedup();
    let message = first
        .reason
        .clone()
        .unwrap_or_else(|| format!("Change {}", numbers.join(", ")));
    (who.clone(), who, format!("{}\n", message.trim_end()))
}

/// Each version's tag, on the first commit with its revision.
fn tags_of(versions: &[(String, Vec<(i64, i64)>)], commits: &[Built]) -> Vec<(String, usize)> {
    let mut tags = Vec::new();
    for (number, list) in versions {
        for (n, revision) in list {
            if let Some(i) = commits
                .iter()
                .position(|c| c.revisions.get(number).is_some_and(|r| r >= revision))
            {
                tags.push((format!("{number}/v{n}"), i));
            }
        }
    }
    tags
}

/// Where each file of a container and its members goes (`git.tree`): the
/// container's at the root, each member's in the directories its
/// `sections` field puts it in.
// r[impl git.tree]
#[must_use]
pub fn tree_of(
    number: &str,
    text: &str,
    title: Option<&str>,
    sections: Option<&Json>,
    members: &[(String, String, Option<String>)],
) -> BTreeMap<String, String> {
    let mut tree = BTreeMap::new();
    tree.insert(file_name(number, title), text.to_owned());
    let mut dirs: HashMap<String, String> = HashMap::new();
    if let Some(items) = sections.and_then(Json::as_array) {
        place(items, "", &mut dirs);
    }
    for (n, text, title) in members {
        let dir = dirs.get(n).cloned().unwrap_or_default();
        tree.insert(
            format!("{dir}{}", file_name(n, title.as_deref())),
            text.clone(),
        );
    }
    tree
}

/// The directory each member of `items` goes in, under `at`.
fn place(items: &[Json], at: &str, dirs: &mut HashMap<String, String>) {
    let mut taken: HashMap<String, usize> = HashMap::new();
    for item in items {
        match item {
            Json::String(n) => {
                dirs.insert(n.clone(), at.to_owned());
            }
            Json::Object(section) => {
                let base = section
                    .get("title")
                    .and_then(Json::as_str)
                    .map(slug)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "section".to_owned());
                let count = taken.entry(base.clone()).or_insert(0);
                *count += 1;
                let name = if *count == 1 {
                    base
                } else {
                    format!("{base}-{count}")
                };
                if let Some(inner) = section.get("items").and_then(Json::as_array) {
                    place(inner, &format!("{at}{name}/"), dirs);
                }
            }
            _ => {}
        }
    }
}

/// `history` as a `git fast-import` stream onto `branch`.
#[must_use]
pub fn fast_import(history: &History, branch: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let data = |out: &mut Vec<u8>, bytes: &[u8]| {
        out.extend_from_slice(format!("data {}\n", bytes.len()).as_bytes());
        out.extend_from_slice(bytes);
        out.push(b'\n');
    };
    out.extend_from_slice(format!("feature done\nreset {branch}\n").as_bytes());
    for (i, c) in history.commits.iter().enumerate() {
        out.extend_from_slice(format!("commit {branch}\nmark :{}\n", i + 1).as_bytes());
        out.extend_from_slice(
            format!("author {}\ncommitter {}\n", c.author, c.committer).as_bytes(),
        );
        data(&mut out, c.message.as_bytes());
        out.extend_from_slice(b"deleteall\n");
        for (path, text) in &c.tree {
            out.extend_from_slice(format!("M 100644 inline {path}\n").as_bytes());
            data(&mut out, text.as_bytes());
        }
        out.push(b'\n');
    }
    for (tag, i) in &history.tags {
        out.extend_from_slice(format!("reset refs/tags/{tag}\nfrom :{}\n\n", i + 1).as_bytes());
    }
    out.extend_from_slice(b"done\n");
    out
}

/// A commit as `git fast-export` gives it.
#[derive(Debug, Default)]
pub struct Exported {
    pub author: String,
    pub committer: String,
    pub message: String,
    pub from: Option<String>,
    pub merges: usize,
    /// Path to its new text, `None` if deleted.
    pub changes: Vec<(String, Option<String>)>,
    pub deleteall: bool,
}

/// Reads a `git fast-export` stream, up to `done`: its commits, in order.
// r[impl git.push]
pub fn read_export(input: &mut impl BufRead) -> Result<Vec<Exported>, CliError> {
    let bad =
        |what: &str| CliError::refused("BAD_REQUEST", format!("git's fast-export stream: {what}"));
    let mut line = String::new();
    let mut blobs: HashMap<String, String> = HashMap::new();
    let mut commits = Vec::new();
    let read_data = |input: &mut dyn BufRead, header: &str| -> Result<String, CliError> {
        let n: usize = header
            .strip_prefix("data ")
            .and_then(|n| n.trim().parse().ok())
            .ok_or_else(|| bad("expected data"))?;
        let mut bytes = vec![0; n];
        input
            .read_exact(&mut bytes)
            .map_err(|_| bad("short data"))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    };
    // The optional newline after a blob's or a file's data.
    let skip_newline = |input: &mut dyn BufRead| {
        if input.fill_buf().is_ok_and(|b| b.first() == Some(&b'\n')) {
            input.consume(1);
        }
    };
    let next = |input: &mut dyn BufRead, line: &mut String| -> Result<bool, CliError> {
        line.clear();
        Ok(input.read_line(line).map_err(|_| bad("unreadable"))? > 0)
    };
    while next(input, &mut line)? {
        let l = line.trim_end_matches('\n').to_owned();
        if l == "done" {
            break;
        } else if l == "blob" {
            next(input, &mut line)?;
            let mark = line
                .trim()
                .strip_prefix("mark ")
                .unwrap_or_default()
                .to_owned();
            next(input, &mut line)?;
            let text = read_data(input, line.trim_end())?;
            skip_newline(input);
            blobs.insert(mark, text);
        } else if l.starts_with("commit ") {
            let mut c = Exported::default();
            loop {
                if !next(input, &mut line)? {
                    break;
                }
                let l = line.trim_end_matches('\n').to_owned();
                if l.is_empty() {
                    break;
                } else if let Some(rest) = l.strip_prefix("author ") {
                    rest.clone_into(&mut c.author);
                } else if let Some(rest) = l.strip_prefix("committer ") {
                    rest.clone_into(&mut c.committer);
                } else if l.starts_with("data ") {
                    c.message = read_data(input, &l)?;
                } else if let Some(rest) = l.strip_prefix("from ") {
                    c.from = Some(rest.to_owned());
                } else if l.starts_with("merge ") {
                    c.merges += 1;
                } else if l == "deleteall" {
                    c.deleteall = true;
                } else if let Some(rest) = l.strip_prefix("M ") {
                    let mut parts = rest.splitn(3, ' ');
                    let (_mode, dataref, path) = (
                        parts.next(),
                        parts.next().unwrap_or_default(),
                        parts.next().unwrap_or_default(),
                    );
                    let text = if dataref == "inline" {
                        next(input, &mut line)?;
                        let text = read_data(input, line.trim_end())?;
                        skip_newline(input);
                        text
                    } else {
                        blobs
                            .get(dataref)
                            .cloned()
                            .ok_or_else(|| bad("a blob it didn't give"))?
                    };
                    c.changes.push((unquote(path), Some(text)));
                } else if let Some(path) = l.strip_prefix("D ") {
                    c.changes.push((unquote(path), None));
                }
            }
            commits.push(c);
        }
    }
    Ok(commits)
}

fn unquote(path: &str) -> String {
    path.trim_matches('"').to_owned()
}

/// `git args…` in the current repository, its output.
fn git(args: &[&str]) -> Result<String, CliError> {
    let out = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| CliError::unavailable(format!("git: {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(CliError::unavailable(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

/// The tree of `commit`: path to text.
fn tree_at(commit: &str) -> Result<BTreeMap<String, String>, CliError> {
    let mut tree = BTreeMap::new();
    for path in git(&["ls-tree", "-r", "--name-only", commit])?.lines() {
        tree.insert(
            path.to_owned(),
            git(&["show", &format!("{commit}:{path}")])?,
        );
    }
    Ok(tree)
}

/// Where the helper keeps what it saw of Kuria at its last fetch.
fn seen_path(remote: &str) -> PathBuf {
    let dir = std::env::var("GIT_DIR").unwrap_or_else(|_| ".git".to_owned());
    Path::new(&dir).join("kuria").join(format!("{remote}.json"))
}

/// What a file says it is, and its values in the file's form.
fn read_file(text: &str) -> Result<(String, Parsed), CliError> {
    let parsed = file::parse(text)?;
    let number = parsed
        .text("number")
        .ok_or_else(|| CliError::refused("BAD_FILE", "a file names no `number`"))?;
    Ok((number, parsed))
}

/// Whether `text`, an akt's file in a clone, is in normal form: as Kuria
/// would write it, every heading with an id (`git.push.normal`).
// r[impl git.push.normal]
fn normal(shape: &Shape, text: &str) -> Result<String, CliError> {
    let (number, parsed) = read_file(text)?;
    let mut values = parsed.values()?;
    for field in &shape.fields {
        if field.kind == "SECTIONS"
            && let Some(Json::String(body)) = values.get(&field.key)
        {
            let read = crate::values::sections_from_text(body)
                .map_err(|e| CliError::refused("BAD_FILE", e))?;
            values.insert(field.key.clone(), read);
        }
        if field.kind == "MARKDOWN"
            && let Some(Json::String(body)) = values.get(&field.key)
        {
            let mut given = body.clone();
            kuria_engine::markdown::give_ids(&mut [&mut given])
                .map_err(|e| CliError::refused("BAD_FILE", e))?;
            values.insert(field.key.clone(), Json::String(given));
        }
    }
    let state = parsed.text("state").unwrap_or_default();
    Ok(file::clone_text(shape, &number, &state, &values))
}

/// The title a clone's file names it by.
fn title_in(shape: &Shape, parsed: &Parsed) -> Option<String> {
    let key = shape.presentation.title.clone().or_else(|| {
        shape
            .fields
            .iter()
            .find(|f| f.kind == "TEXT" && f.required && !f.multiline)
            .map(|f| f.key.clone())
    })?;
    parsed.text(&key)
}

impl Remote {
    async fn shape(
        &self,
        key: &str,
        shapes: &mut HashMap<String, Shape>,
    ) -> Result<Shape, CliError> {
        if let Some(shape) = shapes.get(key) {
            return Ok(shape.clone());
        }
        let shape = crate::reads::shape(crate::reads::Source::Server(&self.session), key).await?;
        shapes.insert(key.to_owned(), shape.clone());
        Ok(shape)
    }

    /// The tree as Kuria would write it, from `tree`'s files: each in normal
    /// form, where the container's `sections` puts it (`git.fmt`).
    pub async fn normal_tree(
        &self,
        tree: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>, CliError> {
        let mut shapes = HashMap::new();
        let mut container = None;
        let mut members = Vec::new();
        for text in tree.values() {
            let (number, parsed) = read_file(text)?;
            let key = parsed.text("type").unwrap_or_default();
            let shape = self.shape(&key, &mut shapes).await?;
            let normal = normal(&shape, text)?;
            let title = title_in(&shape, &parsed);
            if number == self.number {
                let sections = shape
                    .fields
                    .iter()
                    .find(|f| f.kind == "SECTIONS")
                    .and_then(|f| parsed.values().ok()?.get(&f.key).cloned())
                    .and_then(|v| v.as_str().map(crate::values::sections_from_text))
                    .and_then(Result::ok);
                container = Some((normal, title, sections));
            } else {
                members.push((number, normal, title));
            }
        }
        let Some((text, title, sections)) = container else {
            return Err(CliError::refused(
                "BAD_FILE",
                format!("the clone has no file of {}", self.number),
            ));
        };
        Ok(tree_of(
            &self.number,
            &text,
            title.as_deref(),
            sections.as_ref(),
            &members,
        ))
    }

    /// Applies each pushed commit as one batch (`git.push`), refusing one
    /// that isn't a fast-forward, a merge, a new file, or files not in
    /// normal form.
    // r[impl git.push]
    // r[impl git.push.fast-forward]
    pub async fn push(&self, commits: &[Exported], tip: &str) -> Result<(), CliError> {
        let refuse = |why: String| CliError::refused("REFUSED", why);
        let seen: BTreeMap<String, i64> = std::fs::read_to_string(seen_path(&self.name))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if self.history().await?.seen != seen {
            return Err(refuse("Kuria changed since your last fetch: not a fast-forward; `git pull --rebase` and push again".to_owned()));
        }
        let mut tree = tree_at(tip)?;
        for (i, commit) in commits.iter().enumerate() {
            if commit.merges > 0 {
                return Err(refuse(
                    "a merge commit can't be pushed: rebase instead".to_owned(),
                ));
            }
            if i == 0
                && commit.from.as_deref().is_some_and(|f| {
                    f != tip && !git(&["rev-parse", f]).is_ok_and(|s| s.trim() == tip)
                })
            {
                return Err(refuse(
                    "not a fast-forward: `git pull --rebase` and push again".to_owned(),
                ));
            }
            let before = tree.clone();
            if commit.deleteall {
                tree.clear();
            }
            for (path, text) in &commit.changes {
                match text {
                    Some(text) => tree.insert(path.clone(), text.clone()),
                    None => tree.remove(path),
                };
            }
            let normal = self.normal_tree(&tree).await?;
            if normal != tree {
                let differ: Vec<&String> = normal
                    .keys()
                    .chain(tree.keys())
                    .filter(|p| normal.get(*p) != tree.get(*p))
                    .collect();
                return Err(refuse(format!(
                    "a commit's files aren't as Kuria writes them ({}): run `kuria fmt`, amend and push again",
                    differ
                        .iter()
                        .map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
            self.apply(&before, &tree, commit).await?;
        }
        let history = self.history().await?;
        write_seen(&self.name, &history.seen)?;
        Ok(())
    }

    /// One commit's changes, from `before` to `after`, as one batch.
    async fn apply(
        &self,
        before: &BTreeMap<String, String>,
        after: &BTreeMap<String, String>,
        commit: &Exported,
    ) -> Result<(), CliError> {
        let numbers =
            |tree: &BTreeMap<String, String>| -> Result<BTreeMap<String, String>, CliError> {
                tree.values()
                    .map(|t| read_file(t).map(|(n, _)| (n, t.clone())))
                    .collect()
            };
        let (old, new) = (numbers(before)?, numbers(after)?);
        let mut operations = Vec::new();
        for (number, text) in &new {
            match old.get(number) {
                None => {
                    return Err(CliError::refused(
                        "REFUSED",
                        format!(
                            "{number} is new: create it with `kuria new` (its number comes from Kuria), fetch, then edit it"
                        ),
                    ));
                }
                Some(was) if was == text => {}
                Some(_) => {
                    let akt = crate::writes::akt(&self.session, number).await?;
                    let parsed = file::parse(text)?;
                    crate::writes::edit::check_read_only(&akt, &parsed)?;
                    let fields = crate::writes::edit::changes(&self.session, &akt, &parsed).await?;
                    if !fields.is_empty() {
                        operations.push(json!({ "editAkt": { "number": number, "revision": akt.revision, "fields": fields, "reason": commit.message.trim() } }));
                    }
                }
            }
        }
        for number in old.keys().filter(|n| !new.contains_key(*n)) {
            if *number == self.number {
                return Err(CliError::refused(
                    "REFUSED",
                    "the container's own file can't be removed",
                ));
            }
            operations.push(json!({ "removeFromContainer": { "container": self.number, "member": number, "reason": commit.message.trim() } }));
        }
        if operations.is_empty() {
            return Ok(());
        }
        self.session
            .graphql(
                "mutation($input: BatchInput!) { batch(input: $input) { number } }",
                json!({ "input": {
                    "operations": operations,
                    "commit": { "author": commit.author, "committer": commit.committer, "message": commit.message },
                } }),
            )
            .await?;
        Ok(())
    }
}

fn write_seen(remote: &str, seen: &BTreeMap<String, i64>) -> Result<(), CliError> {
    let path = seen_path(remote);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| crate::profile::io_error(dir, &e))?;
    }
    std::fs::write(&path, serde_json::to_string(seen).unwrap_or_default())
        .map_err(|e| crate::profile::io_error(&path, &e))
}

/// The remote helper's conversation with git, on standard input and
/// output (`git.remote`).
// r[impl git.remote]
// r[impl git.fetch]
pub async fn serve(remote: &Remote) -> Result<(), CliError> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut out = std::io::stdout();
    let branch = format!("refs/kuria/{}/heads/main", remote.name);
    let mut line = String::new();
    loop {
        line.clear();
        if input
            .read_line(&mut line)
            .map_err(|e| CliError::unavailable(e.to_string()))?
            == 0
        {
            return Ok(());
        }
        let command = line.trim().to_owned();
        let said = match command.as_str() {
            "" => return Ok(()),
            "capabilities" => format!(
                "import\nexport\nrefspec refs/heads/*:refs/kuria/{}/heads/*\n\n",
                remote.name
            ),
            c if c.starts_with("list") => "? refs/heads/main\n@refs/heads/main HEAD\n\n".to_owned(),
            c if c.starts_with("option") => "unsupported\n".to_owned(),
            c if c.starts_with("import") => {
                // The rest of the batch of imports, up to a blank line.
                loop {
                    line.clear();
                    if input
                        .read_line(&mut line)
                        .map_err(|e| CliError::unavailable(e.to_string()))?
                        == 0
                        || line.trim().is_empty()
                    {
                        break;
                    }
                }
                let history = remote.history().await?;
                write_seen(&remote.name, &history.seen)?;
                out.write_all(&fast_import(&history, &branch))
                    .map_err(|e| CliError::unavailable(e.to_string()))?;
                out.flush().ok();
                continue;
            }
            "export" => {
                let commits = read_export(&mut input)?;
                let tip = git(&["rev-parse", &branch])?.trim().to_owned();
                match remote.push(&commits, &tip).await {
                    Ok(()) => "ok refs/heads/main\n\n".to_owned(),
                    Err(e) => format!("error refs/heads/main {}\n\n", e.message.replace('\n', " ")),
                }
            }
            other => {
                return Err(CliError::refused(
                    "BAD_REQUEST",
                    format!("git asked `{other}`, which the helper doesn't know"),
                ));
            }
        };
        out.write_all(said.as_bytes())
            .map_err(|e| CliError::unavailable(e.to_string()))?;
        out.flush().ok();
    }
}

/// `kuria fmt` in a clone: its files in normal form, where the container's
/// `sections` puts them (`git.fmt`).
// r[impl git.fmt]
pub async fn fmt(remote: &Remote, root: &Path) -> Result<crate::output::Answer, CliError> {
    let listed = git(&["-C", &root.to_string_lossy(), "ls-files"])?;
    let mut tree = BTreeMap::new();
    for path in listed.lines().filter(|p| {
        Path::new(p)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
    }) {
        let full = root.join(path);
        let text =
            std::fs::read_to_string(&full).map_err(|e| crate::profile::io_error(&full, &e))?;
        tree.insert(path.to_owned(), text);
    }
    let normal = remote.normal_tree(&tree).await?;
    let mut changed = Vec::new();
    for (path, text) in &tree {
        if normal.get(path) != Some(text) {
            let full = root.join(path);
            std::fs::remove_file(&full).map_err(|e| crate::profile::io_error(&full, &e))?;
            changed.push(path.clone());
        }
    }
    for (path, text) in &normal {
        if tree.get(path) != Some(text) {
            let full = root.join(path);
            if let Some(dir) = full.parent() {
                std::fs::create_dir_all(dir).map_err(|e| crate::profile::io_error(dir, &e))?;
            }
            std::fs::write(&full, text).map_err(|e| crate::profile::io_error(&full, &e))?;
            if !changed.contains(path) {
                changed.push(path.clone());
            }
        }
    }
    // Directories left empty go too.
    for path in &changed {
        let mut dir = root.join(path);
        while dir.pop() && dir != root {
            if std::fs::remove_dir(&dir).is_err() {
                break;
            }
        }
    }
    Ok(crate::output::Answer {
        markdown: if changed.is_empty() {
            "Every file is in normal form.".to_owned()
        } else {
            format!("Put in normal form: {}.", changed.join(", "))
        },
        json: json!({ "changed": changed }),
    })
}

/// Reads all of standard input; for tests.
pub fn read_all(input: &mut impl Read) -> String {
    let mut text = String::new();
    input.read_to_string(&mut text).ok();
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify git.remote]
    #[test]
    fn reads_a_remote_s_url() {
        assert_eq!(
            split_url("kuria::https://kuria.example/DOCS-1").unwrap(),
            ("https://kuria.example".to_owned(), "DOCS-1".to_owned())
        );
        assert!(split_url("kuria::DOCS-1").is_err());
    }

    // r[verify git.tree]
    #[test]
    fn places_members_where_the_sections_put_them() {
        let sections = json!([
            { "title": "Guides", "items": ["D-2", { "title": "Deep", "items": ["D-3"] }] },
            { "items": ["D-4"] },
            { "title": "Guides", "items": ["D-5"] }
        ]);
        let member = |n: &str| (n.to_owned(), format!("{n}\n"), Some("Doc".to_owned()));
        let tree = tree_of(
            "D-1",
            "index\n",
            Some("Index"),
            Some(&sections),
            &[
                member("D-2"),
                member("D-3"),
                member("D-4"),
                member("D-5"),
                member("D-6"),
            ],
        );
        let paths: Vec<&str> = tree.keys().map(String::as_str).collect();
        assert_eq!(
            paths,
            [
                "D-1-index.md",
                "D-6-doc.md",
                "guides-2/D-5-doc.md",
                "guides/D-2-doc.md",
                "guides/deep/D-3-doc.md",
                "section/D-4-doc.md",
            ]
        );
    }

    // r[verify git.push]
    #[test]
    fn reads_what_git_exports() {
        let stream = "feature done\nblob\nmark :1\ndata 6\nhello\n\ncommit refs/heads/main\nmark :2\nauthor Ada <ada@x> 1 +0000\ncommitter Ada <ada@x> 1 +0000\ndata 4\nmsg\nfrom 0123\nM 100644 :1 a.md\nM 100644 inline b.md\ndata 3\nbye\nD c.md\n\ndone\n";
        let commits = read_export(&mut stream.as_bytes()).unwrap();
        assert_eq!(commits.len(), 1);
        let c = &commits[0];
        assert_eq!(c.author, "Ada <ada@x> 1 +0000");
        assert_eq!(c.message, "msg\n");
        assert_eq!(c.from.as_deref(), Some("0123"));
        assert_eq!(
            c.changes,
            [
                ("a.md".to_owned(), Some("hello\n".to_owned())),
                ("b.md".to_owned(), Some("bye".to_owned())),
                ("c.md".to_owned(), None)
            ]
        );
    }
}
