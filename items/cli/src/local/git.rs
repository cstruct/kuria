//! What git knows of an akt directory: the commits that changed each akt,
//! and so when it was created and last changed (`cli.local.history`).
//!
//! A file may hold several akter (`cli.local.nesting`), so git's history of
//! a file isn't an akt's. An akt changes exactly when its `revision` does,
//! so each commit that touched `akter/` is read for the revisions its files
//! hold, and an akt is followed by its number into whichever file holds it.

use std::collections::{HashMap, HashSet};
use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};

use chrono::{DateTime, SecondsFormat, Utc};

use super::dir::LocalAkt;
use crate::file::split;

/// A commit that changed an akt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub author: String,
    pub email: String,
    /// In RFC 3339, UTC.
    pub at: String,
    pub subject: String,
}

/// `git args…` in `root`, its output if git answered.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("TZ", "UTC")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The blobs `ids` name, by id, read in one `git cat-file --batch`.
fn blobs(root: &Path, ids: &HashSet<String>) -> HashMap<String, String> {
    let mut found = HashMap::new();
    if ids.is_empty() {
        return found;
    }
    let Ok(mut child) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return found;
    };
    let mut stdin = child.stdin.take().expect("piped");
    let input = ids.iter().fold(String::new(), |mut s, id| {
        s.push_str(id);
        s.push('\n');
        s
    });
    // Written from a thread, so that a full pipe can't stall both sides.
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let mut out = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        drop(stdout.read_to_end(&mut out));
    }
    drop(writer.join());
    drop(child.wait());
    let mut at = 0;
    while let Some(end) = out[at..].iter().position(|b| *b == b'\n') {
        let header = String::from_utf8_lossy(&out[at..at + end]).into_owned();
        at += end + 1;
        let mut words = header.split(' ');
        let (Some(id), Some("blob"), Some(size)) = (words.next(), words.next(), words.next())
        else {
            continue;
        };
        let Ok(size) = size.parse::<usize>() else {
            continue;
        };
        let Some(bytes) = out.get(at..at + size) else {
            break;
        };
        found.insert(id.to_owned(), String::from_utf8_lossy(bytes).into_owned());
        at += size + 1;
    }
    found
}

/// The akter a file's `text` holds, with their revisions; parts that
/// don't say are left out.
fn revisions(text: &str) -> Vec<(String, i32)> {
    let text = text.replace("\r\n", "\n");
    split(&text)
        .into_iter()
        .filter_map(|(nested, part)| {
            let front = part.strip_prefix("---\n")?;
            let front = &front[..front.find("\n---").unwrap_or(front.len())];
            let value = |key: &str| {
                front.lines().find_map(|l| {
                    l.strip_prefix(key)
                        .map(|v| v.trim().trim_matches('"').to_owned())
                })
            };
            let number = nested.map(str::to_owned).or_else(|| value("number:"))?;
            let revision = value("revision:")?.parse().ok()?;
            Some((number, revision))
        })
        .collect()
}

/// `text`, a time git wrote, in RFC 3339 UTC.
fn utc(text: &str) -> String {
    DateTime::parse_from_rfc3339(text.trim()).map_or_else(
        |_| text.trim().to_owned(),
        |t| {
            t.with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        },
    )
}

/// When `path` was last written, in RFC 3339 UTC.
fn modified(path: &Path) -> String {
    let at: DateTime<Utc> = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map_or_else(|_| Utc::now(), DateTime::from);
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// A commit as `git log` said it, with the files of `akter/` it changed:
/// each path, and the blob it left there, `None` if it deleted it.
struct Logged {
    commit: Commit,
    files: Vec<(String, Option<String>)>,
}

/// `git log` of `akter/`, oldest first; `None` outside a work tree.
fn log(root: &Path) -> Option<Vec<Logged>> {
    git(root, &["rev-parse", "--is-inside-work-tree"])?;
    // No commits yet is a history with nothing in it.
    let Some(out) = git(
        root,
        &[
            "log",
            "--reverse",
            "--no-renames",
            "--raw",
            "--no-abbrev",
            "--relative",
            "--diff-merges=first-parent",
            "--date=iso-strict-local",
            "--format=%x00%an%x1f%ae%x1f%cd%x1f%s",
            "--",
            "akter",
        ],
    ) else {
        return Some(Vec::new());
    };
    let mut logged = Vec::new();
    for chunk in out.split('\0').filter(|c| !c.trim().is_empty()) {
        let mut lines = chunk.lines();
        let header: Vec<&str> = lines.next().unwrap_or_default().split('\u{1f}').collect();
        let [author, email, at, subject] = header[..] else {
            continue;
        };
        let files = lines
            .filter_map(|line| {
                // `:<mode> <mode> <blob> <blob> <status>\t<path>`
                let (meta, path) = line.strip_prefix(':')?.split_once('\t')?;
                let words: Vec<&str> = meta.split_whitespace().collect();
                let blob = words.get(3)?;
                let deleted = words.get(4)?.starts_with('D');
                Some((path.to_owned(), (!deleted).then(|| (*blob).to_owned())))
            })
            .collect();
        logged.push(Logged {
            commit: Commit {
                author: author.to_owned(),
                email: email.to_owned(),
                at: utc(at),
                subject: subject.to_owned(),
            },
            files,
        });
    }
    Some(logged)
}

/// What git says of each akt of a directory: the commits that changed it,
/// with its revision after each, and its revision at `HEAD`.
#[derive(Debug, Default)]
pub struct Times {
    tracked: bool,
    changes: HashMap<String, Vec<(Commit, i32)>>,
    head: HashMap<String, i32>,
    /// Per akt and revision, the text of the file that first held it at
    /// that revision in a commit, and the file's path.
    texts: HashMap<(String, i32), (String, String)>,
}

impl Times {
    /// What git says of `root`'s `akter/`; nothing outside a work tree.
    // r[impl cli.local.history]
    #[must_use]
    pub fn of(root: &Path) -> Self {
        let Some(logged) = log(root) else {
            return Self::default();
        };
        // The files at HEAD, as `ls-tree` says, whatever merges did.
        let at_head: Vec<String> = git(root, &["ls-tree", "-r", "HEAD", "--", "akter"])
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                l.split_once('\t')?
                    .0
                    .split_whitespace()
                    .nth(2)
                    .map(str::to_owned)
            })
            .collect();
        let ids: HashSet<String> = logged
            .iter()
            .flat_map(|l| l.files.iter().filter_map(|(_, blob)| blob.clone()))
            .chain(at_head.iter().cloned())
            .collect();
        let texts = blobs(root, &ids);
        let mut times = Self {
            tracked: true,
            ..Self::default()
        };
        for l in logged {
            let touched: Vec<(String, i32)> = l
                .files
                .iter()
                .filter_map(|(path, blob)| Some((path, texts.get(blob.as_ref()?)?)))
                .flat_map(|(path, t)| revisions(t).into_iter().map(move |(n, r)| (path, t, n, r)))
                .map(|(path, t, n, r)| {
                    times
                        .texts
                        .entry((n.clone(), r))
                        .or_insert_with(|| (path.clone(), t.clone()));
                    (n, r)
                })
                .collect();
            // An akt changed when it reached a revision it hadn't, so that
            // one moved between files, or seen again on a merge, didn't.
            for (number, revision) in touched {
                let changes = times.changes.entry(number).or_default();
                if changes.last().is_none_or(|(_, r)| revision > *r) {
                    changes.push((l.commit.clone(), revision));
                }
            }
        }
        for (number, revision) in at_head
            .iter()
            .filter_map(|b| texts.get(b))
            .flat_map(|t| revisions(t))
        {
            let head = times.head.entry(number).or_insert(revision);
            *head = (*head).max(revision);
        }
        times
    }

    /// The path and text of the file a commit held `number` at `revision`
    /// in, if git has it (`cli.local.versions`).
    #[must_use]
    pub fn text_at(&self, number: &str, revision: i32) -> Option<(&str, &str)> {
        self.texts
            .get(&(number.to_owned(), revision))
            .map(|(path, text)| (path.as_str(), text.as_str()))
    }

    /// Whether `akt` has changes not committed yet.
    #[must_use]
    pub fn dirty(&self, akt: &LocalAkt) -> bool {
        self.head.get(&akt.number) != Some(&akt.revision)
    }

    /// When `akt` was created and last changed.
    #[must_use]
    pub fn of_akt(&self, akt: &LocalAkt) -> (String, String) {
        let written = || modified(&akt.path);
        let Some(changes) = self.changes.get(&akt.number).filter(|_| self.tracked) else {
            return (written(), written());
        };
        let created = changes.first().map_or_else(written, |(c, _)| c.at.clone());
        let changed = if self.dirty(akt) {
            written()
        } else {
            changes.last().map_or_else(written, |(c, _)| c.at.clone())
        };
        (created, changed)
    }

    /// The commits that changed `akt`, oldest first, each with its revision
    /// after it, and whether it has changes not committed yet; `None`
    /// outside a work tree.
    // r[impl cli.local.history]
    #[must_use]
    pub fn history(&self, akt: &LocalAkt) -> Option<(Vec<(Commit, i32)>, bool)> {
        self.tracked.then(|| {
            (
                self.changes.get(&akt.number).cloned().unwrap_or_default(),
                self.dirty(akt),
            )
        })
    }
}

/// The email `git config user.email` gives in `root`, if any.
#[must_use]
pub fn email(root: &Path) -> Option<String> {
    git(root, &["config", "user.email"])
        .map(|e| e.trim().to_owned())
        .filter(|e| !e.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_revisions_a_file_holds() {
        let text = "---\nnumber: PLAN-001\ntype: plan\nrevision: 4\n---\n\n<!-- field: goal -->\n```\n<!-- akt: X-1 -->\nrevision: 9\n```\n\n<!-- akt: PLAN-001-T01 -->\n---\ntype: task\nrevision: 2\n---\n";
        assert_eq!(
            revisions(text),
            [("PLAN-001".to_owned(), 4), ("PLAN-001-T01".to_owned(), 2)]
        );
    }
}
