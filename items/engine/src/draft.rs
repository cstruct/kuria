//! Rendering a field's draft from the akter its recipe lists.

use std::collections::BTreeMap;

use kuria_config::{DraftDef, GroupBy, LineTemplate, LineValues};
use serde_json::Value as Json;

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

/// Of `found`, what the recipe lists by `latest_by` and `exclude`: with
/// `latest_by`, of the akter with the same value in that field only the one
/// `newness` ranks highest, those without a value each; then without the
/// akter whose `exclude`d field holds one of its options. `value` gives an
/// akt's stored value of a field, if it has one. What is kept keeps its
/// order.
// r[impl draft.latest]
pub fn select<T, N: Ord>(
    recipe: &DraftDef,
    found: Vec<T>,
    newness: impl Fn(&T) -> N,
    value: impl Fn(&T, &str) -> Option<Json>,
) -> Vec<T> {
    let mut kept: Vec<bool> = vec![true; found.len()];
    if let Some(key) = &recipe.latest_by {
        // Per value, the index of the newest akt holding it.
        let mut newest: Vec<(Json, usize)> = Vec::new();
        for (i, akt) in found.iter().enumerate() {
            let Some(held) = value(akt, key).filter(|v| !v.is_null()) else {
                continue;
            };
            match newest.iter_mut().find(|(v, _)| *v == held) {
                Some((_, best)) => {
                    if newness(akt) > newness(&found[*best]) {
                        kept[*best] = false;
                        *best = i;
                    } else {
                        kept[i] = false;
                    }
                }
                None => newest.push((held, i)),
            }
        }
    }
    let excluded = |akt: &T| {
        recipe.exclude.iter().any(|(key, options)| {
            let holds = |v: &Json| v.as_str().is_some_and(|k| options.iter().any(|o| o == k));
            match value(akt, key) {
                Some(Json::Array(keys)) => keys.iter().any(holds),
                Some(v) => holds(&v),
                None => false,
            }
        })
    };
    found
        .into_iter()
        .zip(kept)
        .filter(|(akt, kept)| *kept && !excluded(akt))
        .map(|(akt, _)| akt)
        .collect()
}

#[cfg(test)]
mod tests {
    // r[verify draft.latest]
    #[test]
    fn lists_the_newest_per_value_unless_excluded() {
        let recipe: DraftDef = serde_json::from_value(json!({
            "types": ["artifact"], "line": "{number}",
            "latest_by": "component", "exclude": { "change": ["removed"] }
        }))
        .unwrap();
        // (number, created, component, change)
        let found = vec![
            ("A-1", 1, Some("backend"), "built"),
            ("A-2", 2, Some("frontend"), "built"),
            ("A-3", 5, Some("backend"), "built"),
            ("A-4", 4, Some("docs"), "built"),
            ("A-5", 6, Some("docs"), "removed"),
            ("A-6", 3, None, "built"),
            ("A-7", 7, None, "built"),
            ("A-8", 0, Some("backend"), "built"),
        ];
        let kept = select(
            &recipe,
            found,
            |a| a.1,
            |a, key| match key {
                "component" => a.2.map(|c| json!(c)),
                "change" => Some(json!(a.3)),
                _ => None,
            },
        );
        let numbers: Vec<_> = kept.iter().map(|a| a.0).collect();
        assert_eq!(numbers, vec!["A-2", "A-3", "A-6", "A-7"]);
    }

    // r[verify draft.latest]
    #[test]
    fn excludes_by_any_of_several_options() {
        let recipe: DraftDef = serde_json::from_value(json!({
            "types": ["item"], "line": "{number}", "exclude": { "tags": ["internal"] }
        }))
        .unwrap();
        let found = vec![("I-1", json!(["internal", "ui"])), ("I-2", json!(["ui"]))];
        let kept = select(&recipe, found, |_| 0, |a, _| Some(a.1.clone()));
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].0, "I-2");
    }

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
