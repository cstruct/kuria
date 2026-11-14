//! Writes in local mode: the directory, loaded under its lock, changed by
//! the engine's rules as a server changes its akter, and the files a
//! change touched written whole (`cli.local.rules`, `cli.local.revision`,
//! `cli.local.lock`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use kuria_engine::rules::{AktRef, Broken};
use kuria_engine::{Change, Current};
use serde_json::{Map, Value as Json, json};

use super::answers;
use super::dir::{AktDir, LocalAkt, LocalType};
use super::values::{Ids, current};
use crate::error::{CliError, Kind};
use crate::file::{self, Parsed};
use crate::model::{Shape, read};
use crate::output::Answer;
use crate::profile::io_error;

/// How long a write waits for another to finish.
const WAIT: Duration = Duration::from_secs(10);

/// The directory's lock: `.kuria.lock`, held for one change.
struct Lock(PathBuf);

impl Lock {
    // r[impl cli.local.lock]
    fn take(root: &Path) -> Result<Self, CliError> {
        let path = root.join(".kuria.lock");
        let started = Instant::now();
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Self(path)),
                Err(e)
                    if e.kind() == std::io::ErrorKind::AlreadyExists
                        && started.elapsed() < WAIT =>
                {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(CliError::new(
                        Kind::Unavailable,
                        "LOCKED",
                        format!(
                            "Another write holds {}; remove it if no `kuria` is writing",
                            path.display()
                        ),
                    ));
                }
                Err(e) => return Err(io_error(&path, &e)),
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // Gone already is as good as removed.
        drop(std::fs::remove_file(&self.0));
    }
}

/// A containment, link or ordering refusal, with the server's code.
pub(crate) fn broken(broken: &Broken) -> CliError {
    let code = match broken {
        Broken::NotContainable { .. } => "NOT_CONTAINABLE",
        Broken::AlreadyMember { .. } => "ALREADY_MEMBER",
        Broken::NotMember { .. } => "NOT_MEMBER",
        Broken::Exclusive { .. } => "EXCLUSIVE",
        Broken::Cycle { .. } => "CYCLE",
        Broken::Frozen(_) => "FROZEN",
        Broken::NotAccepting { .. } => "NOT_ACCEPTING",
        Broken::CreatedIn { .. } => "CREATED_IN",
        Broken::InvalidLink(_) => "INVALID_LINK",
        Broken::NotLinked { .. } => "NOT_FOUND",
        Broken::SelfMove => "BAD_REQUEST",
    };
    CliError::from_server(code, broken.to_string(), None)
}

impl From<Broken> for CliError {
    fn from(b: Broken) -> Self {
        broken(&b)
    }
}

/// An akt directory being changed: loaded under its lock, with what the
/// change touched written when it is saved.
pub struct Writer {
    pub dir: AktDir,
    pub ids: Ids,
    pub now: DateTime<Utc>,
    touched: BTreeSet<String>,
    types: BTreeSet<String>,
    _lock: Lock,
}

impl Writer {
    /// The akt directory at `root`, locked for one change.
    pub fn open(root: &Path, me: Option<String>) -> Result<Self, CliError> {
        let lock = Lock::take(root)?;
        let dir = AktDir::open(root)?;
        Ok(Self {
            dir,
            ids: Ids::new(me),
            now: Utc::now(),
            touched: BTreeSet::new(),
            types: BTreeSet::new(),
            _lock: lock,
        })
    }

    pub fn akt(&self, number: &str) -> Result<&LocalAkt, CliError> {
        self.dir
            .akt(number)
            .ok_or_else(|| CliError::from_server("NOT_FOUND", format!("No akt `{number}`"), None))
    }

    pub fn akt_mut(&mut self, number: &str) -> &mut LocalAkt {
        self.touched.insert(number.to_owned());
        self.dir
            .akter
            .iter_mut()
            .find(|a| a.number == number)
            .expect("found before it is changed")
    }

    /// Raises the revision of `number`, as every change to an akt does.
    // r[impl cli.local.revision]
    pub fn bump(&mut self, number: &str) -> i32 {
        let akt = self.akt_mut(number);
        akt.revision += 1;
        akt.revision
    }

    /// `number` as the rules see it.
    pub fn akt_ref(&self, number: &str) -> Result<AktRef<'_>, CliError> {
        let akt = self.akt(number)?;
        Ok(AktRef {
            number: &akt.number,
            state: &akt.state,
            ty: &self.dir.type_of(akt).ty,
            revision: akt.revision,
        })
    }

    /// The akt type `key`, as reads see it.
    pub fn shape(&self, key: &str) -> Result<Shape, CliError> {
        let t = self.dir.types.get(key).ok_or_else(|| {
            CliError::from_server(
                "UNKNOWN_AKT_TYPE",
                format!("No akt type `{key}`; see `kuria types`"),
                None,
            )
        })?;
        read(answers::shape(t), "the akt type")
    }

    pub fn change(&mut self) -> Change {
        Change {
            by: self.ids.changer(),
            at: self.now,
        }
    }

    /// The engine's view of `number`.
    pub fn current(&mut self, number: &str) -> Result<Current, CliError> {
        let akt = self.akt(number)?.clone();
        let ty = self.dir.type_of(&akt).ty.clone();
        Ok(Current {
            state: akt.state.clone(),
            fields: current(&ty, &akt, &mut self.ids)?,
        })
    }

    /// Adds a new akt, to be written when saved.
    pub fn add(&mut self, akt: LocalAkt) {
        self.touched.insert(akt.number.clone());
        self.dir.akter.push(akt);
        self.dir
            .akter
            .sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.number.cmp(&b.number)));
    }

    /// Replaces the akt type `key`'s document, to be written when saved.
    pub fn define(&mut self, t: LocalType) {
        self.types.insert(t.ty.key().to_owned());
        self.dir.types.insert(t.ty.key().to_owned(), t);
    }

    /// Writes every file the change touched, each whole.
    // r[impl cli.local.layout]
    pub fn save(self) -> Result<(), CliError> {
        // Until writes keep nested akter (PLAN-009-T03), never write a file
        // over the other akter it holds.
        for number in &self.touched {
            let akt = self.dir.akt(number).expect("touched akter exist");
            if self
                .dir
                .akter
                .iter()
                .any(|a| a.path == akt.path && a.number != *number)
            {
                return Err(super::unsupported(
                    &format!("Writing {number}"),
                    &format!(
                        "{} holds other akter too, and writes don't keep nested akter yet",
                        akt.path.display()
                    ),
                ));
            }
        }
        for key in &self.types {
            let path = self.dir.root.join("types").join(format!("{key}.json"));
            let text = serde_json::to_string_pretty(&self.dir.types[key].document)
                .expect("serializable")
                + "\n";
            write_whole(&path, &text)?;
        }
        for number in &self.touched {
            let akt = self.dir.akt(number).expect("touched akter exist");
            write_whole(&akt.path, &text_of(&self.dir, akt))?;
        }
        Ok(())
    }

    /// The highest rank in the directory.
    #[must_use]
    pub fn last_rank(&self) -> Option<&str> {
        self.dir.akter.iter().map(|a| a.rank.as_str()).max()
    }

    /// A new akt of `t`, not yet added.
    #[must_use]
    pub fn new_akt(
        &self,
        number: String,
        t: &LocalType,
        state: String,
        rank: String,
        containers: Vec<String>,
        values: Map<String, Json>,
    ) -> LocalAkt {
        LocalAkt {
            path: self.dir.root.join("akter").join(format!("{number}.md")),
            number,
            type_key: t.ty.key().to_owned(),
            state,
            revision: 1,
            rank,
            containers,
            links: Vec::new(),
            values,
            parsed: Parsed {
                front: Map::new(),
                sections: Vec::new(),
            },
            nested: false,
        }
    }
}

/// Writes `text` to `path` through a file beside it, renamed over it, so
/// that a reader never sees half of it.
fn write_whole(path: &Path, text: &str) -> Result<(), CliError> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let partial = path.with_file_name(format!(".{name}.partial"));
    std::fs::write(&partial, text).map_err(|e| io_error(&partial, &e))?;
    std::fs::rename(&partial, path).map_err(|e| io_error(path, &e))
}

/// `akt`'s file: the akt file `cat` prints, with its `rank`.
// r[impl cli.local.layout]
#[must_use]
pub fn text_of(dir: &AktDir, akt: &LocalAkt) -> String {
    let t = dir.type_of(akt);
    let mut front = vec![
        ("number", Json::from(akt.number.clone())),
        ("type", akt.type_key.clone().into()),
        ("revision", akt.revision.into()),
        ("rank", akt.rank.clone().into()),
        ("state", akt.state.clone().into()),
    ];
    if !akt.containers.is_empty() {
        front.push(("in", akt.containers.clone().into()));
    }
    if !akt.links.is_empty() {
        front.push(("links", akt.links.clone().into()));
    }
    let mut single = Vec::new();
    let mut sections = Vec::new();
    for field in &t.ty.document().fields {
        let Some(value) = akt
            .values
            .get(&field.key)
            .filter(|v| !v.is_null() && **v != Json::Bool(false))
        else {
            continue;
        };
        let text = serde_json::to_value(field.kind).expect("serializable") == "text";
        match value {
            Json::String(body) if text && field.multiline => {
                sections.push((field.key.clone(), body.clone()));
            }
            _ => single.push((field.key.clone(), value.clone())),
        }
    }
    file::write_parts(&front, &single, &sections)
}

/// An answer, with the reason to commit with (`cli.local.reason`).
// r[impl cli.local.reason]
#[must_use]
pub fn answer(markdown: String, mut json: Json, reason: Option<&str>) -> Answer {
    let mut markdown = markdown;
    if let Some(reason) = reason {
        json["reason"] = reason.into();
        markdown.push_str(&format!("\n\nCommit with: {reason}"));
    }
    Answer { markdown, json }
}

/// Every akt the directory has, by number.
pub fn numbers(dir: &AktDir) -> impl Iterator<Item = &str> {
    dir.akter.iter().map(|a| a.number.as_str())
}

/// `json!` for an answer's revision.
#[must_use]
pub fn revision(number: &str, revision: i32) -> Json {
    json!({ "number": number, "revision": revision })
}
