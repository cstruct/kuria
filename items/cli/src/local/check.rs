//! `kuria check`: a whole akt directory checked as a server would hold it,
//! every problem named with its file (`cli.local.check`).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use kuria_config::{check_field_references, check_state_references};
use kuria_engine::rules::containment;
use serde_json::json;

use super::answers;
use super::dir::{AktDir, LocalAkt, LocalType, files, read_file, read_type};
use crate::error::CliError;
use crate::output::Answer;

/// What is wrong, and in which file.
struct Problems {
    root: PathBuf,
    found: Vec<(String, String)>,
}

impl Problems {
    fn add(&mut self, path: &Path, message: impl Into<String>) {
        let file = path
            .strip_prefix(&self.root)
            .unwrap_or(path)
            .display()
            .to_string();
        let message = message.into();
        // A refusal reading the file already names it.
        let message = message
            .strip_prefix(&format!("{}: ", path.display()))
            .map_or(message.clone(), str::to_owned);
        self.found.push((file, message));
    }

    /// What is wrong with `akt`, named by its number too if it is nested.
    fn of(&mut self, akt: &LocalAkt, message: impl Into<String>) {
        let message = message.into();
        if akt.nested {
            self.add(&akt.path, format!("{}: {message}", akt.number));
        } else {
            self.add(&akt.path, message);
        }
    }
}

/// A link as a file lists it: its name, the other akt, and any pin.
pub(super) fn said(text: &str) -> Option<(&str, &str, Option<i32>)> {
    let (name, target) = text.trim().rsplit_once(' ')?;
    Some(match target.split_once('@') {
        Some((number, pin)) => (name, number, pin.parse().ok()),
        None => (name, target, None),
    })
}

/// Checks the akt directory at `root`.
// r[impl cli.local.check]
pub fn check(root: &Path) -> Result<Answer, CliError> {
    // A directory that isn't one is refused as every command refuses it.
    AktDir::open(root).map(drop).or_else(|e| {
        if e.code == "NOT_AN_AKT_DIRECTORY" {
            Err(e)
        } else {
            Ok(())
        }
    })?;
    let mut problems = Problems {
        root: root.to_owned(),
        found: Vec::new(),
    };

    let mut types: BTreeMap<String, LocalType> = BTreeMap::new();
    for path in files(&root.join("types"), "json")? {
        match read_type(&path) {
            Ok(t) => {
                types.insert(t.ty.key().to_owned(), t);
            }
            Err(e) => problems.add(&path, e.message),
        }
    }
    for (key, t) in &types {
        let others: Vec<_> = types
            .values()
            .filter(|o| o.ty.key() != key)
            .map(|o| o.ty.clone())
            .collect();
        let mut found = check_state_references(&t.ty, &others);
        found.extend(check_field_references(&t.ty, &others));
        for p in found {
            problems.add(
                &root.join("types").join(format!("{key}.json")),
                p.to_string(),
            );
        }
    }

    let mut akter = Vec::new();
    for path in files(&root.join("akter"), "md")? {
        for akt in read_file(&path, &types) {
            match akt {
                Ok(akt) => akter.push(akt),
                Err(e) => problems.add(&path, e.message),
            }
        }
    }
    akter.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.number.cmp(&b.number)));
    let dir = AktDir {
        root: root.to_owned(),
        types,
        akter,
    };
    check_akter(&dir, &mut problems);

    if problems.found.is_empty() {
        return Ok(Answer {
            markdown: format!(
                "{} is consistent: {} akt types, {} akter.",
                root.display(),
                dir.types.len(),
                dir.akter.len()
            ),
            json: json!({ "consistent": true, "types": dir.types.len(), "akter": dir.akter.len(), "problems": [] }),
        });
    }
    let lines: Vec<String> = problems
        .found
        .iter()
        .map(|(file, message)| format!("- {file}: {message}"))
        .collect();
    Err(CliError::refused(
        "INCONSISTENT",
        format!(
            "{} problems in {}:\n{}",
            lines.len(),
            root.display(),
            lines.join("\n")
        ),
    ))
}

fn check_akter(dir: &AktDir, problems: &mut Problems) {
    let mut numbers: HashMap<String, &LocalAkt> = HashMap::new();
    let mut ranks: HashMap<&str, &LocalAkt> = HashMap::new();
    for akt in &dir.akter {
        let t = dir.type_of(akt);
        if let Err(e) = answers::values(dir, akt) {
            problems.of(akt, e.message);
        }
        if t.ty.number().read(&akt.number).is_none() {
            problems.of(
                akt,
                format!(
                    "`{}` isn't a number `{}` gives (`{}`)",
                    akt.number,
                    t.ty.key(),
                    t.ty.document().number
                ),
            );
        }
        if let Some(other) = numbers.insert(akt.number.to_lowercase(), akt) {
            problems.of(akt, format!("has the number of {}", other.path.display()));
        }
        if let Some(other) = ranks.insert(&akt.rank, akt) {
            problems.of(akt, format!("has the rank of {}", other.number));
        }
        check_containers(dir, akt, problems);
        check_links(dir, akt, problems);
    }
}

// r[impl cli.local.check]
fn check_containers(dir: &AktDir, akt: &LocalAkt, problems: &mut Problems) {
    let ty = &dir.type_of(akt).ty;
    let mut exclusive: HashMap<&str, &str> = HashMap::new();
    for (i, number) in akt.containers.iter().enumerate() {
        if akt.containers[..i].contains(number) {
            problems.of(akt, format!("is in `{number}` twice"));
            continue;
        }
        if *number == akt.number {
            problems.of(akt, "is in itself");
            continue;
        }
        let Some(container) = dir.akt(number) else {
            problems.of(akt, format!("is in `{number}`, which the directory lacks"));
            continue;
        };
        let container_ty = &dir.type_of(container).ty;
        match container_ty.contains(ty.key()) {
            None => problems.of(
                akt,
                format!(
                    "is in `{number}`, but a `{}` can't contain a `{}`",
                    container_ty.key(),
                    ty.key()
                ),
            ),
            Some(contains) if contains.exclusive => {
                if let Some(other) = exclusive.insert(container_ty.key(), number) {
                    problems.of(
                        akt,
                        format!(
                            "is in `{other}` and `{number}`, but may be in only one `{}`",
                            container_ty.key()
                        ),
                    );
                }
            }
            Some(_) => {}
        }
    }
    let members_of = |n: &str| {
        dir.members_of(n)
            .map(|m| m.number.clone())
            .collect::<Vec<_>>()
    };
    if containment::contains(&akt.number, &akt.number, members_of) {
        problems.of(akt, "contains itself, through its members");
    }
}

// r[impl cli.local.check]
fn check_links(dir: &AktDir, akt: &LocalAkt, problems: &mut Problems) {
    let ty = &dir.type_of(akt).ty;
    for text in &akt.links {
        let Some((name, number, pin)) = said(text) else {
            problems.of(akt, format!("the link `{text}` isn't `<name> <number>`"));
            continue;
        };
        let Some(other) = dir.akt(number) else {
            problems.of(
                akt,
                format!("the link `{text}` names an akt the directory lacks"),
            );
            continue;
        };
        let other_ty = &dir.type_of(other).ty;
        let at = pin.map(|p| format!("@{p}")).unwrap_or_default();
        let (target, counterpart) =
            if let Some(def) = ty.document().links.iter().find(|l| l.name == name) {
                if !def.to.iter().any(|k| k == other_ty.key()) {
                    problems.of(
                        akt,
                        format!(
                            "the link `{text}` goes to a `{}`, which `{}` doesn't link to",
                            other_ty.key(),
                            def.key
                        ),
                    );
                }
                (other, format!("{} {}{at}", def.inverse, akt.number))
            } else if let Some(def) = other_ty.document().links.iter().find(|l| l.inverse == name) {
                if !def.to.iter().any(|k| k == ty.key()) {
                    problems.of(akt,
                    format!(
                        "the link `{text}` comes from a `{}`, whose `{}` doesn't link to a `{}`",
                        other_ty.key(),
                        def.key,
                        ty.key()
                    ),
                );
                }
                (akt, format!("{} {}{at}", def.name, akt.number))
            } else {
                problems.of(
                    akt,
                    format!("the link `{text}` is of no kind either akt's type declares"),
                );
                continue;
            };
        // A pin names a revision of the link's target.
        if let Some(pin) = pin
            && !(1..=target.revision).contains(&pin)
        {
            problems.of(
                akt,
                format!(
                    "the link `{text}` pins a revision {} hasn't reached",
                    target.number
                ),
            );
        }
        if !other.links.iter().any(|l| l.trim() == counterpart) {
            problems.of(
                akt,
                format!(
                    "lists `{text}`, but {} doesn't list `{counterpart}`",
                    other.number
                ),
            );
        }
    }
}
