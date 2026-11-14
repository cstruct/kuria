//! An akt directory, loaded: its akt types and its akter, each checked
//! against its type as it is read (`cli.local.layout`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kuria_config::{AktType, AktTypeDocument, validate};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::error::CliError;
use crate::file::{self, Parsed};
use crate::profile::io_error;

/// The akt directory format this client reads and writes.
pub const FORMAT: u32 = 1;

/// `kuria.toml`.
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: u32,
}

/// An akt type of the directory.
#[derive(Clone, Debug)]
pub struct LocalType {
    pub ty: AktType,
    /// The document as a server would store it: as it validated.
    pub document: Json,
}

/// An akt of the directory, as its file says.
#[derive(Clone, Debug)]
pub struct LocalAkt {
    pub number: String,
    pub path: PathBuf,
    pub type_key: String,
    pub state: String,
    pub revision: i32,
    pub rank: String,
    /// The containers it is in, by number.
    pub containers: Vec<String>,
    /// Its links as the file says them: "Blocks PLAN-001-T02".
    pub links: Vec<String>,
    /// Its field values in the akt file's form, by key.
    pub values: Map<String, Json>,
    /// The file as read.
    pub parsed: Parsed,
}

/// An akt directory, loaded.
#[derive(Debug)]
pub struct AktDir {
    pub root: PathBuf,
    pub types: BTreeMap<String, LocalType>,
    /// In the directory's order (`rank`).
    pub akter: Vec<LocalAkt>,
}

fn bad(path: &Path, message: impl std::fmt::Display) -> CliError {
    CliError::refused("BAD_FILE", format!("{}: {message}", path.display()))
}

/// Makes an akt directory at `root`: `kuria.toml`, `types/` and `akter/`.
// r[impl cli.local.dir]
pub fn init(root: &Path) -> Result<(), CliError> {
    let manifest = root.join("kuria.toml");
    if manifest.exists() {
        return Err(CliError::refused(
            "ALREADY_AN_AKT_DIRECTORY",
            format!("{} is already an akt directory", root.display()),
        ));
    }
    for dir in [root.join("types"), root.join("akter")] {
        std::fs::create_dir_all(&dir).map_err(|e| io_error(&dir, &e))?;
    }
    let text = toml::to_string(&Manifest { format: FORMAT }).expect("serializable");
    std::fs::write(&manifest, text).map_err(|e| io_error(&manifest, &e))?;
    // What a write leaves only while it runs.
    let ignore = root.join(".gitignore");
    std::fs::write(&ignore, ".kuria.lock\n.*.partial\n").map_err(|e| io_error(&ignore, &e))
}

impl AktDir {
    /// The akt directory at `root`, every type and akt read and checked.
    // r[impl cli.local.dir]
    // r[impl cli.local.layout]
    pub fn open(root: &Path) -> Result<Self, CliError> {
        let not_one = || {
            CliError::refused(
                "NOT_AN_AKT_DIRECTORY",
                format!(
                    "{} isn't an akt directory (no kuria.toml); make one with `kuria init --dir {}`",
                    root.display(),
                    root.display()
                ),
            )
        };
        let manifest = root.join("kuria.toml");
        let text = std::fs::read_to_string(&manifest).map_err(|_| not_one())?;
        let manifest: Manifest =
            toml::from_str(&text).map_err(|e| bad(&root.join("kuria.toml"), e))?;
        if manifest.format != FORMAT {
            return Err(CliError::refused(
                "NOT_AN_AKT_DIRECTORY",
                format!(
                    "{} is an akt directory of format {}; this client reads format {FORMAT}",
                    root.display(),
                    manifest.format
                ),
            ));
        }
        let types = read_types(&root.join("types"))?;
        let mut akter = read_akter(&root.join("akter"), &types)?;
        akter.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.number.cmp(&b.number)));
        Ok(Self {
            root: root.to_owned(),
            types,
            akter,
        })
    }

    #[must_use]
    pub fn akt(&self, number: &str) -> Option<&LocalAkt> {
        self.akter.iter().find(|a| a.number == number)
    }

    #[must_use]
    pub fn type_of(&self, akt: &LocalAkt) -> &LocalType {
        &self.types[&akt.type_key]
    }

    /// The akter in `container`, in the directory's order.
    pub fn members_of<'a>(&'a self, container: &'a str) -> impl Iterator<Item = &'a LocalAkt> + 'a {
        self.akter
            .iter()
            .filter(move |a| a.containers.iter().any(|c| c == container))
    }
}

/// Every `<key>.json` in `dir`, validated as `defineAktType` validates.
fn read_types(dir: &Path) -> Result<BTreeMap<String, LocalType>, CliError> {
    let mut types = BTreeMap::new();
    for path in files(dir, "json")? {
        let t = read_type(&path)?;
        types.insert(t.ty.key().to_owned(), t);
    }
    Ok(types)
}

/// The akt type document at `path`, validated as `defineAktType`
/// validates it.
pub fn read_type(path: &Path) -> Result<LocalType, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    let document: AktTypeDocument = serde_json::from_str(&text).map_err(|e| bad(path, e))?;
    let ty = validate(document).map_err(|problems| {
        bad(
            path,
            problems
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        )
    })?;
    let named = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if named != ty.key() {
        return Err(bad(
            path,
            format!("defines `{}`, so it is `types/{}.json`", ty.key(), ty.key()),
        ));
    }
    let document = serde_json::to_value(ty.document()).expect("serializable");
    Ok(LocalType { ty, document })
}

/// The files of `dir` with `extension`, by name.
pub fn files(dir: &Path, extension: &str) -> Result<Vec<PathBuf>, CliError> {
    let mut found = Vec::new();
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let path = entry.map_err(|e| io_error(dir, &e))?.path();
                if path.extension().and_then(|e| e.to_str()) == Some(extension) && path.is_file() {
                    found.push(path);
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_error(dir, &e)),
    }
    found.sort();
    Ok(found)
}

fn strings(parsed: &Parsed, key: &str, path: &Path) -> Result<Vec<String>, CliError> {
    match parsed.front.get(key) {
        None | Some(Json::Null) => Ok(Vec::new()),
        Some(Json::Array(items)) => items
            .iter()
            .map(|i| {
                i.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| bad(path, format!("`{key}` lists text")))
            })
            .collect(),
        Some(_) => Err(bad(path, format!("`{key}` is a list"))),
    }
}

/// Every `<number>.md` in `dir`, as akt files of `types`.
fn read_akter(dir: &Path, types: &BTreeMap<String, LocalType>) -> Result<Vec<LocalAkt>, CliError> {
    files(dir, "md")?
        .iter()
        .map(|path| read_akt(path, types))
        .collect()
}

/// The akt file at `path`, read as an akt of one of `types`.
pub fn read_akt(path: &Path, types: &BTreeMap<String, LocalType>) -> Result<LocalAkt, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    let parsed = file::parse(&text).map_err(|e| bad(path, e.message))?;
    let named = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned();
    if parsed.text("number").as_deref() != Some(named.as_str()) {
        return Err(bad(
            path,
            format!("its `number` isn't `{named}`, its file's name"),
        ));
    }
    let type_key = parsed
        .text("type")
        .ok_or_else(|| bad(path, "names no `type`"))?;
    let ty = types
        .get(&type_key)
        .ok_or_else(|| bad(path, format!("is a `{type_key}`, which `types/` lacks")))?;
    let state = parsed
        .text("state")
        .ok_or_else(|| bad(path, "names no `state`"))?;
    if ty.ty.state(&state).is_none() {
        return Err(bad(path, format!("`{type_key}` has no state `{state}`")));
    }
    let revision = parsed
        .front
        .get("revision")
        .and_then(Json::as_i64)
        .and_then(|r| i32::try_from(r).ok())
        .ok_or_else(|| bad(path, "names no `revision`"))?;
    let rank = parsed
        .text("rank")
        .ok_or_else(|| bad(path, "has no `rank`"))?;
    let values = parsed.values().map_err(|e| bad(path, e.message))?;
    if let Some(key) = values.keys().find(|k| ty.ty.field(k).is_none()) {
        return Err(bad(path, format!("`{type_key}` has no field `{key}`")));
    }
    Ok(LocalAkt {
        containers: strings(&parsed, "in", path)?,
        links: strings(&parsed, "links", path)?,
        number: named,
        path: path.to_owned(),
        type_key,
        state,
        revision,
        rank,
        values,
        parsed,
    })
}
