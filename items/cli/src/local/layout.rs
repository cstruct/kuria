//! Where an akt directory stores each akt, and how a file holding several
//! reads (`cli.local.nesting`, `cli.local.names`).
//!
//! An akt numbered after its parent is stored in its parent's file; any
//! other begins a file of its own, named by its number and title. A write
//! moves the akter it touched, with every other akt of their files, to
//! where they belong, and writes each file whole.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::dir::{AktDir, LocalAkt};
use super::write::text_of;

/// The most characters of a title a file's name keeps.
const SLUG: usize = 40;

/// `title` as a file name's: lower case, every run of characters other
/// than ASCII letters and digits one `-`, cut at a `-` to at most 40.
// r[impl cli.local.names]
#[must_use]
pub fn slug(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.len() <= SLUG {
        return slug.to_owned();
    }
    // At the last `-` that keeps it short enough, else at 40.
    let cut = slug[..=SLUG].rfind('-').unwrap_or(SLUG);
    slug[..cut].trim_end_matches('-').to_owned()
}

/// The name of the file that `number`, titled `title`, begins.
// r[impl cli.local.names]
#[must_use]
pub fn file_name(number: &str, title: Option<&str>) -> String {
    match title.map(slug).filter(|s| !s.is_empty()) {
        Some(slug) => format!("{number}-{slug}.md"),
        None => format!("{number}.md"),
    }
}

/// The akt `akt`'s number was made from, if the directory has it.
// r[impl cli.local.nesting]
#[must_use]
pub fn parent<'a>(dir: &'a AktDir, akt: &LocalAkt) -> Option<&'a LocalAkt> {
    let pattern = dir.type_of(akt).ty.number();
    let number = pattern.parent_of(&akt.number, &|n| n != akt.number && dir.akt(n).is_some())?;
    dir.akt(&number)
}

/// The file `akt` belongs in: its parent's, else one of its own.
#[must_use]
pub fn home(dir: &AktDir, akt: &LocalAkt) -> PathBuf {
    match parent(dir, akt) {
        Some(parent) => parent.path.clone(),
        None => dir.root.join("akter").join(file_name(
            &akt.number,
            super::answers::title(dir, akt).as_deref(),
        )),
    }
}

/// Moves the akter of the files `from` to where they belong, parents
/// first, and answers the files they are in now.
// r[impl cli.local.nesting]
pub fn relocate(dir: &mut AktDir, from: &BTreeSet<PathBuf>) -> BTreeSet<PathBuf> {
    let mut moving: Vec<String> = dir
        .akter
        .iter()
        .filter(|a| from.contains(&a.path))
        .map(|a| a.number.clone())
        .collect();
    // A parent's number is part of each of its akter's, so it is shorter.
    moving.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    let mut to = BTreeSet::new();
    for number in moving {
        let akt = dir.akt(&number).expect("moving akter exist");
        let nested = parent(dir, akt).is_some();
        let path = home(dir, akt);
        to.insert(path.clone());
        let akt = dir
            .akter
            .iter_mut()
            .find(|a| a.number == number)
            .expect("moving akter exist");
        akt.path = path;
        akt.nested = nested;
    }
    to
}

/// The file at `path` as the directory holds it: the akt it begins with,
/// then its nested akter in the directory's order, each after its parent.
// r[impl cli.local.layout]
#[must_use]
pub fn file_text(dir: &AktDir, path: &Path) -> String {
    let mut held: Vec<&LocalAkt> = dir.akter.iter().filter(|a| a.path == path).collect();
    // As the directory orders them now, a move included.
    held.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.number.cmp(&b.number)));
    let mut text = String::new();
    let mut written: Vec<&str> = Vec::new();
    if let Some(first) = held.iter().find(|a| !a.nested) {
        text.push_str(&text_of(dir, first, true));
        written.push(&first.number);
    }
    // Each round writes those whose parent is written, in order.
    loop {
        let next: Vec<&LocalAkt> = held
            .iter()
            .filter(|a| !written.contains(&a.number.as_str()))
            .filter(|a| parent(dir, a).is_none_or(|p| written.contains(&p.number.as_str())))
            .copied()
            .collect();
        if next.is_empty() {
            break;
        }
        for akt in next {
            text.push_str(&format!("\n<!-- akt: {} -->\n", akt.number));
            text.push_str(&text_of(dir, akt, false));
            written.push(&akt.number);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cli.local.names]
    #[test]
    fn names_a_file_by_its_number_and_title() {
        assert_eq!(
            file_name(
                "PLAN-008",
                Some("The CLI in local mode: akter as Markdown files")
            ),
            "PLAN-008-the-cli-in-local-mode-akter-as-markdown.md"
        );
        assert_eq!(
            file_name("PLAN-009", Some("Fewer, readable files")),
            "PLAN-009-fewer-readable-files.md"
        );
        assert_eq!(file_name("T-1", Some("Ärende — ÖVER")), "T-1-rende-ver.md");
        assert_eq!(file_name("T-1", Some("!!!")), "T-1.md");
        assert_eq!(file_name("T-1", None), "T-1.md");
        assert_eq!(slug(&"a".repeat(50)), "a".repeat(40));
        assert!(slug("one two three four five six seven eight nine").len() <= 40);
    }
}
