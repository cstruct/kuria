//! `kuria fmt` (`clone.normal`): a clone's files put in normal form, each
//! where the container's `sections` puts it, with the pictures of the
//! images they embed and no others.
//!
//! A clone is a directory, which needn't be a git repository: its files are
//! those under it, but for `.git/` and `.kuria/`. Its container is the one
//! `.kuria/clone.json` names, if the clone has one, else the akt of the
//! file at its root that no other file there is a member of.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use super::{CloneOf, KURIA_DIR, layout::number_of_file, pictures};
use crate::error::CliError;
use crate::output::Answer;
use crate::profile::io_error;
use crate::session::Session;

/// What a clone keeps of itself in `.kuria/`: the container it is of.
pub const CLONE_FILE: &str = "clone.json";

/// The clone `dir` is in: the nearest directory holding `.kuria/`, else
/// `dir` itself.
#[must_use]
pub fn root_of(dir: &Path) -> PathBuf {
    dir.ancestors()
        .find(|d| d.join(KURIA_DIR).is_dir())
        .unwrap_or(dir)
        .to_path_buf()
}

/// The paths of the files under `root`, from it, with `/` between parts:
/// every file but those in `.git/` and `.kuria/`.
pub fn files_in(root: &Path) -> Result<Vec<String>, CliError> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, at)) = pending.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| io_error(&dir, &e))?;
        for entry in entries {
            let entry = entry.map_err(|e| io_error(&dir, &e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let kind = entry.file_type().map_err(|e| io_error(&entry.path(), &e))?;
            if kind.is_dir() {
                if name != ".git" && name != KURIA_DIR {
                    pending.push((entry.path(), format!("{at}{name}/")));
                }
            } else if kind.is_file() {
                found.push(format!("{at}{name}"));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Whether `path` is a Markdown file.
fn is_markdown(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// The container `.kuria/clone.json` at `root` names, if there is one.
fn recorded_number(root: &Path) -> Result<Option<String>, CliError> {
    let path = root.join(KURIA_DIR).join(CLONE_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let read: Json = serde_json::from_str(&text)
        .map_err(|e| CliError::refused("BAD_FILE", format!("{}: {e}", path.display())))?;
    Ok(read["number"].as_str().map(str::to_owned))
}

/// The numbers of the akt files at the root of `tree`, the container's
/// among them.
fn at_root(tree: &BTreeMap<String, String>) -> Vec<String> {
    tree.iter()
        .filter(|(path, _)| !path.contains('/'))
        .filter_map(|(_, text)| number_of_file(text))
        .collect()
}

/// The container of the clone at `root`, of files `tree`: the one
/// `.kuria/` names, else the root's one akt file, else the one of the
/// root's akt files that, as Kuria says, isn't a member of another.
async fn container_of(
    session: &Session,
    root: &Path,
    tree: &BTreeMap<String, String>,
) -> Result<String, CliError> {
    if let Some(number) = recorded_number(root)? {
        return Ok(number);
    }
    let candidates = at_root(tree);
    if let [only] = candidates.as_slice() {
        return Ok(only.clone());
    }
    let mut outer = Vec::new();
    for number in &candidates {
        let data = session
            .graphql(
                "query($n: String!) { akt(number: $n) { containers { number } } }",
                json!({ "n": number }),
            )
            .await?;
        let inside = data["akt"]["containers"].as_array().is_some_and(|c| {
            c.iter().any(|c| {
                c["number"]
                    .as_str()
                    .is_some_and(|n| candidates.iter().any(|m| m == n))
            })
        });
        if !inside {
            outer.push(number.clone());
        }
    }
    match outer.as_slice() {
        [only] => Ok(only.clone()),
        _ => Err(CliError::refused(
            "BAD_REQUEST",
            format!(
                "`kuria fmt` runs in a clone, whose root holds its container's file; {} holds {}",
                root.display(),
                if candidates.is_empty() {
                    "no akt file".to_owned()
                } else {
                    format!("several: {}", candidates.join(", "))
                }
            ),
        )),
    }
}

/// `kuria fmt` in the clone `dir` is in: its files in normal form, where
/// the container's `sections` puts them, and the pictures of the images
/// they embed (`clone.normal`, `clone.pictures`).
// r[impl clone.normal]
pub async fn fmt(session: &Session, dir: &Path) -> Result<Answer, CliError> {
    let root = root_of(dir);
    let listed = files_in(&root)?;
    let mut tree = BTreeMap::new();
    for path in listed.iter().filter(|p| is_markdown(p)) {
        let full = root.join(path);
        let text = std::fs::read_to_string(&full).map_err(|e| io_error(&full, &e))?;
        tree.insert(path.clone(), text);
    }
    let clone = CloneOf {
        session,
        number: container_of(session, &root, &tree).await?,
        root: root.clone(),
    };
    let (normal, pictures) = clone.normal_tree(&tree).await?;
    let mut changed = Vec::new();
    for (path, text) in &tree {
        if normal.get(path) != Some(text) {
            let full = root.join(path);
            std::fs::remove_file(&full).map_err(|e| io_error(&full, &e))?;
            changed.push(path.clone());
        }
    }
    for (path, text) in &normal {
        if tree.get(path) != Some(text) {
            write(&root.join(path), text.as_bytes())?;
            if !changed.contains(path) {
                changed.push(path.clone());
            }
        }
    }
    // The pictures of the images the files embed, and no others.
    // r[impl clone.pictures]
    for path in listed.iter().filter(|p| pictures::is_picture(p)) {
        if !pictures.contains_key(path) {
            let full = root.join(path);
            std::fs::remove_file(&full).map_err(|e| io_error(&full, &e))?;
            changed.push(path.clone());
        }
    }
    for (path, bytes) in &pictures {
        let full = root.join(path);
        if std::fs::read(&full).ok().as_ref() != Some(bytes) {
            write(&full, bytes)?;
            changed.push(path.clone());
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
    Ok(Answer {
        markdown: if changed.is_empty() {
            "Every file is in normal form.".to_owned()
        } else {
            format!("Put in normal form: {}.", changed.join(", "))
        },
        json: json!({ "changed": changed }),
    })
}

/// Writes `bytes` to `path`, making its directory if need be.
fn write(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
    }
    std::fs::write(path, bytes).map_err(|e| io_error(path, &e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn akt_file(number: &str) -> String {
        format!("---\nnumber: {number}\ntype: document\nstate: draft\n---\n")
    }

    // r[verify clone.normal]
    #[test]
    fn finds_a_clone_s_files_and_container_without_git() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (path, text) in [
            ("D-1-index.md", akt_file("D-1")),
            ("guides/D-2-guide.md", akt_file("D-2")),
            ("attachments/IMG-1.png", "png".to_owned()),
            (".git/HEAD", "ref: refs/heads/main\n".to_owned()),
            (".kuria/base/D-1-index.md", akt_file("D-1")),
        ] {
            write(&root.join(path), text.as_bytes()).unwrap();
        }
        assert_eq!(
            files_in(root).unwrap(),
            [
                "D-1-index.md",
                "attachments/IMG-1.png",
                "guides/D-2-guide.md"
            ]
        );
        assert_eq!(root_of(&root.join("guides")), root);
        let tree = BTreeMap::from([
            ("D-1-index.md".to_owned(), akt_file("D-1")),
            ("guides/D-2-guide.md".to_owned(), akt_file("D-2")),
        ]);
        assert_eq!(at_root(&tree), ["D-1"]);

        // Without a `clone.json`, the root's file; with one, what it names.
        assert_eq!(recorded_number(root).unwrap(), None);
        write(
            &root.join(".kuria").join(CLONE_FILE),
            br#"{ "number": "D-9" }"#,
        )
        .unwrap();
        assert_eq!(recorded_number(root).unwrap().as_deref(), Some("D-9"));
    }
}
