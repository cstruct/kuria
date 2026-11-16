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
use super::layout;
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
    /// Files to copy into the directory: each source, and its path there.
    copies: Vec<(PathBuf, String)>,
    /// What git says of the directory (`cli.local.history`).
    pub times: super::git::Times,
    /// Files akter were taken out of, by archiving.
    emptied: BTreeSet<PathBuf>,
    /// Whether `akter/archived.md` changed.
    archive_changed: bool,
    /// Archived akter's own files to write, or to remove (`None`).
    kept: std::collections::BTreeMap<PathBuf, Option<String>>,
    _lock: Lock,
}

impl Writer {
    /// The akt directory at `root`, locked for one change.
    pub fn open(root: &Path, me: Option<String>) -> Result<Self, CliError> {
        let lock = Lock::take(root)?;
        let mut dir = AktDir::open(root)?;
        let times = super::git::Times::of(root);
        dir.load_archived(&times);
        Ok(Self {
            dir,
            ids: Ids::new(me),
            now: Utc::now(),
            touched: BTreeSet::new(),
            types: BTreeSet::new(),
            copies: Vec::new(),
            times,
            emptied: BTreeSet::new(),
            archive_changed: false,
            kept: std::collections::BTreeMap::new(),
            _lock: lock,
        })
    }

    /// `akter/archived.md`, to be written when saved.
    pub fn archive_mut(&mut self) -> &mut super::archive::Archive {
        self.archive_changed = true;
        &mut self.dir.archive
    }

    /// Writes `text` to `path`, an archived akt's own file, when saved, or
    /// removes it for `None`.
    pub fn keep_file(&mut self, path: PathBuf, text: Option<String>) {
        self.kept.insert(path, text);
    }

    /// Takes the live akt `number` out of the directory, its file to be
    /// written without it when saved.
    pub fn remove(&mut self, number: &str) -> LocalAkt {
        let i = self
            .dir
            .akter
            .iter()
            .position(|a| a.number == number)
            .expect("found before it is removed");
        let akt = self.dir.akter.remove(i);
        self.emptied.insert(akt.path.clone());
        self.touched.remove(number);
        akt
    }

    /// The value `raw` given for the attachment field `key` of `ty`, for
    /// the akt `number`, as the path of a file of the directory that fits
    /// the field: a path given as one, or `@<file>`, a file copied into
    /// `attachments/<number>/` when the change is saved, under a name of
    /// its own if another file has it. An attachment's identifier names an
    /// upload, which local mode doesn't do.
    // r[impl cli.local.attachments]
    // r[impl cli.local.unsupported]
    pub fn attachment(
        &mut self,
        ty: &kuria_config::AktType,
        key: &str,
        number: &str,
        raw: &Json,
    ) -> Result<Json, CliError> {
        let field = ty.field(key).expect("a field of the type");
        let refused = |reason: String| CliError {
            field: Some(key.to_owned()),
            ..CliError::from_server(
                "REFUSED",
                kuria_engine::Refusal::InvalidValue {
                    field: key.to_owned(),
                    reason,
                }
                .to_string(),
                None,
            )
        };
        let given = match raw {
            Json::Null => return Ok(Json::Null),
            Json::String(text) => text.trim(),
            Json::Object(o) if o.contains_key("id") => "",
            _ => {
                return Err(refused(format!(
                    "`{raw}` isn't a file's path under attachments/"
                )));
            }
        };
        if given.is_empty() || uuid::Uuid::parse_str(given).is_ok() {
            return Err(CliError {
                field: Some(key.to_owned()),
                ..super::unsupported(
                    "An uploaded attachment",
                    "an akt directory holds its attachments as files: name one under attachments/<number>/, or copy one in with `@<file>`",
                )
            });
        }
        let Some(source) = given.strip_prefix('@') else {
            return match super::attachments::problem(&self.dir.root, field, given) {
                Some(reason) => Err(refused(reason)),
                None => Ok(given.into()),
            };
        };
        let source = PathBuf::from(source);
        let bytes = std::fs::read(&source).map_err(|e| io_error(&source, &e))?;
        let name = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let name = super::attachments::file_name(name, super::attachments::media_type(name));
        if let Some(reason) = super::attachments::unfit(field, &name, &bytes) {
            return Err(refused(reason));
        }
        for n in 1.. {
            let path = format!(
                "{}/{number}/{}",
                super::attachments::DIRECTORY,
                super::attachments::numbered(&name, n)
            );
            let staged = self.copies.iter().find(|(_, p)| *p == path);
            let same = match staged {
                Some((from, _)) => std::fs::read(from).ok().as_ref() == Some(&bytes),
                None => match std::fs::read(self.dir.root.join(&path)) {
                    Ok(there) => there == bytes,
                    Err(_) => {
                        self.copies.push((source.clone(), path.clone()));
                        true
                    }
                },
            };
            if same {
                return Ok(path.into());
            }
        }
        unreachable!("a name is found")
    }

    /// The live akt `number`; refused as `ARCHIVED` if it is archived
    /// (`archive.readonly`).
    pub fn akt(&self, number: &str) -> Result<&LocalAkt, CliError> {
        self.dir.akt(number).ok_or_else(|| {
            if self.dir.is_archived(number) {
                super::archive::archived(number)
            } else {
                CliError::from_server("NOT_FOUND", format!("No akt `{number}`"), None)
            }
        })
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

    /// Writes every file the change touched, each whole: the akter it
    /// touched, with the other akter of their files, moved to where they
    /// belong (`cli.local.nesting`), and a file whose akter all left it,
    /// such as one renamed for a new title, removed.
    // r[impl cli.local.layout]
    // r[impl cli.local.names]
    pub fn save(mut self) -> Result<(), CliError> {
        for key in &self.types {
            let path = self.dir.root.join("types").join(format!("{key}.json"));
            let text = serde_json::to_string_pretty(&self.dir.types[key].document)
                .expect("serializable")
                + "\n";
            write_whole(&path, &text)?;
        }
        // Written before the akter that name them.
        for (source, path) in &self.copies {
            let to = self.dir.root.join(path);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|e| io_error(parent, &e))?;
            }
            let bytes = std::fs::read(source).map_err(|e| io_error(source, &e))?;
            write_whole(&to, &bytes)?;
        }
        let mut from: BTreeSet<PathBuf> = self
            .touched
            .iter()
            .filter_map(|n| self.dir.akt(n))
            .map(|a| a.path.clone())
            .collect();
        from.extend(self.emptied.iter().cloned());
        let to = layout::relocate(&mut self.dir, &from);
        // Written first, so that an akt is never in no file.
        for path in &to {
            write_whole(path, layout::file_text(&self.dir, path))?;
        }
        // r[impl cli.local.archive]
        for (path, text) in &self.kept {
            match text {
                Some(text) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| io_error(parent, &e))?;
                    }
                    write_whole(path, text)?;
                }
                None => match std::fs::remove_file(path) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                        return Err(io_error(path, &e));
                    }
                    _ => {}
                },
            }
        }
        if self.archive_changed {
            let path = super::archive::Archive::path(&self.dir.root);
            write_whole(&path, self.dir.archive.text())?;
        }
        for path in from.difference(&to) {
            if self.dir.akter.iter().all(|a| a.path != *path) {
                match std::fs::remove_file(path) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                        return Err(io_error(path, &e));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Refuses sections that `changes` set on `number`, `None` for an akt
    /// being created, that place an akt that isn't one of its members, one
    /// twice, or one of a type the field doesn't place (`akt.field.sections`).
    // r[impl cli.local.rules]
    pub fn check_sections(
        &self,
        ty: &kuria_config::AktType,
        number: Option<&str>,
        changes: &std::collections::BTreeMap<String, Option<kuria_engine::Value>>,
    ) -> Result<(), CliError> {
        for field in ty
            .document()
            .fields
            .iter()
            .filter(|f| f.kind == kuria_config::FieldKind::Sections)
        {
            let Some(Some(kuria_engine::Value::Sections(items))) = changes.get(&field.key) else {
                continue;
            };
            let members: Vec<(&str, &str)> = number
                .map(|n| {
                    self.dir
                        .members_of(n)
                        .map(|m| (m.number.as_str(), m.type_key.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            kuria_engine::sections::check(items, &members, &field.of).map_err(|reason| {
                CliError::from_server(
                    "REFUSED",
                    kuria_engine::Refusal::InvalidValue {
                        field: field.key.clone(),
                        reason,
                    }
                    .to_string(),
                    None,
                )
            })?;
        }
        Ok(())
    }

    /// Takes `member` out of `container`'s sections, as when it leaves the
    /// container (`akt.field.sections`).
    pub fn leave_sections(&mut self, container: &str, member: &str) -> Result<(), CliError> {
        let akt = self.akt(container)?;
        let Some(field) = self
            .dir
            .type_of(akt)
            .ty
            .document()
            .fields
            .iter()
            .find(|f| f.kind == kuria_config::FieldKind::Sections)
        else {
            return Ok(());
        };
        let Some(raw) = akt.values.get(&field.key) else {
            return Ok(());
        };
        let Ok(kuria_engine::Value::Sections(items)) = kuria_engine::Value::from_json(field, raw)
        else {
            return Err(CliError::refused(
                "BAD_FILE",
                format!("{}: `{}` isn't sections", akt.path.display(), field.key),
            ));
        };
        if !kuria_engine::sections::numbers(&items).contains(&member) {
            return Ok(());
        }
        let kept = kuria_engine::Value::Sections(kuria_engine::sections::without(&items, member))
            .to_json();
        let key = field.key.clone();
        self.akt_mut(container).values.insert(key, kept);
        Ok(())
    }

    /// Marks every akt as touched, so that saving moves each into the file
    /// it belongs in and names every file, changing no akt.
    pub fn touch_all(&mut self) {
        self.touched
            .extend(self.dir.akter.iter().map(|a| a.number.clone()));
    }

    /// The highest rank in the directory, archived akter's too, so that an
    /// archived akt's place is never taken.
    #[must_use]
    pub fn last_rank(&self) -> Option<&str> {
        self.dir
            .akter
            .iter()
            .chain(&self.dir.archived)
            .map(|a| a.rank.as_str())
            .max()
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
            // Where it belongs is found when it is saved.
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
            versions: Vec::new(),
        }
    }
}

/// Writes `text` to `path` through a file beside it, renamed over it, so
/// that a reader never sees half of it.
fn write_whole(path: &Path, text: impl AsRef<[u8]>) -> Result<(), CliError> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let partial = path.with_file_name(format!(".{name}.partial"));
    std::fs::write(&partial, text).map_err(|e| io_error(&partial, &e))?;
    std::fs::rename(&partial, path).map_err(|e| io_error(path, &e))
}

/// `akt` as its file holds it: the akt file `cat` prints, with its
/// `rank`, and without its `number` unless `numbered`, as a nested akt,
/// whose line gives it.
// r[impl cli.local.layout]
#[must_use]
pub fn text_of(dir: &AktDir, akt: &LocalAkt, numbered: bool) -> String {
    let t = dir.type_of(akt);
    let mut front = Vec::new();
    if numbered {
        front.push(("number", Json::from(akt.number.clone())));
    }
    front.extend([
        ("type", Json::from(akt.type_key.clone())),
        ("revision", akt.revision.into()),
        ("rank", akt.rank.clone().into()),
        ("state", akt.state.clone().into()),
    ]);
    // r[impl cli.local.versions]
    if !akt.versions.is_empty() {
        front.push(("versions", akt.versions.clone().into()));
    }
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
        let text = field.kind == kuria_config::FieldKind::Text;
        match value {
            Json::String(body)
                if (text && field.multiline) || field.kind == kuria_config::FieldKind::Markdown =>
            {
                sections.push((field.key.clone(), body.clone()));
            }
            _ if field.kind == kuria_config::FieldKind::Sections => {
                sections.push((field.key.clone(), crate::values::sections_yaml(value)));
            }
            Json::Object(o) if o.is_empty() && field.kind == kuria_config::FieldKind::Verdict => {}
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

/// Every akt the directory has, by number, archived ones too, so that no
/// number is used again (`cli.local.archive`).
pub fn numbers(dir: &AktDir) -> impl Iterator<Item = &str> {
    dir.akter
        .iter()
        .map(|a| a.number.as_str())
        .chain(dir.archive.archived.iter().map(|e| e.number.as_str()))
}

/// `json!` for an answer's revision.
#[must_use]
pub fn revision(number: &str, revision: i32) -> Json {
    json!({ "number": number, "revision": revision })
}
