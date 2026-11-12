//! Rendering a field's draft from the akter its recipe lists.

use std::collections::BTreeMap;

use kuria_config::{DraftDef, GroupBy, LineTemplate, LineValues};

/// One akt a draft lists, with what its line may show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub number: String,
    /// Its akt type's key, for grouping and filtering.
    pub akt_type: String,
    /// Its akt type's name, for headings and `{type}`.
    pub type_name: String,
    /// Its state's name, for `{state}`.
    pub state: String,
    /// Its field values as text, by key, for `{field:<key>}`.
    pub fields: BTreeMap<String, String>,
}

/// The draft `recipe` gives for `listed`, which must be in rank order and
/// hold each akt once.
///
/// # Panics
///
/// If the recipe's line doesn't parse, which validation rules out.
// r[impl draft.render]
#[must_use]
pub fn render(recipe: &DraftDef, listed: &[Listed]) -> String {
    let line = LineTemplate::parse(&recipe.line).expect("a validated recipe");
    let render = |akt: &Listed| {
        let field = |key: &str| akt.fields.get(key).cloned().unwrap_or_default();
        line.render(&LineValues {
            number: &akt.number,
            akt_type: &akt.type_name,
            state: &akt.state,
            field: &field,
        })
    };
    match recipe.group_by {
        None => listed
            .iter()
            .filter(|a| recipe.types.contains(&a.akt_type))
            .map(render)
            .collect::<Vec<_>>()
            .join("\n"),
        Some(GroupBy::Type) => recipe
            .types
            .iter()
            .filter_map(|key| {
                let group: Vec<&Listed> = listed.iter().filter(|a| &a.akt_type == key).collect();
                let name = &group.first()?.type_name;
                let lines: Vec<String> = group.into_iter().map(render).collect();
                Some(format!("## {name}\n{}", lines.join("\n")))
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn listed(number: &str, akt_type: &str, title: &str) -> Listed {
        Listed {
            number: number.to_owned(),
            akt_type: akt_type.to_owned(),
            type_name: if akt_type == "epic" { "Epics" } else { "Items" }.to_owned(),
            state: "Done".to_owned(),
            fields: BTreeMap::from([("title".to_owned(), title.to_owned())]),
        }
    }

    fn recipe(group: bool) -> DraftDef {
        let mut recipe =
            json!({ "types": ["epic", "item"], "line": "{number} {field:title}{field:none}" });
        if group {
            recipe["group_by"] = json!("type");
        }
        serde_json::from_value(recipe).unwrap()
    }

    // r[verify draft.render]
    #[test]
    fn groups_by_type_in_the_recipes_order() {
        let listed = [
            listed("ITEM-2", "item", "Crash on save"),
            listed("EPIC-1", "epic", "Login"),
            listed("ITEM-1", "item", "Typo"),
            listed("BUG-1", "bug", "Not listed"),
        ];
        assert_eq!(
            render(&recipe(true), &listed),
            "## Epics\nEPIC-1 Login\n\n## Items\nITEM-2 Crash on save\nITEM-1 Typo"
        );
        assert_eq!(
            render(&recipe(false), &listed),
            "ITEM-2 Crash on save\nEPIC-1 Login\nITEM-1 Typo"
        );
    }

    // r[verify draft.render]
    #[test]
    fn leaves_out_empty_groups() {
        let listed = [listed("ITEM-1", "item", "Typo")];
        assert_eq!(render(&recipe(true), &listed), "## Items\nITEM-1 Typo");
        assert_eq!(render(&recipe(true), &[]), "");
    }
}
