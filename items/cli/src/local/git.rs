//! What git knows of an akt directory: when each akt file was created and
//! last changed, and the commits that changed it (`cli.local.history`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, SecondsFormat, Utc};

/// A commit that changed an akt's file.
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

/// When each file under `root` was created and last changed: by its
/// commits, else, for one with changes not committed yet, when it was
/// written.
#[derive(Debug, Default)]
pub struct Times {
    committed: HashMap<PathBuf, (String, String)>,
    dirty: HashSet<PathBuf>,
}

impl Times {
    /// What git says of `root`'s `akter/`; nothing outside a work tree.
    #[must_use]
    pub fn of(root: &Path) -> Self {
        let mut times = Self::default();
        let Some(log) = git(
            root,
            &[
                "log",
                "--relative",
                "--date=iso-strict-local",
                "--format=%x00%cd",
                "--name-only",
                "--",
                "akter",
            ],
        ) else {
            return times;
        };
        let mut at = String::new();
        for line in log.lines() {
            if let Some(time) = line.strip_prefix('\0') {
                at = utc(time);
            } else if !line.trim().is_empty() {
                let entry = times
                    .committed
                    .entry(PathBuf::from(line.trim()))
                    .or_insert_with(|| (at.clone(), at.clone()));
                // Newest first: each older commit moves its creation back.
                entry.0.clone_from(&at);
            }
        }
        if let Some(status) = git(
            root,
            &["status", "--porcelain", "--relative", "--", "akter"],
        ) {
            times.dirty = status
                .lines()
                .filter_map(|l| l.get(3..))
                .map(|p| PathBuf::from(p.trim()))
                .collect();
        }
        times
    }

    /// When the akt file at `path` (under `root`) was created and changed.
    #[must_use]
    pub fn of_file(&self, root: &Path, path: &Path) -> (String, String) {
        let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        match self.committed.get(&relative) {
            Some((created, updated)) if !self.dirty.contains(&relative) => {
                (created.clone(), updated.clone())
            }
            Some((created, _)) => (created.clone(), modified(path)),
            None => (modified(path), modified(path)),
        }
    }
}

/// The commits that changed `path`, oldest first, and whether it has
/// changes not committed yet; `None` outside a work tree.
// r[impl cli.local.history]
#[must_use]
pub fn history(root: &Path, path: &Path) -> Option<(Vec<Commit>, bool)> {
    git(root, &["rev-parse", "--is-inside-work-tree"])?;
    let file = path.to_string_lossy();
    let log = git(
        root,
        &[
            "log",
            "--follow",
            "--reverse",
            "--date=iso-strict-local",
            "--format=%an%x1f%ae%x1f%cd%x1f%s",
            "--",
            &file,
        ],
    )
    .unwrap_or_default();
    let commits = log
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\u{1f}').collect();
            (parts.len() == 4).then(|| Commit {
                author: parts[0].to_owned(),
                email: parts[1].to_owned(),
                at: utc(parts[2]),
                subject: parts[3].to_owned(),
            })
        })
        .collect();
    let dirty =
        git(root, &["status", "--porcelain", "--", &file]).is_some_and(|s| !s.trim().is_empty());
    Some((commits, dirty))
}

/// The email `git config user.email` gives in `root`, if any.
#[must_use]
pub fn email(root: &Path) -> Option<String> {
    git(root, &["config", "user.email"])
        .map(|e| e.trim().to_owned())
        .filter(|e| !e.is_empty())
}
