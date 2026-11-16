//! A clone's working files against its base, offline: `kuria docs status`,
//! `kuria docs diff` and `kuria docs add` (`clone.status`, `clone.add`).

use std::io::{BufRead, Write};
use std::path::Path;

use serde_json::json;
use similar::{Algorithm, DiffOp, capture_diff_slices, group_diff_ops};

use super::base::{Local, is_under};
use super::fmt::files_in;
use super::pictures;
use crate::error::CliError;
use crate::output::Answer;

/// How a file of a clone stands against its base.
#[derive(Debug, Default)]
pub struct Status {
    /// Staged, differing from the base.
    pub staged: Vec<String>,
    /// Differing from what is staged, or from the base if nothing is.
    pub unstaged: Vec<String>,
    /// In the base, gone from the working files.
    pub removed: Vec<String>,
    /// Holding conflict markers.
    pub conflicted: Vec<String>,
    /// Akt files the base doesn't have.
    pub untracked: Vec<String>,
}

/// Whether `bytes` hold a conflict a pull marked.
fn conflicted(bytes: &[u8]) -> bool {
    kuria_engine::merge::has_conflict(&String::from_utf8_lossy(bytes))
}

/// Every path the base has: its akter's files, then its pictures.
fn base_paths(local: &Local) -> Result<Vec<String>, CliError> {
    let mut paths: Vec<String> = local.bases.akter.values().map(|a| a.path.clone()).collect();
    paths.extend(local.base_pictures()?);
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// How the clone at `local` stands (`clone.status`).
// r[impl clone.status]
pub fn status_of(local: &Local) -> Result<Status, CliError> {
    let mut status = Status::default();
    let known = base_paths(local)?;
    for path in &known {
        let base = local.base(path);
        let staged = local.staged(path);
        let index = staged.clone().map(String::into_bytes).or(base.clone());
        if staged.is_some() && staged.map(String::into_bytes) != base {
            status.staged.push(path.clone());
        }
        match local.working(path) {
            None => status.removed.push(path.clone()),
            Some(working) => {
                if conflicted(&working) {
                    status.conflicted.push(path.clone());
                }
                if Some(&working) != index.as_ref() {
                    status.unstaged.push(path.clone());
                }
            }
        }
    }
    for path in files_in(&local.root)? {
        let is_akt_file = Path::new(&path)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if is_akt_file && !known.contains(&path) {
            status.untracked.push(path);
        }
    }
    Ok(status)
}

fn listed(title: &str, paths: &[String], how: &str) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let mut text = format!("\n{title}\n\n");
    for path in paths {
        text.push_str(&format!("- {how}{path}\n"));
    }
    text
}

/// `kuria docs status`: each file staged, each changed but not staged, each
/// removed, each holding conflict markers (`clone.status`).
// r[impl clone.status]
pub fn status(here: &Path) -> Result<Answer, CliError> {
    let local = Local::open(here)?;
    let s = status_of(&local)?;
    let unstaged: Vec<String> = s
        .unstaged
        .iter()
        .filter(|p| !s.conflicted.contains(p))
        .cloned()
        .collect();
    let mut markdown = format!(
        "A clone of {} at {}.\n",
        local.recorded.number, local.recorded.url
    );
    markdown.push_str(&listed(
        "Staged, to push with `kuria docs push -m <message>`:",
        &s.staged,
        "changed: ",
    ));
    markdown.push_str(&listed(
        "Holding conflict markers; resolve them, then `kuria docs add`:",
        &s.conflicted,
        "conflicted: ",
    ));
    markdown.push_str(&listed(
        "Not staged; stage with `kuria docs add`:",
        &unstaged,
        "changed: ",
    ));
    markdown.push_str(&listed(
        "Removed; akter are removed in Kuria, so `kuria docs pull` brings them back:",
        &s.removed,
        "removed: ",
    ));
    markdown.push_str(&listed(
        "Not in Kuria; akter are created in Kuria, then pulled:",
        &s.untracked,
        "",
    ));
    if s.staged.is_empty()
        && s.unstaged.is_empty()
        && s.removed.is_empty()
        && s.untracked.is_empty()
    {
        markdown.push_str("\nNothing changed since the base.\n");
    }
    Ok(Answer {
        markdown,
        json: json!({
            "number": local.recorded.number,
            "staged": s.staged,
            "unstaged": s.unstaged,
            "removed": s.removed,
            "conflicted": s.conflicted,
            "untracked": s.untracked,
        }),
    })
}

/// The base's paths `given` names from `here`, every one if none is given.
fn chosen(local: &Local, here: &Path, given: &[String]) -> Result<Vec<String>, CliError> {
    let known = base_paths(local)?;
    if given.is_empty() {
        return Ok(known);
    }
    let mut paths = Vec::new();
    for g in given {
        let path = local.path_from(here, g)?;
        let under: Vec<&String> = known.iter().filter(|k| is_under(k, &path)).collect();
        if under.is_empty() {
            return Err(if local.root.join(&path).is_file() {
                CliError::refused(
                    "BAD_REQUEST",
                    format!(
                        "{path} isn't one of Kuria's files: akter are created in Kuria (`kuria new <type> --in {}`), then pulled",
                        local.recorded.number
                    ),
                )
            } else {
                CliError::refused("BAD_REQUEST", format!("The clone has no file {path}"))
            });
        }
        for k in under {
            if !paths.contains(k) {
                paths.push(k.clone());
            }
        }
    }
    Ok(paths)
}

/// Lines of `text`, each with its line break.
fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// A unified diff from `old` to `new`, for the file at `path`, empty if
/// they are alike.
#[must_use]
pub fn unified(path: &str, old: Option<&str>, new: Option<&str>) -> String {
    if old == new {
        return String::new();
    }
    let header = (
        old.map_or_else(|| "/dev/null".to_owned(), |_| format!("a/{path}")),
        new.map_or_else(|| "/dev/null".to_owned(), |_| format!("b/{path}")),
    );
    similar::TextDiff::from_lines(old.unwrap_or_default(), new.unwrap_or_default())
        .unified_diff()
        .context_radius(3)
        .header(&header.0, &header.1)
        .to_string()
}

/// `kuria docs diff [--staged] [<path>…]`: the working files against what is
/// staged of them, else their base; with `staged`, what is staged against
/// the base (`clone.status`).
// r[impl clone.status]
pub fn diff(here: &Path, staged: bool, given: &[String]) -> Result<Answer, CliError> {
    let local = Local::open(here)?;
    let mut text = String::new();
    let mut files = Vec::new();
    for path in chosen(&local, here, given)? {
        let (old, new) = if staged {
            match local.staged(&path) {
                Some(s) => (local.base_text(&path), Some(s)),
                None => continue,
            }
        } else {
            let working = local
                .working(&path)
                .map(|b| String::from_utf8_lossy(&b).into_owned());
            (local.index(&path), working)
        };
        if old == new {
            continue;
        }
        files.push(path.clone());
        if pictures::is_picture(&path) {
            text.push_str(&format!("The picture {path} differs.\n"));
        } else {
            text.push_str(&unified(&path, old.as_deref(), new.as_deref()));
        }
    }
    Ok(Answer {
        markdown: text,
        json: json!({ "staged": staged, "files": files }),
    })
}

/// Refuses to stage the file at `path` as `working` is: a picture, a
/// removed file or one holding conflict markers (`clone.add`).
// r[impl clone.add]
fn stageable(path: &str, working: Option<&Vec<u8>>) -> Result<(), CliError> {
    if pictures::is_picture(path) {
        return Err(CliError::refused(
            "BAD_REQUEST",
            format!("{path} is a picture: pictures are uploaded in Kuria, never pushed"),
        ));
    }
    match working {
        None => Err(CliError::refused(
            "BAD_REQUEST",
            format!(
                "{path} is removed: akter are removed in Kuria; `kuria docs pull` brings it back until then"
            ),
        )),
        Some(bytes) if conflicted(bytes) => Err(CliError::refused(
            "CONFLICTED",
            format!("{path} holds conflict markers: resolve them, then `kuria docs add` it"),
        )),
        Some(_) => Ok(()),
    }
}

/// The base's akt files `given` names that changed: each with the text it
/// would be staged against and its working text, checked to be stageable.
/// A path named by a directory skips its pictures and removed files.
fn to_stage(
    local: &Local,
    here: &Path,
    given: &[String],
) -> Result<Vec<(String, String, String)>, CliError> {
    let mut found = Vec::new();
    let named: Vec<String> = given
        .iter()
        .map(|g| local.path_from(here, g))
        .collect::<Result<_, _>>()?;
    for path in chosen(local, here, given)? {
        let working = local.working(&path);
        let explicit = named.contains(&path);
        if !explicit && (pictures::is_picture(&path) || working.is_none()) {
            continue;
        }
        stageable(&path, working.as_ref())?;
        let working = String::from_utf8_lossy(&working.unwrap_or_default()).into_owned();
        let index = local.index(&path).unwrap_or_default();
        found.push((path, index, working));
    }
    Ok(found)
}

/// Stages `text` as the file at `path`, unstaging it if it is the base.
fn stage(local: &Local, path: &str, text: &str) -> Result<(), CliError> {
    if local.base_text(path).as_deref() == Some(text) {
        local.set_staged(path, None)
    } else {
        local.set_staged(path, Some(text))
    }
}

/// `kuria docs add <path>…`: stages each file as it now is, a directory's
/// changed files for a directory; the working files stay as they are
/// (`clone.add`).
// r[impl clone.add]
pub fn add(here: &Path, given: &[String]) -> Result<Answer, CliError> {
    let local = Local::open(here)?;
    if given.is_empty() {
        return Err(CliError::refused(
            "BAD_REQUEST",
            "Name the files to stage, `.` for every one, or `kuria docs add -p` to choose hunks",
        ));
    }
    let files = to_stage(&local, here, given)?;
    let mut staged = Vec::new();
    for (path, index, working) in &files {
        if index != working {
            stage(&local, path, working)?;
            staged.push(path.clone());
        }
    }
    Ok(Answer {
        markdown: if staged.is_empty() {
            "Nothing to stage.".to_owned()
        } else {
            format!("Staged {}.", staged.join(", "))
        },
        json: json!({ "staged": staged }),
    })
}

/// A hunk of a file's change: the ops it groups, as similar groups them
/// with three lines of context.
fn hunk_text(path: &str, old: &[&str], new: &[&str], ops: &[DiffOp]) -> String {
    let (first, last) = (&ops[0], &ops[ops.len() - 1]);
    let old_start = first.old_range().start;
    let new_start = first.new_range().start;
    let old_len = last.old_range().end - old_start;
    let new_len = last.new_range().end - new_start;
    let mut text = format!(
        "--- a/{path}\n+++ b/{path}\n@@ -{},{old_len} +{},{new_len} @@\n",
        old_start + 1,
        new_start + 1
    );
    let mut line = |sign: char, l: &str| {
        text.push(sign);
        text.push_str(l);
        if !l.ends_with('\n') {
            text.push_str("\n\\ No newline at end of file\n");
        }
    };
    for op in ops {
        match op {
            DiffOp::Equal { .. } => op.old_range().for_each(|i| line(' ', old[i])),
            _ => {
                op.old_range().for_each(|i| line('-', old[i]));
                op.new_range().for_each(|i| line('+', new[i]));
            }
        }
    }
    text
}

/// `old` with the change to `new` made only in the hunks `taken` says to
/// take, by their order.
fn with_hunks(old: &[&str], new: &[&str], groups: &[Vec<DiffOp>], taken: &[bool]) -> String {
    let mut text = String::new();
    let mut at = 0;
    for (group, take) in groups.iter().zip(taken) {
        for op in group {
            if matches!(op, DiffOp::Equal { .. }) {
                continue;
            }
            let range = op.old_range();
            text.extend(old[at..range.start].iter().copied());
            if *take {
                text.extend(new[op.new_range()].iter().copied());
            } else {
                text.extend(old[range.clone()].iter().copied());
            }
            at = range.end;
        }
    }
    text.extend(old[at..].iter().copied());
    text
}

const HELP: &str = "y - stage this hunk
n - don't stage this hunk
q - quit; stage no more hunks
a - stage this hunk and the file's later ones
d - stage none of the file's later hunks
? - say this
";

/// `kuria docs add -p [<path>…]`: offers each changed hunk of each file in turn
/// and stages only those chosen, reading each answer as a line of `input`
/// and asking on `prompts` (`clone.add`). The working files stay as they
/// are.
// r[impl clone.add]
pub fn add_patch(
    here: &Path,
    given: &[String],
    input: &mut dyn BufRead,
    prompts: &mut dyn Write,
) -> Result<Answer, CliError> {
    let local = Local::open(here)?;
    let files = to_stage(&local, here, given)?;
    let mut staged = Vec::new();
    let mut quit = false;
    for (path, index, working) in &files {
        if quit {
            break;
        }
        let (old, new) = (lines(index), lines(working));
        let groups = group_diff_ops(capture_diff_slices(Algorithm::Myers, &old, &new), 3);
        let mut taken = vec![false; groups.len()];
        let mut rest: Option<bool> = None;
        let mut i = 0;
        while i < groups.len() {
            if let Some(all) = rest {
                taken[i] = all;
                i += 1;
                continue;
            }
            let shown = hunk_text(path, &old, &new, &groups[i]);
            // A prompt that can't be written leaves the answer to the input.
            let _ = write!(
                prompts,
                "{shown}({}/{}) Stage this hunk [y,n,q,a,d,?]? ",
                i + 1,
                groups.len()
            );
            let _ = prompts.flush();
            let mut answer = String::new();
            let read = input
                .read_line(&mut answer)
                .map_err(|e| CliError::unavailable(format!("reading the answer: {e}")))?;
            match (read, answer.trim()) {
                (0, _) | (_, "q") => {
                    quit = true;
                    break;
                }
                (_, "y") => taken[i] = true,
                (_, "n") => {}
                (_, "a") => {
                    taken[i] = true;
                    rest = Some(true);
                }
                (_, "d") => rest = Some(false),
                _ => {
                    let _ = write!(prompts, "{HELP}");
                    continue;
                }
            }
            i += 1;
        }
        if taken.iter().any(|t| *t) {
            stage(&local, path, &with_hunks(&old, &new, &groups, &taken))?;
            staged.push(path.clone());
        }
    }
    Ok(Answer {
        markdown: if staged.is_empty() {
            "Nothing staged.".to_owned()
        } else {
            format!("Staged hunks of {}.", staged.join(", "))
        },
        json: json!({ "staged": staged }),
    })
}

#[cfg(test)]
mod tests {
    use super::super::base::{AktBase, Recorded};
    use super::*;

    fn clone_with(base: &str, working: &str) -> (tempfile::TempDir, Local) {
        let dir = tempfile::tempdir().unwrap();
        let mut local = Local::start(
            dir.path(),
            Recorded {
                url: "https://kuria.example".to_owned(),
                number: "D-1".to_owned(),
            },
        )
        .unwrap();
        local.bases.akter.insert(
            "D-1".to_owned(),
            AktBase {
                path: "D-1-doc.md".to_owned(),
                revision: 1,
                live: None,
            },
        );
        local.save().unwrap();
        local.set_base("D-1-doc.md", Some(base.as_bytes())).unwrap();
        local
            .set_working("D-1-doc.md", Some(working.as_bytes()))
            .unwrap();
        (dir, local)
    }

    const BASE: &str =
        "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n";

    // r[verify clone.add]
    // r[verify clone.status]
    #[test]
    fn stages_the_hunks_chosen() {
        let working = BASE.replace("two", "TWO").replace("eleven", "ELEVEN");
        let (dir, local) = clone_with(BASE, &working);
        let root = dir.path();
        let mut prompts = Vec::new();
        let answer = add_patch(root, &[], &mut "x\nn\ny\n".as_bytes(), &mut prompts).unwrap();
        assert_eq!(answer.json["staged"], json!(["D-1-doc.md"]));
        let prompts = String::from_utf8(prompts).unwrap();
        assert!(
            prompts.contains("+TWO") && prompts.contains("(2/2)"),
            "{prompts}"
        );
        assert!(prompts.contains("y - stage this hunk"), "{prompts}");
        assert_eq!(
            local.staged("D-1-doc.md").unwrap(),
            BASE.replace("eleven", "ELEVEN")
        );
        // The working file stays as it was.
        assert_eq!(local.working("D-1-doc.md").unwrap(), working.as_bytes());
        let s = status_of(&local).unwrap();
        assert_eq!(s.staged, ["D-1-doc.md"]);
        assert_eq!(s.unstaged, ["D-1-doc.md"]);
        let staged = diff(root, true, &[]).unwrap().markdown;
        assert!(
            staged.contains("+ELEVEN") && !staged.contains("+TWO"),
            "{staged}"
        );
        let unstaged = diff(root, false, &[]).unwrap().markdown;
        assert!(
            unstaged.contains("+TWO") && !unstaged.contains("+ELEVEN"),
            "{unstaged}"
        );
        // The rest, whole.
        add(root, &["D-1-doc.md".to_owned()]).unwrap();
        assert_eq!(local.staged("D-1-doc.md").unwrap(), working);
        assert_eq!(status_of(&local).unwrap().unstaged, Vec::<String>::new());
        // Back to the base: nothing staged.
        local
            .set_working("D-1-doc.md", Some(BASE.as_bytes()))
            .unwrap();
        add(root, &[".".to_owned()]).unwrap();
        assert!(local.staged("D-1-doc.md").is_none());
    }

    // r[verify clone.add]
    #[test]
    fn refuses_conflicts_pictures_and_new_files() {
        let marked = "<<<<<<< ours\na\n=======\nb\n>>>>>>> kuria\n";
        let (dir, local) = clone_with(BASE, marked);
        let root = dir.path();
        let refused = add(root, &["D-1-doc.md".to_owned()]).unwrap_err();
        assert_eq!(refused.code, "CONFLICTED");
        assert!(add_patch(root, &[], &mut "y\n".as_bytes(), &mut Vec::new()).is_err());
        assert_eq!(status_of(&local).unwrap().conflicted, ["D-1-doc.md"]);
        local
            .set_working("new.md", Some(b"---\nnumber: D-9\n---\n"))
            .unwrap();
        assert!(add(root, &["new.md".to_owned()]).is_err());
        assert_eq!(status_of(&local).unwrap().untracked, ["new.md"]);
        local
            .set_base("attachments/IMG-1.png", Some(b"png"))
            .unwrap();
        local
            .set_working("attachments/IMG-1.png", Some(b"png!"))
            .unwrap();
        assert!(add(root, &["attachments/IMG-1.png".to_owned()]).is_err());
        local.set_working("D-1-doc.md", None).unwrap();
        assert!(add(root, &["D-1-doc.md".to_owned()]).is_err());
        assert_eq!(status_of(&local).unwrap().removed, ["D-1-doc.md"]);
    }
}
