//! What a clone keeps of Kuria in `.kuria/` (`clone.clone`): which Kuria
//! and container it is of, its base and what is staged.
//!
//! ```text
//! .kuria/
//!   .gitignore      `*`: git leaves the folder alone
//!   clone.json      { "url": …, "number": … }
//!   base.json       { "akter": { <number>: { "path", "revision", "live"? } } }
//!   base/<path>     each file and picture as Kuria last had it
//!   staged/<path>   each file as staged, to push
//!   pictures/<id>   pictures' bytes, kept so they are fetched once
//! ```
//!
//! An akt with a live draft's `live` holds its live draft's state as Kuria
//! last answered it (`live.state`), and its base, both base64: a push is
//! made against them.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::KURIA_DIR;
use super::fmt::{CLONE_FILE, files_in, root_of};
use super::pictures;
use crate::error::CliError;
use crate::profile::io_error;

/// The file holding each akt's base.
pub const BASE_FILE: &str = "base.json";
/// The directory holding the base's files.
pub const BASE_DIR: &str = "base";
/// The directory holding the staged files.
pub const STAGED_DIR: &str = "staged";

/// Which Kuria and container a clone is of: `.kuria/clone.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recorded {
    pub url: String,
    pub number: String,
}

/// A live draft as the base has it: its state, a Yjs v1 update, and its
/// base, a Yjs v1 snapshot, both base64.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveBase {
    pub state: String,
    pub base: String,
}

/// An akt as the base has it: where its file is, its revision, and its
/// live draft if it has one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AktBase {
    pub path: String,
    pub revision: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live: Option<LiveBase>,
}

/// Each akt of the base, by number: `.kuria/base.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bases {
    pub akter: BTreeMap<String, AktBase>,
}

impl Bases {
    /// The akt whose file is at `path`, if one is.
    #[must_use]
    pub fn at(&self, path: &str) -> Option<(&String, &AktBase)> {
        self.akter.iter().find(|(_, a)| a.path == path)
    }
}

/// A clone on disk: its root, what it is of, and its base.
pub struct Local {
    pub root: PathBuf,
    pub recorded: Recorded,
    pub bases: Bases,
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    serde_json::from_str(&text)
        .map_err(|e| CliError::refused("BAD_FILE", format!("{}: {e}", path.display())))
}

/// Writes `bytes` to `path`, making its directory if need be.
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
    }
    std::fs::write(path, bytes).map_err(|e| io_error(path, &e))
}

/// Removes the file at `path` if there is one, and the directories it
/// leaves empty up to `root`.
pub fn remove(root: &Path, path: &Path) -> Result<(), CliError> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(io_error(path, &e)),
    }
    let mut dir = path.to_path_buf();
    while dir.pop() && dir != root && dir.starts_with(root) {
        if std::fs::remove_dir(&dir).is_err() {
            break;
        }
    }
    Ok(())
}

impl Local {
    /// The clone `dir` is in, refused if it isn't in one.
    pub fn open(dir: &Path) -> Result<Self, CliError> {
        let root = root_of(dir);
        let kuria = root.join(KURIA_DIR);
        let clone_file = kuria.join(CLONE_FILE);
        if !clone_file.is_file() {
            return Err(CliError::refused(
                "NOT_A_CLONE",
                format!(
                    "{} isn't in a clone; make one with `kuria docs clone <number>`",
                    dir.display()
                ),
            ));
        }
        let recorded = read_json(&clone_file)?;
        let base_file = kuria.join(BASE_FILE);
        let bases = if base_file.is_file() {
            read_json(&base_file)?
        } else {
            Bases::default()
        };
        Ok(Self {
            root,
            recorded,
            bases,
        })
    }

    /// Whether `dir` is in a clone `kuria docs clone` made.
    #[must_use]
    pub fn is_in_one(dir: &Path) -> bool {
        root_of(dir).join(KURIA_DIR).join(CLONE_FILE).is_file()
    }

    /// Starts a clone at `root`, of `recorded`, with nothing in its base.
    pub fn start(root: &Path, recorded: Recorded) -> Result<Self, CliError> {
        let kuria = root.join(KURIA_DIR);
        write(&kuria.join(".gitignore"), b"*\n")?;
        let local = Self {
            root: root.to_path_buf(),
            recorded,
            bases: Bases::default(),
        };
        let text = serde_json::to_string_pretty(&local.recorded).expect("serializable");
        write(&kuria.join(CLONE_FILE), format!("{text}\n").as_bytes())?;
        let staged = kuria.join(STAGED_DIR);
        std::fs::create_dir_all(&staged).map_err(|e| io_error(&staged, &e))?;
        Ok(local)
    }

    fn kuria(&self) -> PathBuf {
        self.root.join(KURIA_DIR)
    }

    /// Writes `base.json`.
    pub fn save(&self) -> Result<(), CliError> {
        let text = serde_json::to_string_pretty(&self.bases).expect("serializable");
        write(
            &self.kuria().join(BASE_FILE),
            format!("{text}\n").as_bytes(),
        )
    }

    /// The working file at `path`, from the root, if there is one.
    #[must_use]
    pub fn working(&self, path: &str) -> Option<Vec<u8>> {
        std::fs::read(self.root.join(path)).ok()
    }

    /// Writes the working file at `path`, or removes it.
    pub fn set_working(&self, path: &str, bytes: Option<&[u8]>) -> Result<(), CliError> {
        let full = self.root.join(path);
        match bytes {
            Some(bytes) => write(&full, bytes),
            None => remove(&self.root, &full),
        }
    }

    /// The base's file at `path`, if it has one.
    #[must_use]
    pub fn base(&self, path: &str) -> Option<Vec<u8>> {
        std::fs::read(self.kuria().join(BASE_DIR).join(path)).ok()
    }

    /// The base's file at `path` as text, if it has one.
    #[must_use]
    pub fn base_text(&self, path: &str) -> Option<String> {
        self.base(path)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    /// Sets the base's file at `path`, or removes it.
    pub fn set_base(&self, path: &str, bytes: Option<&[u8]>) -> Result<(), CliError> {
        let dir = self.kuria().join(BASE_DIR);
        let full = dir.join(path);
        match bytes {
            Some(bytes) => write(&full, bytes),
            None => remove(&dir, &full),
        }
    }

    /// The pictures the base holds, by path.
    pub fn base_pictures(&self) -> Result<Vec<String>, CliError> {
        let dir = self.kuria().join(BASE_DIR);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        Ok(files_in(&dir)?
            .into_iter()
            .filter(|p| pictures::is_picture(p))
            .collect())
    }

    /// The staged file at `path`, if one is staged.
    #[must_use]
    pub fn staged(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(self.kuria().join(STAGED_DIR).join(path)).ok()
    }

    /// Stages `text` as the file at `path`, or unstages it.
    pub fn set_staged(&self, path: &str, text: Option<&str>) -> Result<(), CliError> {
        let dir = self.kuria().join(STAGED_DIR);
        let full = dir.join(path);
        match text {
            Some(text) => write(&full, text.as_bytes()),
            None => remove(&dir, &full),
        }
    }

    /// Every staged file's path.
    pub fn staged_paths(&self) -> Result<Vec<String>, CliError> {
        let dir = self.kuria().join(STAGED_DIR);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        files_in(&dir)
    }

    /// The text a file is compared with before it is staged: what is
    /// staged of it, else its base.
    #[must_use]
    pub fn index(&self, path: &str) -> Option<String> {
        self.staged(path).or_else(|| self.base_text(path))
    }

    /// `given`, a path as given from `here`, from the clone's root, with
    /// `/` between parts; `""` for the root. Refused if it is outside the
    /// clone.
    pub fn path_from(&self, here: &Path, given: &str) -> Result<String, CliError> {
        let full = here.join(given);
        let mut parts: Vec<String> = Vec::new();
        for part in full.components() {
            match part {
                Component::ParentDir => {
                    parts.pop();
                }
                Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
                Component::CurDir => {}
                Component::RootDir | Component::Prefix(_) => parts.clear(),
            }
        }
        let mut root: Vec<String> = Vec::new();
        for part in self.root.components() {
            match part {
                Component::Normal(p) => root.push(p.to_string_lossy().into_owned()),
                Component::ParentDir => {
                    root.pop();
                }
                _ => {}
            }
        }
        if parts.len() < root.len() || parts[..root.len()] != root[..] {
            return Err(CliError::refused(
                "BAD_REQUEST",
                format!("{given} is outside the clone at {}", self.root.display()),
            ));
        }
        Ok(parts[root.len()..].join("/"))
    }
}

/// Whether the file at `path`, from a clone's root, is `under`, a path from
/// the root that names it or a directory holding it; `""` holds every
/// file.
#[must_use]
pub fn is_under(path: &str, under: &str) -> bool {
    under.is_empty() || path == under || path.starts_with(&format!("{under}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify clone.clone]
    #[test]
    fn keeps_a_base_and_what_is_staged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut local = Local::start(
            root,
            Recorded {
                url: "https://kuria.example".to_owned(),
                number: "D-1".to_owned(),
            },
        )
        .unwrap();
        assert!(Local::is_in_one(&root.join("guides")));
        local.bases.akter.insert(
            "D-1".to_owned(),
            AktBase {
                path: "D-1-index.md".to_owned(),
                revision: 2,
                live: None,
            },
        );
        local.save().unwrap();
        local.set_base("D-1-index.md", Some(b"base\n")).unwrap();
        local
            .set_base("attachments/IMG-1.png", Some(b"png"))
            .unwrap();
        local.set_staged("D-1-index.md", Some("staged\n")).unwrap();
        let opened = Local::open(&root.join("guides")).unwrap();
        assert_eq!(opened.bases, local.bases);
        assert_eq!(opened.recorded.number, "D-1");
        assert_eq!(opened.index("D-1-index.md").as_deref(), Some("staged\n"));
        assert_eq!(opened.base_pictures().unwrap(), ["attachments/IMG-1.png"]);
        assert_eq!(opened.staged_paths().unwrap(), ["D-1-index.md"]);
        opened.set_staged("D-1-index.md", None).unwrap();
        assert_eq!(opened.index("D-1-index.md").as_deref(), Some("base\n"));
        assert_eq!(
            std::fs::read_to_string(root.join(".kuria/.gitignore")).unwrap(),
            "*\n"
        );
        // Paths as given, from the root.
        assert_eq!(
            opened
                .path_from(&root.join("guides"), "../D-1-index.md")
                .unwrap(),
            "D-1-index.md"
        );
        assert_eq!(opened.path_from(root, ".").unwrap(), "");
        assert!(opened.path_from(root, "../elsewhere").is_err());
        assert!(is_under("guides/D-2.md", "guides"));
        assert!(!is_under("guides-2/D-2.md", "guides"));
        assert!(Local::open(&tempfile::tempdir().unwrap().path().join("x")).is_err());
    }
}
