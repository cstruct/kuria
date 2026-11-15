//! Sections: an akt's members laid out in nested sections, in a `sections`
//! field (`akt.field.sections`).
//!
//! An item is a member, by number, or a section with an optional title and
//! Markdown text, whether it is collapsible, and its own items. Which
//! members an akt has isn't the engine's to know, so the stores check a
//! value against them ([`check`]) and drop a member that leaves
//! ([`without`]).

use serde_json::{Map, Value as Json, json};

use crate::outcomes::{Outcome, of_all};

/// An item of a `sections` value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// A member of the akt, by number.
    Akt(String),
    Section(Section),
}

/// A section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub title: Option<String>,
    /// Markdown.
    pub text: Option<String>,
    pub collapsible: bool,
    pub items: Vec<Item>,
}

/// Reads a stored or given `sections` value: a list of items, each an akt
/// number or `{"title", "text", "collapsible", "items"}`, all optional but
/// `items`.
// r[impl akt.field.sections]
pub fn from_json(json: &Json) -> Result<Vec<Item>, String> {
    let items = json
        .as_array()
        .ok_or("expected a list of akt numbers and sections")?;
    items.iter().map(item_from_json).collect()
}

fn item_from_json(json: &Json) -> Result<Item, String> {
    if let Some(number) = json.as_str() {
        if number.is_empty() {
            return Err("an akt number can't be empty".to_owned());
        }
        return Ok(Item::Akt(number.to_owned()));
    }
    let object = crate::value::members(json, &["title", "text", "collapsible", "items"])?;
    let text = |key: &str| match object.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
        _ => Err(format!("a section's `{key}` is a non-empty string")),
    };
    let title = text("title")?;
    if title.as_ref().is_some_and(|t| t.contains(['\n', '\r'])) {
        return Err("a section's title is one line".to_owned());
    }
    let collapsible = match object.get("collapsible") {
        None | Some(Json::Null) => false,
        Some(Json::Bool(b)) => *b,
        Some(_) => return Err("a section's `collapsible` is true or false".to_owned()),
    };
    Ok(Item::Section(Section {
        title,
        text: text("text")?,
        collapsible,
        items: from_json(object.get("items").unwrap_or(&json!([])))?,
    }))
}

/// `items` as a stored or given `sections` value.
#[must_use]
pub fn to_json(items: &[Item]) -> Json {
    Json::Array(
        items
            .iter()
            .map(|item| match item {
                Item::Akt(number) => json!(number),
                Item::Section(s) => {
                    let mut object = Map::new();
                    if let Some(title) = &s.title {
                        object.insert("title".to_owned(), json!(title));
                    }
                    if let Some(text) = &s.text {
                        object.insert("text".to_owned(), json!(text));
                    }
                    if s.collapsible {
                        object.insert("collapsible".to_owned(), json!(true));
                    }
                    object.insert("items".to_owned(), to_json(&s.items));
                    Json::Object(object)
                }
            })
            .collect(),
    )
}

/// Every akt number `items` names, in order, nested ones included.
#[must_use]
pub fn numbers(items: &[Item]) -> Vec<&str> {
    let mut found = Vec::new();
    for item in items {
        match item {
            Item::Akt(number) => found.push(number.as_str()),
            Item::Section(s) => found.extend(numbers(&s.items)),
        }
    }
    found
}

/// Refuses `items` naming an akt that isn't one of `members`, given as
/// number and akt type key, one more than once, or, if `of` names akt
/// types, one of another type.
// r[impl akt.field.sections]
pub fn check(items: &[Item], members: &[(&str, &str)], of: &[String]) -> Result<(), String> {
    let named = numbers(items);
    for (i, number) in named.iter().enumerate() {
        if named[..i].contains(number) {
            return Err(format!("`{number}` is in the sections twice"));
        }
        let Some((_, key)) = members.iter().find(|(n, _)| n == number) else {
            return Err(format!("`{number}` isn't a member of the akt"));
        };
        if !of.is_empty() && !of.iter().any(|k| k == key) {
            return Err(format!(
                "`{number}` is a `{key}`, which the sections don't place"
            ));
        }
    }
    Ok(())
}

/// `items` without `number`, as when that member leaves the akt.
// r[impl akt.field.sections]
#[must_use]
pub fn without(items: &[Item], number: &str) -> Vec<Item> {
    items
        .iter()
        .filter(|item| !matches!(item, Item::Akt(n) if n == number))
        .map(|item| match item {
            Item::Section(s) => Item::Section(Section {
                items: without(&s.items, number),
                ..s.clone()
            }),
            other @ Item::Akt(_) => other.clone(),
        })
        .collect()
}

/// `items` with the akter `keep` says to keep, and every section still
/// holding something, as a copy keeps them (`akt.copy.only`), each number
/// renamed by `rename`.
// r[impl akt.copy.only]
#[must_use]
pub fn copied(
    items: &[Item],
    keep: &dyn Fn(&str) -> bool,
    rename: &dyn Fn(&str) -> String,
) -> Vec<Item> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Akt(n) if keep(n) => Some(Item::Akt(rename(n))),
            Item::Akt(_) => None,
            Item::Section(s) => {
                let items = copied(&s.items, keep, rename);
                (!items.is_empty()).then(|| Item::Section(Section { items, ..s.clone() }))
            }
        })
        .collect()
}

/// A section's outcome, from those of the members it names, as
/// `outcome_of` gives them (`None` for a member without outcomes), and of
/// its nested sections.
// r[impl akt.outcome.section]
pub fn outcome(section: &Section, outcome_of: &dyn Fn(&str) -> Option<Outcome>) -> Outcome {
    of_all(section.items.iter().filter_map(|item| match item {
        Item::Akt(n) => outcome_of(n),
        Item::Section(s) => Some(outcome(s, outcome_of)),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Vec<Item> {
        from_json(&json!([
            { "title": "Setup", "collapsible": true, "items": ["P-1-T1",
                { "title": "Network", "text": "Plug it in.", "items": ["P-1-T2"] }] },
            { "items": ["P-1-T3"] },
            "P-1-N1"
        ]))
        .unwrap()
    }

    // r[verify akt.field.sections]
    #[test]
    fn sections_read_back_and_name_only_members_once() {
        let items = tree();
        assert_eq!(from_json(&to_json(&items)).unwrap(), items);
        assert_eq!(numbers(&items), ["P-1-T1", "P-1-T2", "P-1-T3", "P-1-N1"]);
        let members = [
            ("P-1-T1", "test"),
            ("P-1-T2", "test"),
            ("P-1-T3", "test"),
            ("P-1-N1", "note"),
        ];
        assert!(check(&items, &members, &[]).is_ok());
        assert!(
            check(&items, &members[..3], &[])
                .unwrap_err()
                .contains("isn't a member")
        );
        assert!(
            check(&items, &members, &["test".to_owned()])
                .unwrap_err()
                .contains("`note`")
        );
        let twice = from_json(&json!(["P-1-T1", { "items": ["P-1-T1"] }])).unwrap();
        assert!(check(&twice, &members, &[]).unwrap_err().contains("twice"));
        for bad in [
            json!({}),
            json!([{ "title": "A\nB", "items": [] }]),
            json!([""]),
            json!([{ "items": [], "other": 1 }]),
        ] {
            assert!(from_json(&bad).is_err(), "{bad}");
        }
    }

    // r[verify akt.field.sections]
    // r[verify akt.copy.only]
    #[test]
    fn a_member_leaves_and_a_copy_keeps_what_it_names() {
        let items = tree();
        assert_eq!(
            numbers(&without(&items, "P-1-T2")),
            ["P-1-T1", "P-1-T3", "P-1-N1"]
        );
        let kept = copied(&items, &|n| n == "P-1-T2", &|n| n.replace("P-1", "P-2"));
        assert_eq!(
            to_json(&kept),
            json!([{ "title": "Setup", "collapsible": true, "items": [
                { "title": "Network", "text": "Plug it in.", "items": ["P-2-T2"] }] }])
        );
    }

    // r[verify akt.outcome.section]
    #[test]
    fn a_section_s_outcome_follows_what_it_holds() {
        let Item::Section(setup) = &tree()[0] else {
            panic!()
        };
        let outcome_of = |n: &str| match n {
            "P-1-T1" => Some(Outcome::Pass),
            "P-1-T2" => Some(Outcome::Skip),
            _ => None,
        };
        assert_eq!(outcome(setup, &outcome_of), Outcome::Pass);
        let failing = |n: &str| {
            (n == "P-1-T2")
                .then_some(Outcome::Fail)
                .or(Some(Outcome::Pass))
        };
        assert_eq!(outcome(setup, &failing), Outcome::Fail);
        let empty = Section {
            title: None,
            text: None,
            collapsible: false,
            items: vec![],
        };
        assert_eq!(outcome(&empty, &outcome_of), Outcome::Pending);
    }
}
