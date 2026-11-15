//! `markdown` fields (`akt.field.markdown`): their headings and the ids
//! they keep, and their references to other akter
//! (`akt.field.markdown.references`).
//!
//! A value is kept as it was written. Writing one only adds `{#id}` to
//! the headings without one, so that the text reads and diffs as its
//! author left it.

use std::collections::HashSet;
use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// The Markdown a `markdown` field holds: `CommonMark` with GitHub's
/// tables, task lists, strikethrough and footnotes, and heading ids.
pub(crate) fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_HEADING_ATTRIBUTES
}

/// One heading of a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    /// 1 to 6.
    pub level: u8,
    /// Its text, without inline markup.
    pub text: String,
    /// Its id, if it has one.
    pub id: Option<String>,
    /// Where its first line ends in the value, before any closing `#`s or
    /// trailing spaces: where an id goes.
    end_of_text: usize,
}

/// A link to an akt in a `markdown` field: `kuria:<number>`, optionally
/// with `@<version>` and `#<heading id>`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference {
    pub number: String,
    pub version: Option<u32>,
    pub heading: Option<String>,
}

impl Reference {
    /// Reads a link's destination; `None` if it isn't a reference.
    #[must_use]
    pub fn parse(destination: &str) -> Option<Self> {
        let rest = destination.strip_prefix("kuria:")?;
        let (rest, heading) = match rest.split_once('#') {
            Some((rest, heading)) => (rest, Some(heading.to_owned())),
            None => (rest, None),
        };
        let (number, version) = match rest.split_once('@') {
            Some((number, version)) => (number, Some(version.parse::<u32>().ok()?)),
            None => (rest, None),
        };
        if number.is_empty() || version == Some(0) || heading.as_deref() == Some("") {
            return None;
        }
        Some(Self {
            number: number.to_owned(),
            version,
            heading,
        })
    }
}

impl std::fmt::Display for Reference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kuria:{}", self.number)?;
        if let Some(version) = self.version {
            write!(f, "@{version}")?;
        }
        if let Some(heading) = &self.heading {
            write!(f, "#{heading}")?;
        }
        Ok(())
    }
}

fn level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// The headings of `text`, in order.
#[must_use]
pub fn headings(text: &str) -> Vec<Heading> {
    let mut found = Vec::new();
    let mut open: Option<(u8, Option<String>, Range<usize>, String)> = None;
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level: l, id, .. }) => {
                open = Some((level(l), id.map(|i| i.to_string()), range, String::new()));
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, _, _, words)) = &mut open {
                    words.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, id, range, words)) = open.take() {
                    found.push(Heading {
                        level,
                        text: words.trim().to_owned(),
                        id,
                        end_of_text: end_of_text(text, &range),
                    });
                }
            }
            _ => {}
        }
    }
    found
}

/// Where a heading's first line ends, before an ATX heading's closing
/// sequence and any trailing spaces.
fn end_of_text(text: &str, range: &Range<usize>) -> usize {
    let source = &text[range.clone()];
    let line = source.split(['\n', '\r']).next().unwrap_or_default();
    let mut end = line.trim_end();
    if end.starts_with('#') {
        // An ATX heading: its closing `#`s, if a space comes before them.
        let without = end.trim_end_matches('#');
        if without.len() < end.len() && without.ends_with([' ', '\t']) {
            end = without.trim_end();
        }
    }
    range.start + end.len()
}

/// The references of `text`, in order, each once.
#[must_use]
pub fn references(text: &str) -> Vec<Reference> {
    let mut found = Vec::new();
    for event in Parser::new_ext(text, options()) {
        if let Event::Start(Tag::Link { dest_url, .. }) = event
            && let Some(reference) = Reference::parse(&dest_url)
            && !found.contains(&reference)
        {
            found.push(reference);
        }
    }
    found
}

/// Whether `id` may be a heading id: lowercase ASCII letters, digits, `-`
/// and `_`, starting with a letter.
#[must_use]
pub fn is_id(id: &str) -> bool {
    id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// A heading's text as an id: in lower case, every run of characters other
/// than ASCII letters and digits made one `-`, trimmed of `-`; `section` if
/// that leaves nothing, or if it would start with a digit.
#[must_use]
pub fn slug(text: &str) -> String {
    let mut slug = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.starts_with(|c: char| c.is_ascii_lowercase()) {
        slug.to_owned()
    } else if slug.is_empty() {
        "section".to_owned()
    } else {
        format!("section-{slug}")
    }
}

/// Gives every heading without an id in `values`, the `markdown` fields of
/// one akt, an id unique across them all, keeping those they have
/// (`akt.field.markdown`). Refuses an id that isn't one, or that two
/// headings share, saying which.
// r[impl akt.field.markdown]
pub fn give_ids(values: &mut [&mut String]) -> Result<(), String> {
    let all: Vec<Vec<Heading>> = values.iter().map(|v| headings(v)).collect();
    let mut taken = HashSet::new();
    for heading in all.iter().flatten() {
        if let Some(id) = &heading.id {
            if !is_id(id) {
                return Err(format!(
                    "`{id}` isn't a heading id: lowercase letters, digits, `-` and `_`, starting with a letter"
                ));
            }
            if !taken.insert(id.clone()) {
                return Err(format!("two headings have the id `{id}`"));
            }
        }
    }
    for (value, headings) in values.iter_mut().zip(all) {
        // Ids chosen in order, then put in from the end, so that the
        // offsets before stay right.
        let mut given = Vec::new();
        for heading in headings.iter().filter(|h| h.id.is_none()) {
            let base = slug(&heading.text);
            let mut id = base.clone();
            let mut n = 2;
            while taken.contains(&id) {
                id = format!("{base}-{n}");
                n += 1;
            }
            taken.insert(id.clone());
            given.push((heading.end_of_text, id));
        }
        for (at, id) in given.into_iter().rev() {
            value.insert_str(at, &format!(" {{#{id}}}"));
        }
    }
    Ok(())
}

/// Per link kind that a `markdown` field of `ty` keeps its references as
/// (`akt.field.markdown.references`), the akt numbers `fields` reference
/// through those fields, in order, each once. Every such kind is listed,
/// with no numbers if nothing is referenced, so that its links can be
/// cleared.
// r[impl akt.field.markdown.references]
#[must_use]
pub fn referenced(
    ty: &kuria_config::AktType,
    fields: &std::collections::BTreeMap<String, crate::Value>,
) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut kept: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for field in &ty.document().fields {
        let Some(kind) = &field.references else {
            continue;
        };
        let numbers = kept.entry(kind.clone()).or_default();
        if let Some(crate::Value::Markdown(text)) = fields.get(&field.key) {
            for reference in references(text) {
                if !numbers.contains(&reference.number) {
                    numbers.push(reference.number);
                }
            }
        }
    }
    kept
}

/// The heading ids of an akt of `ty` with `fields`, across its `markdown`
/// fields, in order.
#[must_use]
pub fn heading_ids(
    ty: &kuria_config::AktType,
    fields: &std::collections::BTreeMap<String, crate::Value>,
) -> Vec<String> {
    ty.document()
        .fields
        .iter()
        .filter_map(|f| match fields.get(&f.key) {
            Some(crate::Value::Markdown(text)) => Some(headings(text)),
            _ => None,
        })
        .flatten()
        .filter_map(|h| h.id)
        .collect()
}

/// What a reference's target has, as the reader may see it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Target {
    /// Its heading ids, as of its current revision.
    pub headings: Vec<String>,
    /// Its versions, oldest first, each with its heading ids.
    pub versions: Vec<TargetVersion>,
    /// Its current revision.
    pub revision: i32,
}

/// One version of a reference's target.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetVersion {
    pub number: u32,
    pub revision: i32,
    /// Its heading ids, as of the revision it names.
    pub headings: Vec<String>,
}

/// Why `reference` is broken, if it is (`broken reference`): `target` is
/// `None` for an akt that doesn't exist or the reader may not read.
#[must_use]
pub fn broken(reference: &Reference, target: Option<&Target>) -> Option<String> {
    let Some(target) = target else {
        return Some(format!("there is no akt `{}`", reference.number));
    };
    let headings = match reference.version {
        Some(version) => match target.versions.iter().find(|v| v.number == version) {
            Some(v) => &v.headings,
            None => return Some(format!("`{}` has no version {version}", reference.number)),
        },
        None => &target.headings,
    };
    if let Some(heading) = &reference.heading
        && !headings.contains(heading)
    {
        return Some(match reference.version {
            Some(version) => format!(
                "`{}` has no heading `{heading}` in version {version}",
                reference.number
            ),
            None => format!("`{}` has no heading `{heading}`", reference.number),
        });
    }
    None
}

/// Whether `text` holds a value: anything but white space.
#[must_use]
pub fn is_empty(text: &str) -> bool {
    text.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn given(texts: &[&str]) -> Result<Vec<String>, String> {
        let mut values: Vec<String> = texts.iter().map(|t| (*t).to_owned()).collect();
        let mut refs: Vec<&mut String> = values.iter_mut().collect();
        give_ids(&mut refs)?;
        Ok(values)
    }

    // r[verify akt.field.markdown]
    #[test]
    fn headings_are_given_ids_and_keep_them() {
        let cases: [(&str, &str); 8] = [
            ("# Installing\n", "# Installing {#installing}\n"),
            (
                "## Set up `kuria` ##\n",
                "## Set up `kuria` {#set-up-kuria} ##\n",
            ),
            ("Overview\n========\n", "Overview {#overview}\n========\n"),
            ("# 2. Steps\n", "# 2. Steps {#section-2-steps}\n"),
            ("# ÅÄÖ\n", "# ÅÄÖ {#section}\n"),
            // Kept, whatever the text says now.
            (
                "# Installing Kuria {#install}\n",
                "# Installing Kuria {#install}\n",
            ),
            // Unique within the value, against ids given too.
            (
                "# Notes\n\n# Notes {#notes}\n\n# Notes\n",
                "# Notes {#notes-2}\n\n# Notes {#notes}\n\n# Notes {#notes-3}\n",
            ),
            // Text that only looks like a heading is left alone.
            ("```\n# not a heading\n```\n", "```\n# not a heading\n```\n"),
        ];
        for (text, expected) in cases {
            assert_eq!(given(&[text]).unwrap(), [expected], "{text:?}");
        }
        // Unique across an akt's fields, and parsed back as ids.
        let two = given(&["# Scope\n", "# Scope\n"]).unwrap();
        assert_eq!(two, ["# Scope {#scope}\n", "# Scope {#scope-2}\n"]);
        assert_eq!(headings(&two[1])[0].id.as_deref(), Some("scope-2"));
        assert_eq!(headings(&two[1])[0].text, "Scope");
        // Writing again changes nothing.
        assert_eq!(given(&[&two[0], &two[1]]).unwrap(), two);
    }

    // r[verify akt.field.markdown]
    #[test]
    fn refuses_ids_that_clash_or_arent_ids() {
        assert_eq!(
            given(&["# A {#same}\n", "# B {#same}\n"]),
            Err("two headings have the id `same`".to_owned())
        );
        assert!(
            given(&["# A {#Big}\n"])
                .unwrap_err()
                .contains("`Big` isn't a heading id")
        );
    }

    // r[verify akt.field.markdown.references]
    #[test]
    fn says_why_a_reference_is_broken() {
        let reference = |text: &str| Reference::parse(text).unwrap();
        let target = Target {
            headings: vec!["install".to_owned()],
            versions: vec![TargetVersion {
                number: 1,
                revision: 3,
                headings: vec!["setup".to_owned()],
            }],
            revision: 5,
        };
        assert_eq!(broken(&reference("kuria:D-1#install"), Some(&target)), None);
        assert_eq!(broken(&reference("kuria:D-1@1#setup"), Some(&target)), None);
        assert_eq!(
            broken(&reference("kuria:D-1@1#install"), Some(&target)).unwrap(),
            "`D-1` has no heading `install` in version 1"
        );
        assert_eq!(
            broken(&reference("kuria:D-1#gone"), Some(&target)).unwrap(),
            "`D-1` has no heading `gone`"
        );
        assert_eq!(
            broken(&reference("kuria:D-1@2"), Some(&target)).unwrap(),
            "`D-1` has no version 2"
        );
        assert_eq!(
            broken(&reference("kuria:D-9"), None).unwrap(),
            "there is no akt `D-9`"
        );
    }

    // r[verify akt.field.markdown.references]
    #[test]
    fn reads_references() {
        let text = "See [setup](kuria:DOCS-1-D04#install), [v2](kuria:DOCS-1-D04@2#install), \
                    [the index](kuria:DOCS-1) and [again](kuria:DOCS-1).\n\n\
                    [web](https://example.com) [bad](kuria:) [zero](kuria:A-1@0) [text](kuria:A-1@x)\n\n\
                    `[code](kuria:A-2)`\n";
        let reference = |number: &str, version, heading: Option<&str>| Reference {
            number: number.to_owned(),
            version,
            heading: heading.map(str::to_owned),
        };
        assert_eq!(
            references(text),
            [
                reference("DOCS-1-D04", None, Some("install")),
                reference("DOCS-1-D04", Some(2), Some("install")),
                reference("DOCS-1", None, None),
            ]
        );
        assert_eq!(
            references(text)[1].to_string(),
            "kuria:DOCS-1-D04@2#install"
        );
    }

    // r[verify akt.field.markdown]
    // r[verify akt.field.markdown.references]
    #[test]
    fn reads_references_in_footnotes() {
        let text = "Said[^1].\n\n[^1]: See [setup](kuria:D-4#install).\n";
        assert_eq!(
            references(text),
            [Reference {
                number: "D-4".to_owned(),
                version: None,
                heading: Some("install".to_owned()),
            }]
        );
    }
}
