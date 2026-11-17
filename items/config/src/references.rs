//! States and fields one akt type names of another, and whether they still
//! exist.

use crate::FieldKind;
use crate::validate::{AktType, Problem};

/// A state of an akt type, named by another akt type's document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateReference {
    /// Where the naming document names it, such as
    /// `board.columns[1].states.story[0]`.
    pub path: String,
    pub akt_type: String,
    pub state: String,
}

impl AktType {
    /// Every state of an akt type this one names: in its board's columns,
    /// and in its fields' draft recipes.
    #[must_use]
    pub fn state_references(&self) -> Vec<StateReference> {
        let mut references = Vec::new();
        if let Some(board) = self.board() {
            for (i, column) in board.columns.iter().enumerate() {
                for (akt_type, states) in &column.states {
                    for (j, state) in states.iter().enumerate() {
                        references.push(StateReference {
                            path: format!("board.columns[{i}].states.{akt_type}[{j}]"),
                            akt_type: akt_type.clone(),
                            state: state.clone(),
                        });
                    }
                }
            }
        }
        for (i, field) in self.document().fields.iter().enumerate() {
            let Some(draft) = &field.draft else { continue };
            for (akt_type, states) in &draft.states {
                for (j, state) in states.iter().enumerate() {
                    references.push(StateReference {
                        path: format!("fields[{i}].draft.states.{akt_type}[{j}]"),
                        akt_type: akt_type.clone(),
                        state: state.clone(),
                    });
                }
            }
        }
        references
    }
}

/// The problems with defining `new` next to `others`, the latest versions of
/// the tenant's other akt types: states `new` names that the named akt type
/// doesn't have, and states `new` lacks that another names for it.
// r[impl akt.type.state-references]
#[must_use]
pub fn check_state_references(new: &AktType, others: &[AktType]) -> Vec<Problem> {
    let mut problems = Vec::new();
    for reference in new.state_references() {
        let named = if reference.akt_type == new.key() {
            Some(new)
        } else {
            others.iter().find(|o| o.key() == reference.akt_type)
        };
        if let Some(named) = named
            && named.state(&reference.state).is_none()
        {
            problems.push(Problem {
                path: reference.path,
                message: format!(
                    "`{}` has no state `{}`",
                    reference.akt_type, reference.state
                ),
            });
        }
    }
    for other in others.iter().filter(|o| o.key() != new.key()) {
        for reference in other.state_references() {
            if reference.akt_type == new.key() && new.state(&reference.state).is_none() {
                problems.push(Problem {
                    path: "states".to_owned(),
                    message: format!(
                        "`{}` names the state `{}` at {}; stop naming it there first",
                        other.key(),
                        reference.state,
                        reference.path
                    ),
                });
            }
        }
    }
    problems
}

/// A field, and perhaps options of it, that a draft recipe names of the akt
/// types it lists.
struct FieldReference<'a> {
    path: String,
    types: &'a [String],
    field: &'a str,
    options: &'a [String],
}

impl AktType {
    /// The fields this akt type's draft recipes name of the akt types they
    /// list: `latest_by`, and `exclude` with its options.
    fn field_references(&self) -> Vec<FieldReference<'_>> {
        let mut references = Vec::new();
        for (i, field) in self.document().fields.iter().enumerate() {
            let Some(draft) = &field.draft else { continue };
            if let Some(key) = &draft.latest_by {
                references.push(FieldReference {
                    path: format!("fields[{i}].draft.latest_by"),
                    types: &draft.types,
                    field: key,
                    options: &[],
                });
            }
            for (key, options) in &draft.exclude {
                references.push(FieldReference {
                    path: format!("fields[{i}].draft.exclude.{key}"),
                    types: &draft.types,
                    field: key,
                    options,
                });
            }
        }
        references
    }
}

/// Whether `reference` holds among `defined`, the listed akt types that are
/// defined: some has the field, and for each option some has it there. A
/// reference to akt types none of which is defined yet holds.
fn holds(reference: &FieldReference<'_>, defined: &[&AktType]) -> Result<(), String> {
    if defined.is_empty() {
        return Ok(());
    }
    let fields: Vec<_> = defined
        .iter()
        .filter_map(|ty| ty.field(reference.field))
        .collect();
    if fields.is_empty() {
        return Err(format!(
            "no akt type it lists has a field `{}`",
            reference.field
        ));
    }
    for option in reference.options {
        let has = fields.iter().any(|f| {
            matches!(f.kind, FieldKind::Choice | FieldKind::Choices)
                && f.options.iter().any(|o| &o.key == option)
        });
        if !has {
            return Err(format!(
                "no akt type it lists has the option `{option}` in a choice field `{}`",
                reference.field
            ));
        }
    }
    Ok(())
}

/// The problems with defining `new` next to `others`, the latest versions of
/// the tenant's other akt types, in what they number from: another with
/// `new`'s pattern that doesn't name `new`'s sequence, or one naming its
/// sequence with another pattern.
// r[impl akt.number.shared]
#[must_use]
pub fn check_sequences(new: &AktType, others: &[AktType]) -> Vec<Problem> {
    let mine = new.document();
    others
        .iter()
        .filter(|other| other.key() != new.key())
        .filter_map(|other| {
            let theirs = other.document();
            let same_pattern = theirs.number == mine.number;
            let same_sequence = mine.sequence.is_some() && theirs.sequence == mine.sequence;
            let message = if same_pattern && !same_sequence {
                format!(
                    "`{}` is numbered `{}` too: both must name the same `sequence` to share its numbers",
                    other.key(),
                    mine.number
                )
            } else if same_sequence && !same_pattern {
                format!(
                    "`{}` numbers from the sequence `{}` as `{}`: akt types sharing a sequence must be numbered alike",
                    other.key(),
                    theirs.sequence.as_deref().unwrap_or_default(),
                    theirs.number
                )
            } else {
                return None;
            };
            let path = if same_pattern { "number" } else { "sequence" };
            Some(Problem {
                path: path.to_owned(),
                message,
            })
        })
        .collect()
}

/// The problems with defining `new` next to `others`, the latest versions of
/// the tenant's other akt types: fields and options `new`'s draft recipes
/// name that none of the akt types they list has, and fields or options
/// `new` drops that another's recipe still needs of it.
// r[impl draft.latest]
#[must_use]
pub fn check_field_references(new: &AktType, others: &[AktType]) -> Vec<Problem> {
    let others: Vec<&AktType> = others.iter().filter(|o| o.key() != new.key()).collect();
    let all: Vec<&AktType> = std::iter::once(new).chain(others.iter().copied()).collect();
    let defined = |types: &[String]| -> Vec<&AktType> {
        all.iter()
            .copied()
            .filter(|ty| types.iter().any(|t| t == ty.key()))
            .collect()
    };
    let mut problems = Vec::new();
    for reference in new.field_references() {
        if let Err(message) = holds(&reference, &defined(reference.types)) {
            problems.push(Problem {
                path: reference.path,
                message,
            });
        }
    }
    for other in &others {
        for reference in other.field_references() {
            if !reference.types.iter().any(|t| t == new.key()) {
                continue;
            }
            if let Err(message) = holds(&reference, &defined(reference.types)) {
                problems.push(Problem {
                    path: "fields".to_owned(),
                    message: format!(
                        "`{}` needs it at {}: {message}; stop naming it there first",
                        other.key(),
                        reference.path
                    ),
                });
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{AktTypeDocument, validate};

    fn ty(document: serde_json::Value) -> AktType {
        validate(serde_json::from_value::<AktTypeDocument>(document).unwrap()).unwrap()
    }

    fn story(states: &[&str]) -> AktType {
        let states: Vec<_> = states
            .iter()
            .enumerate()
            .map(|(i, s)| json!({ "key": s, "name": s, "initial": i == 0 }))
            .collect();
        ty(json!({ "key": "story", "name": "Story", "number": "S-{seq}", "states": states }))
    }

    fn epic() -> AktType {
        ty(json!({
            "key": "epic", "name": "Epic", "number": "E-{seq}",
            "states": [{ "key": "open", "name": "Open", "initial": true }],
            "contains": [{ "type": "story" }, { "type": "bug" }],
            "board": { "columns": [
                { "key": "todo", "name": "To do", "states": { "story": ["open"], "bug": ["new"] } },
                { "key": "doing", "name": "Doing", "states": { "story": ["doing"] } }
            ] }
        }))
    }

    // r[verify akt.type.state-references]
    #[test]
    fn a_board_must_name_states_its_member_types_have() {
        assert_eq!(
            check_state_references(&epic(), &[story(&["open", "doing"])]),
            vec![]
        );
        // `bug` isn't defined yet, which is fine.
        let problems = check_state_references(&epic(), &[story(&["open", "in_progress"])]);
        assert_eq!(
            problems,
            vec![Problem {
                path: "board.columns[1].states.story[0]".to_owned(),
                message: "`story` has no state `doing`".to_owned(),
            }]
        );
    }

    // r[verify akt.type.state-references]
    #[test]
    fn a_member_type_cant_drop_a_state_a_board_names() {
        let problems = check_state_references(&story(&["open", "in_progress"]), &[epic()]);
        assert_eq!(
            problems,
            vec![Problem {
                path: "states".to_owned(),
                message: "`epic` names the state `doing` at board.columns[1].states.story[0]; stop naming it there first".to_owned(),
            }]
        );
        assert_eq!(
            check_state_references(&story(&["open", "doing", "done"]), &[epic()]),
            vec![]
        );
    }

    fn artifact(options: &[&str]) -> AktType {
        let options: Vec<_> = options
            .iter()
            .map(|o| json!({ "key": o, "name": o }))
            .collect();
        ty(json!({
            "key": "artifact", "name": "Artifact", "number": "A-{seq}",
            "states": [{ "key": "built", "name": "Built", "initial": true }],
            "fields": [
                { "key": "component", "name": "Component", "kind": "text" },
                { "key": "change", "name": "Change", "kind": "choice", "options": options }
            ]
        }))
    }

    fn manifest(latest_by: &str, exclude: &str) -> AktType {
        ty(json!({
            "key": "notes", "name": "Notes", "number": "N-{seq}",
            "states": [{ "key": "draft", "name": "Draft", "initial": true }],
            "fields": [{ "key": "text", "name": "Text", "kind": "text", "multiline": true,
                         "draft": { "types": ["artifact", "image"], "line": "{number}",
                                    "latest_by": latest_by,
                                    "exclude": { "change": [exclude] } } }]
        }))
    }

    // r[verify draft.latest]
    #[test]
    fn a_recipe_names_fields_and_options_its_listed_types_have() {
        let built = artifact(&["built", "removed"]);
        assert_eq!(
            check_field_references(&manifest("component", "removed"), &[]),
            vec![]
        );
        assert_eq!(
            check_field_references(
                &manifest("component", "removed"),
                std::slice::from_ref(&built)
            ),
            vec![]
        );
        let problems: Vec<_> =
            check_field_references(&manifest("name", "gone"), std::slice::from_ref(&built))
                .into_iter()
                .map(|p| format!("{}: {}", p.path, p.message))
                .collect();
        assert_eq!(
            problems,
            vec![
                "fields[0].draft.latest_by: no akt type it lists has a field `name`",
                "fields[0].draft.exclude.change: no akt type it lists has the option `gone` in a choice field `change`",
            ]
        );
    }

    // r[verify draft.latest]
    #[test]
    fn a_listed_type_cant_drop_what_a_recipe_needs() {
        let notes = manifest("component", "removed");
        let problems = check_field_references(&artifact(&["built"]), std::slice::from_ref(&notes));
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].path, "fields");
        assert!(
            problems[0]
                .message
                .contains("`notes` needs it at fields[0].draft.exclude.change")
        );
        assert_eq!(
            check_field_references(&artifact(&["built", "removed"]), &[notes]),
            vec![]
        );
    }

    // r[verify akt.type.state-references]
    #[test]
    fn draft_states_are_references_too() {
        let notes = ty(json!({
            "key": "notes", "name": "Notes", "number": "N-{seq}",
            "states": [{ "key": "draft", "name": "Draft", "initial": true }],
            "fields": [{ "key": "text", "name": "Text", "kind": "text", "multiline": true,
                         "draft": { "types": ["story"], "line": "{number}",
                                    "states": { "story": ["shipped"] } } }]
        }));
        let problems = check_state_references(&notes, &[story(&["open"])]);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].path, "fields[0].draft.states.story[0]");
    }

    fn numbered(key: &str, number: &str, sequence: Option<&str>) -> AktType {
        let mut document = serde_json::json!({
            "key": key, "name": key, "number": number,
            "fields": [], "states": [{ "key": "open", "name": "Open", "initial": true }]
        });
        if let Some(sequence) = sequence {
            document["sequence"] = serde_json::json!(sequence);
        }
        crate::validate(serde_json::from_value(document).unwrap()).unwrap()
    }

    // r[verify akt.number.shared]
    #[test]
    fn akt_types_share_numbers_only_by_naming_one_sequence() {
        let policy = numbered("policy", "QMS-{seq:4}", Some("qms"));
        let procedure = numbered("procedure", "QMS-{seq:4}", Some("qms"));
        assert_eq!(
            check_sequences(&procedure, std::slice::from_ref(&policy)),
            []
        );
        // A new version of the same type is no other.
        assert_eq!(check_sequences(&policy, std::slice::from_ref(&policy)), []);

        let unnamed = numbered("procedure", "QMS-{seq:4}", None);
        let problems = check_sequences(&unnamed, std::slice::from_ref(&policy));
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].path, "number");
        assert!(problems[0].message.contains("`policy`"), "{problems:?}");
        let neither = numbered("other", "QMS-{seq:4}", None);
        assert_eq!(check_sequences(&neither, &[unnamed]).len(), 1);
        assert_eq!(
            check_sequences(
                &numbered("p", "QMS-{seq:4}", Some("other")),
                std::slice::from_ref(&policy)
            )
            .len(),
            1
        );

        let elsewhere = numbered("form", "FORM-{seq:4}", Some("qms"));
        let problems = check_sequences(&elsewhere, &[policy]);
        assert_eq!(problems[0].path, "sequence");
        assert!(problems[0].message.contains("QMS-{seq:4}"), "{problems:?}");
    }
}
