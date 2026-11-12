//! Placing a container's members in its board's columns.

use kuria_config::BoardDef;

/// The key of the column for members no configured column names.
pub const UNMAPPED: &str = "unmapped";

/// The key of the column a member of the akt type `akt_type` in `state`
/// falls in, or [`UNMAPPED`].
// r[impl board.placement]
// r[impl board.unmapped]
// r[impl board.default]
#[must_use]
pub fn column_of<'a>(board: Option<&'a BoardDef>, akt_type: &str, state: &str) -> &'a str {
    board
        .and_then(|board| {
            board.columns.iter().find(|column| {
                column
                    .states
                    .get(akt_type)
                    .is_some_and(|states| states.iter().any(|s| s == state))
            })
        })
        .map_or(UNMAPPED, |column| column.key.as_str())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn board() -> BoardDef {
        serde_json::from_value(json!({ "columns": [
            { "key": "todo", "name": "To do", "states": { "story": ["open"], "bug": ["new", "triaged"] } },
            { "key": "done", "name": "Done", "states": { "story": ["done"], "bug": ["fixed"] } }
        ] }))
        .unwrap()
    }

    // r[verify board.placement]
    #[test]
    fn places_each_type_by_its_own_states() {
        let board = board();
        assert_eq!(column_of(Some(&board), "story", "open"), "todo");
        assert_eq!(column_of(Some(&board), "bug", "triaged"), "todo");
        assert_eq!(column_of(Some(&board), "bug", "fixed"), "done");
    }

    // r[verify board.unmapped]
    // r[verify board.default]
    #[test]
    fn what_no_column_names_is_unmapped() {
        let board = board();
        assert_eq!(column_of(Some(&board), "story", "blocked"), UNMAPPED);
        assert_eq!(column_of(Some(&board), "epic", "open"), UNMAPPED);
        assert_eq!(column_of(None, "story", "open"), UNMAPPED);
    }
}
