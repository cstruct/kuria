//! `kuria docs import <folder>` (`cli.docs-import`): a document kept
//! elsewhere brought in with its history. `metadata.json` names its field
//! values, its draft and its old versions, each a PDF; the draft is
//! converted to Markdown with pandoc. The akt is created, each version
//! released with its PDF, oldest first, and the draft made its body.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::model::Shape;
use crate::output::Answer;
use crate::session::Session;
use crate::versions::Releasing;

/// One old version: its PDF, label and description.
#[derive(Clone, Debug)]
pub struct Version {
    pub label: Option<String>,
    pub description: Option<String>,
    pub file: PathBuf,
}

/// What a folder holds, checked: its field values as `--set`s, its draft
/// in Markdown with the file it came from, and its versions, oldest first.
#[derive(Clone, Debug)]
pub struct Folder {
    pub sets: Vec<String>,
    pub draft: Option<(String, String)>,
    pub versions: Vec<Version>,
}

fn bad(message: impl Into<String>) -> CliError {
    CliError::refused("BAD_FILE", message)
}

/// Reads and checks `folder`, converting its draft (`cli.docs-import`).
// r[impl cli.docs-import]
pub fn read(folder: &Path) -> Result<Folder, CliError> {
    let path = folder.join("metadata.json");
    let text =
        std::fs::read_to_string(&path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
    let Json::Object(metadata) =
        serde_json::from_str(&text).map_err(|e| bad(format!("{}: {e}", path.display())))?
    else {
        return Err(bad(format!("{} isn't a JSON object", path.display())));
    };
    let in_folder = |name: &Json, what: &str| -> Result<PathBuf, CliError> {
        let name = name
            .as_str()
            .ok_or_else(|| bad(format!("{what} names no file")))?;
        let file = folder.join(name);
        if file.is_file() {
            Ok(file)
        } else {
            Err(bad(format!("{what}: there is no {}", file.display())))
        }
    };
    let mut sets = Vec::new();
    let mut draft = None;
    let mut versions = Vec::new();
    for (key, value) in &metadata {
        match key.as_str() {
            "draft" => {
                let file = in_folder(value, "the draft")?;
                draft = Some((file.display().to_string(), markdown_of(&file)?));
            }
            "versions" => {
                let listed = value
                    .as_array()
                    .ok_or_else(|| bad("`versions` is a list of versions, oldest first"))?;
                for (i, v) in listed.iter().enumerate() {
                    let what = format!("version {}", i + 1);
                    let file = in_folder(&v["file"], &what)?;
                    let bytes = std::fs::read(&file)
                        .map_err(|e| bad(format!("{}: {e}", file.display())))?;
                    if !bytes.starts_with(b"%PDF") {
                        return Err(bad(format!("{what}: {} isn't a PDF", file.display())));
                    }
                    let text = |key: &str| v[key].as_str().map(str::to_owned);
                    versions.push(Version {
                        label: text("label"),
                        description: text("description"),
                        file,
                    });
                }
            }
            _ => sets.push(set_of(key, value)?),
        }
    }
    Ok(Folder {
        sets,
        draft,
        versions,
    })
}

/// A field's value from `metadata.json` as `--set` writes it
/// (`cli.values`): text as it is, a list as its items joined by commas.
fn set_of(key: &str, value: &Json) -> Result<String, CliError> {
    let text = match value {
        Json::String(text) => text.clone(),
        Json::Number(n) => n.to_string(),
        Json::Bool(b) => b.to_string(),
        Json::Array(items) => items
            .iter()
            .map(|i| i.as_str().map_or_else(|| i.to_string(), str::to_owned))
            .collect::<Vec<_>>()
            .join(","),
        Json::Null | Json::Object(_) => {
            return Err(bad(format!(
                "`{key}` in metadata.json isn't a value to set"
            )));
        }
    };
    Ok(format!("{key}={text}"))
}

/// The draft at `file` in Markdown: as it is for Markdown, else converted
/// by pandoc to GitHub-flavoured Markdown.
fn markdown_of(file: &Path) -> Result<String, CliError> {
    let extension = file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if matches!(extension.as_deref(), Some("md" | "markdown")) {
        return std::fs::read_to_string(file).map_err(|e| bad(format!("{}: {e}", file.display())));
    }
    let ran = Command::new("pandoc")
        .arg(file)
        .args(["--to", "gfm", "--wrap", "none"])
        .output()
        .map_err(|e| {
            CliError::refused(
                "PANDOC_MISSING",
                format!("converting {} needs pandoc on the PATH ({e}): install it, or give the draft as Markdown", file.display()),
            )
        })?;
    if !ran.status.success() {
        return Err(CliError::refused(
            "PANDOC_FAILED",
            format!(
                "pandoc couldn't convert {}: {}",
                file.display(),
                String::from_utf8_lossy(&ran.stderr).trim()
            ),
        ));
    }
    Ok(String::from_utf8_lossy(&ran.stdout).into_owned())
}

/// How many pictures `markdown` embeds that aren't Kuria's, which an
/// import leaves as they are (PLAN-021-G01).
fn pictures_left(markdown: &str) -> usize {
    markdown
        .match_indices("![")
        .filter(|(at, _)| {
            let rest = &markdown[*at..];
            rest.find("](")
                .is_some_and(|i| !rest[i + 2..].starts_with("kuria:"))
        })
        .count()
}

/// The `--set` that makes `draft` the value of `shape`'s one `markdown`
/// field, written to a file of its own for `@<file>`; refused unless it
/// has exactly one.
fn draft_set(shape: &Shape, draft: &str) -> Result<(String, Scratch), CliError> {
    let mut markdown = shape.fields.iter().filter(|f| f.kind == "MARKDOWN");
    let (Some(field), None) = (markdown.next(), markdown.next()) else {
        return Err(bad(format!(
            "a draft needs `{}` to have exactly one markdown field to hold it",
            shape.key
        )));
    };
    let name = getrandom::u64().expect("system randomness");
    let path = std::env::temp_dir().join(format!("kuria-draft-{name:016x}.md"));
    std::fs::write(&path, draft).map_err(|e| CliError::unavailable(e.to_string()))?;
    Ok((format!("{}=@{}", field.key, path.display()), Scratch(path)))
}

/// A file of the import's own, removed once it is done with.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// What has been made so far, for a failure part way to say.
struct Made {
    number: Option<String>,
    versions: Vec<i64>,
}

impl Made {
    fn said(&self, e: CliError) -> CliError {
        let Some(number) = &self.number else { return e };
        let versions = match self.versions.as_slice() {
            [] => "no versions".to_owned(),
            [one] => format!("version {one}"),
            all => format!(
                "versions {}",
                all.iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        CliError {
            message: format!("{number} was made, with {versions}; then {}", e.message),
            ..e
        }
    }

    fn answer(&self, folder: &Folder) -> Answer {
        let number = self.number.clone().unwrap_or_default();
        let pictures = folder
            .draft
            .as_ref()
            .map_or(0, |(_, markdown)| pictures_left(markdown));
        let mut lines = vec![format!(
            "Imported {number}, with {} version{}{}.",
            self.versions.len(),
            if self.versions.len() == 1 { "" } else { "s" },
            if folder.draft.is_some() {
                " and its draft as its body"
            } else {
                ""
            }
        )];
        if pictures > 0 {
            lines.push(format!(
                "The draft's {pictures} picture{} {} left out: add {} with Image.",
                if pictures == 1 { "" } else { "s" },
                if pictures == 1 { "was" } else { "were" },
                if pictures == 1 { "it" } else { "them" }
            ));
        }
        Answer {
            markdown: lines.join("\n"),
            json: json!({ "number": number, "versions": self.versions, "picturesLeftOut": pictures }),
        }
    }
}

/// What to import, and where.
pub struct Asked<'a> {
    pub folder: &'a Path,
    pub akt_type: &'a str,
    pub inside: &'a [String],
    pub reason: Option<&'a str>,
}

/// `kuria docs import` against a server.
// r[impl cli.docs-import]
pub async fn on_server(session: &Session, asked: &Asked<'_>) -> Result<Answer, CliError> {
    let folder = read(asked.folder)?;
    let shape = crate::reads::shape(crate::reads::Source::Server(session), asked.akt_type).await?;
    let draft = folder
        .draft
        .as_ref()
        .map(|(_, markdown)| draft_set(&shape, markdown))
        .transpose()?;
    let mut made = Made {
        number: None,
        versions: Vec::new(),
    };
    let created = crate::writes::create(
        session,
        &crate::writes::Created {
            akt_type: asked.akt_type,
            inside: asked.inside,
            sets: &folder.sets,
            file: None,
            reason: asked.reason,
        },
    )
    .await?;
    let number = created.json["number"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    made.number = Some(number.clone());
    for version in &folder.versions {
        let released =
            crate::versions::release(session, std::slice::from_ref(&number), &releasing(version))
                .await
                .map_err(|e| made.said(e))?;
        made.versions
            .push(released.json["version"].as_i64().unwrap_or_default());
    }
    if let Some((set, _file)) = &draft {
        crate::writes::set(session, &number, std::slice::from_ref(set), asked.reason)
            .await
            .map_err(|e| made.said(e))?;
    }
    Ok(made.answer(&folder))
}

fn releasing(version: &Version) -> Releasing<'_> {
    Releasing {
        label: version.label.as_deref(),
        description: version.description.as_deref(),
        file: Some(&version.file),
    }
}

/// `kuria docs import` in an akt directory: each step committed, since a
/// version is read from git (`cli.local.versions`).
// r[impl cli.docs-import]
pub fn in_dir(root: &Path, me: Option<String>, asked: &Asked<'_>) -> Result<Answer, CliError> {
    use crate::local::{edits, git, versions, write::Writer};
    match git::uncommitted(root) {
        Some(false) => {}
        Some(true) => {
            return Err(CliError::refused(
                "UNCOMMITTED",
                "the directory has changes git hasn't committed: commit them, then import, which commits each step",
            ));
        }
        None => {
            return Err(CliError::refused(
                "BAD_REQUEST",
                "an import commits each step, so the akt directory must be in a git repository",
            ));
        }
    }
    let folder = read(asked.folder)?;
    let mut w = Writer::open(root, me.clone())?;
    let draft = folder
        .draft
        .as_ref()
        .map(|(_, markdown)| draft_set(&w.shape(asked.akt_type)?, markdown))
        .transpose()?;
    let mut made = Made {
        number: None,
        versions: Vec::new(),
    };
    let created = edits::create(
        &mut w,
        asked.akt_type,
        asked.inside,
        &folder.sets,
        None,
        asked.reason,
    )?;
    w.save()?;
    let number = created.json["number"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    made.number = Some(number.clone());
    git::commit(
        root,
        &format!("Import {number} from {}", asked.folder.display()),
    )
    .map_err(|e| made.said(e))?;
    for version in &folder.versions {
        let times = git::Times::of(root);
        let mut w = Writer::open(root, me.clone()).map_err(|e| made.said(e))?;
        let released = versions::release(
            &mut w,
            &times,
            std::slice::from_ref(&number),
            &releasing(version),
        )
        .map_err(|e| made.said(e))?;
        w.save().map_err(|e| made.said(e))?;
        let n = released.json["version"].as_i64().unwrap_or_default();
        git::commit(root, &format!("Import {number}: version {n}")).map_err(|e| made.said(e))?;
        made.versions.push(n);
    }
    if let Some((set, _file)) = &draft {
        let mut w = Writer::open(root, me).map_err(|e| made.said(e))?;
        edits::set(&mut w, &number, std::slice::from_ref(set), asked.reason)
            .map_err(|e| made.said(e))?;
        w.save().map_err(|e| made.said(e))?;
        git::commit(root, &format!("Import {number}: its draft")).map_err(|e| made.said(e))?;
    }
    Ok(made.answer(&folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cli.docs-import]
    #[test]
    fn counts_the_pictures_an_import_leaves_out() {
        assert_eq!(
            pictures_left("![a](media/image1.png) and ![b](kuria:IMG-1) ![c](x.png)"),
            2
        );
        assert_eq!(pictures_left("no pictures [link](x)"), 0);
    }

    // r[verify cli.docs-import]
    #[test]
    fn writes_metadata_values_as_sets() {
        assert_eq!(set_of("title", &json!("My doc")).unwrap(), "title=My doc");
        assert_eq!(set_of("tags", &json!(["a", "b"])).unwrap(), "tags=a,b");
        assert_eq!(set_of("n", &json!(3)).unwrap(), "n=3");
        assert!(set_of("x", &json!({})).is_err());
    }
}
