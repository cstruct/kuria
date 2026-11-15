//! A clone's layout (`clone.tree`, `clone.normal`): where each file of a
//! container and its members goes, what an akt's file in a clone holds,
//! and a file in normal form.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value as Json};

use crate::error::CliError;
use crate::file::{self, Parsed};
use crate::local::layout::{file_name, slug};
use crate::model::{Shape, Value, read};
use crate::values::file_value;

/// What `akt { … }` selects for [`filed`], with the `Shape`, `Values` and
/// `Someone` fragments defined.
pub const AT_REVISION: &str = "number title state { key } type { ...Shape } fields { ...Values }";

/// An akt at a revision, as a clone holds it: its file, its title, and its
/// value of a `sections` field, if any.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Filed {
    pub text: String,
    pub title: Option<String>,
    pub sections: Option<Json>,
}

/// The akt `number`, from what `akt { AT_REVISION }` answered, as its clone
/// file says it (`clone.tree`).
pub fn filed(number: &str, akt: &Json) -> Result<Filed, CliError> {
    let shape: Shape = read(akt["type"].clone(), "the akt type")?;
    let fields: Vec<Value> = read(akt["fields"].clone(), "the fields")?;
    let values: Map<String, Json> = fields
        .iter()
        .map(|v| (v.field().to_owned(), file_value(v)))
        .collect();
    let sections = fields.iter().find_map(|v| match v {
        Value::SectionsValue { value, .. } => Some(value.clone()),
        _ => None,
    });
    Ok(Filed {
        text: file::clone_text(
            &shape,
            number,
            akt["state"]["key"].as_str().unwrap_or_default(),
            &values,
        ),
        title: akt["title"].as_str().map(str::to_owned),
        sections,
    })
}

/// Where each file of a container and its members goes (`clone.tree`): the
/// container's at the root, each member's in the directories its
/// `sections` field puts it in.
// r[impl clone.tree]
#[must_use]
pub fn tree_of(
    number: &str,
    text: &str,
    title: Option<&str>,
    sections: Option<&Json>,
    members: &[(String, String, Option<String>)],
) -> BTreeMap<String, String> {
    let mut tree = BTreeMap::new();
    tree.insert(file_name(number, title), text.to_owned());
    let mut dirs: HashMap<String, String> = HashMap::new();
    if let Some(items) = sections.and_then(Json::as_array) {
        place(items, "", &mut dirs);
    }
    for (n, text, title) in members {
        let dir = dirs.get(n).cloned().unwrap_or_default();
        tree.insert(
            format!("{dir}{}", file_name(n, title.as_deref())),
            text.clone(),
        );
    }
    tree
}

/// The directory each member of `items` goes in, under `at`: one per
/// section, named by its title, `section` without one, and `-2`, `-3`, …
/// for siblings named alike.
// r[impl clone.tree]
fn place(items: &[Json], at: &str, dirs: &mut HashMap<String, String>) {
    let mut taken: HashMap<String, usize> = HashMap::new();
    for item in items {
        match item {
            Json::String(n) => {
                dirs.insert(n.clone(), at.to_owned());
            }
            Json::Object(section) => {
                let base = section
                    .get("title")
                    .and_then(Json::as_str)
                    .map(slug)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "section".to_owned());
                let count = taken.entry(base.clone()).or_insert(0);
                *count += 1;
                let name = if *count == 1 {
                    base
                } else {
                    format!("{base}-{count}")
                };
                if let Some(inner) = section.get("items").and_then(Json::as_array) {
                    place(inner, &format!("{at}{name}/"), dirs);
                }
            }
            _ => {}
        }
    }
}

/// What a file says it is, and its values in the file's form.
pub fn read_file(text: &str) -> Result<(String, Parsed), CliError> {
    let parsed = file::parse(text)?;
    let number = parsed
        .text("number")
        .ok_or_else(|| CliError::refused("BAD_FILE", "a file names no `number`"))?;
    Ok((number, parsed))
}

/// The number a clone's file names, if it is an akt file.
#[must_use]
pub fn number_of_file(text: &str) -> Option<String> {
    read_file(text).ok().map(|(number, _)| number)
}

/// `text`, an akt's file in a clone, in normal form (`clone.normal`): as
/// Kuria would write it, every heading with an id.
// r[impl clone.normal]
pub fn normal(shape: &Shape, text: &str) -> Result<String, CliError> {
    let (number, parsed) = read_file(text)?;
    let mut values = parsed.values()?;
    for field in &shape.fields {
        if field.kind == "SECTIONS"
            && let Some(Json::String(body)) = values.get(&field.key)
        {
            let read = crate::values::sections_from_text(body)
                .map_err(|e| CliError::refused("BAD_FILE", e))?;
            values.insert(field.key.clone(), read);
        }
        if field.kind == "MARKDOWN"
            && let Some(Json::String(body)) = values.get(&field.key)
        {
            let mut given = body.clone();
            kuria_engine::markdown::give_ids(&mut [&mut given])
                .map_err(|e| CliError::refused("BAD_FILE", e))?;
            values.insert(field.key.clone(), Json::String(given));
        }
    }
    let state = parsed.text("state").unwrap_or_default();
    Ok(file::clone_text(shape, &number, &state, &values))
}

/// The title a clone's file names it by.
#[must_use]
pub fn title_in(shape: &Shape, parsed: &Parsed) -> Option<String> {
    let key = shape.presentation.title.clone().or_else(|| {
        shape
            .fields
            .iter()
            .find(|f| f.kind == "TEXT" && f.required && !f.multiline)
            .map(|f| f.key.clone())
    })?;
    parsed.text(&key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // r[verify clone.tree]
    #[test]
    fn places_members_where_the_sections_put_them() {
        let sections = json!([
            { "title": "Guides", "items": ["D-2", { "title": "Deep", "items": ["D-3"] }] },
            { "items": ["D-4"] },
            { "title": "Guides", "items": ["D-5"] }
        ]);
        let member = |n: &str| (n.to_owned(), format!("{n}\n"), Some("Doc".to_owned()));
        let tree = tree_of(
            "D-1",
            "index\n",
            Some("Index"),
            Some(&sections),
            &[
                member("D-2"),
                member("D-3"),
                member("D-4"),
                member("D-5"),
                member("D-6"),
            ],
        );
        let paths: Vec<&str> = tree.keys().map(String::as_str).collect();
        assert_eq!(
            paths,
            [
                "D-1-index.md",
                "D-6-doc.md",
                "guides-2/D-5-doc.md",
                "guides/D-2-doc.md",
                "guides/deep/D-3-doc.md",
                "section/D-4-doc.md",
            ]
        );
    }

    fn document() -> Shape {
        serde_json::from_value(json!({
            "key": "document", "version": 1, "name": "Document", "numberPattern": "D-{seq}",
            "fields": [
                { "key": "title", "name": "Title", "kind": "TEXT", "description": null, "required": true, "multiline": false, "options": [], "format": null },
                { "key": "body", "name": "Body", "kind": "MARKDOWN", "description": null, "required": false, "multiline": true, "options": [], "format": null }
            ],
            "states": [{ "key": "draft", "name": "Draft", "initial": true, "final": false, "tone": "NEUTRAL" }],
            "transitions": [],
            "presentation": { "title": "title", "card": [], "list": [], "icon": null, "colour": "NEUTRAL" }
        }))
        .unwrap()
    }

    // r[verify clone.normal]
    #[test]
    fn puts_a_file_in_normal_form() {
        let shape = document();
        let written = "---\nnumber: D-2\ntype: document\nstate: draft\ntitle: Guide\n---\n\n<!-- field: body -->\n\n## Install\n\nRun it.\n";
        let normal = normal(&shape, written).unwrap();
        assert!(normal.contains("## Install {#install}"), "{normal}");
        assert_eq!(super::normal(&shape, &normal).unwrap(), normal);
        let (number, parsed) = read_file(&normal).unwrap();
        assert_eq!(number, "D-2");
        assert_eq!(title_in(&shape, &parsed).as_deref(), Some("Guide"));
        assert!(read_file("no front matter\n").is_err());
    }
}
