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
    /// Whether it is a nested akt, after another in its file
    /// (`cli.local.nesting`).
    pub nested: bool,
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

fn strings(parsed: &Parsed, key: &str) -> Result<Vec<String>, String> {
    match parsed.front.get(key) {
        None | Some(Json::Null) => Ok(Vec::new()),
        Some(Json::Array(items)) => items
            .iter()
            .map(|i| {
                i.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("`{key}` lists text"))
            })
            .collect(),
        Some(_) => Err(format!("`{key}` is a list")),
    }
}

/// Every akt in the files of `dir`, as akter of `types`.
fn read_akter(dir: &Path, types: &BTreeMap<String, LocalType>) -> Result<Vec<LocalAkt>, CliError> {
    let mut akter = Vec::new();
    for path in files(dir, "md")? {
        for akt in read_file(&path, types) {
            akter.push(akt?);
        }
    }
    Ok(akter)
}

/// The akt a nested akt's line names: `<!-- akt: <NUMBER> -->`.
fn nested_marker(line: &str) -> Option<&str> {
    line.trim()
        .strip_prefix("<!-- akt:")
        .and_then(|l| l.strip_suffix("-->"))
        .map(str::trim)
        .filter(|n| !n.is_empty())
}

/// `text` split into its akter: the first, then each nested akt with the
/// number its line names. A line in a fenced code block is text.
// r[impl cli.local.layout]
fn split(text: &str) -> Vec<(Option<&str>, String)> {
    let mut parts: Vec<(Option<&str>, Vec<&str>)> = vec![(None, Vec::new())];
    let mut fence: Option<&str> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let opens = ["```", "~~~"].into_iter().find(|f| trimmed.starts_with(f));
        match (fence, opens) {
            (None, Some(f)) => fence = Some(f),
            (Some(open), Some(_))
                if trimmed
                    .trim_end()
                    .chars()
                    .all(|c| c == open.chars().next().unwrap_or('`')) =>
            {
                fence = None;
            }
            _ => {}
        }
        match nested_marker(line).filter(|_| fence.is_none() && opens.is_none()) {
            Some(number) => parts.push((Some(number), Vec::new())),
            None => parts.last_mut().expect("never empty").1.push(line),
        }
    }
    parts
        .into_iter()
        .map(|(number, lines)| {
            let mut text = lines.join("\n").trim_matches('\n').to_owned();
            text.push('\n');
            (number, text)
        })
        .collect()
}

/// Every akt in the file at `path`, each read as an akt of one of
/// `types`: the first, and its nested akter (`cli.local.layout`).
#[must_use]
pub fn read_file(
    path: &Path,
    types: &BTreeMap<String, LocalType>,
) -> Vec<Result<LocalAkt, CliError>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => return vec![Err(io_error(path, &e))],
    };
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    split(&text)
        .into_iter()
        .map(|(number, text)| read_akt(path, number, &text, types))
        .collect()
}

/// One akt of the file at `path`, from its `text`: the first, or the
/// nested akt `nested` names.
fn read_akt(
    path: &Path,
    nested: Option<&str>,
    text: &str,
    types: &BTreeMap<String, LocalType>,
) -> Result<LocalAkt, CliError> {
    // A problem in a nested akt names it too.
    let bad = |message: String| match nested {
        Some(number) => bad(path, format!("{number}: {message}")),
        None => bad(path, message),
    };
    let parsed = file::parse(text).map_err(|e| bad(e.message))?;
    let number = match (nested, parsed.text("number")) {
        (None, Some(number)) => number,
        (None, None) => return Err(bad("names no `number`".to_owned())),
        (Some(number), None) => number.to_owned(),
        (Some(_), Some(_)) => {
            return Err(bad(
                "has a `number`, which its `<!-- akt: -->` line gives".to_owned()
            ));
        }
    };
    let type_key = parsed
        .text("type")
        .ok_or_else(|| bad("names no `type`".to_owned()))?;
    let ty = types
        .get(&type_key)
        .ok_or_else(|| bad(format!("is a `{type_key}`, which `types/` lacks")))?;
    let state = parsed
        .text("state")
        .ok_or_else(|| bad("names no `state`".to_owned()))?;
    if ty.ty.state(&state).is_none() {
        return Err(bad(format!("`{type_key}` has no state `{state}`")));
    }
    let revision = parsed
        .front
        .get("revision")
        .and_then(Json::as_i64)
        .and_then(|r| i32::try_from(r).ok())
        .ok_or_else(|| bad("names no `revision`".to_owned()))?;
    let rank = parsed
        .text("rank")
        .ok_or_else(|| bad("has no `rank`".to_owned()))?;
    let values = parsed.values().map_err(|e| bad(e.message))?;
    if let Some(key) = values.keys().find(|k| ty.ty.field(k).is_none()) {
        return Err(bad(format!("`{type_key}` has no field `{key}`")));
    }
    let list = |key: &str| strings(&parsed, key).map_err(bad);
    Ok(LocalAkt {
        containers: list("in")?,
        links: list("links")?,
        number,
        path: path.to_owned(),
        type_key,
        state,
        revision,
        rank,
        values,
        nested: nested.is_some(),
        parsed,
    })
}
