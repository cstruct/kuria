//! States one akt type names of another, and whether they still exist.

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
}
