//! Validating an akt type document.

use std::collections::BTreeSet;

use std::collections::BTreeMap;

use crate::document::{
    AktTypeDocument, BoardDef, ContainsDef, FieldDef, FieldKind, StateDef, TransitionDef,
};
use crate::draft::{DraftDef, LineTemplate, MAX_DEPTH};
use crate::number::NumberPattern;

/// A validated akt type: its document, which is what is stored, and its
/// parsed number pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AktType {
    document: AktTypeDocument,
    number: NumberPattern,
}

/// One reason a document was refused: where, as a path such as
/// `fields[2].options`, and what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl AktType {
    #[must_use]
    pub fn document(&self) -> &AktTypeDocument {
        &self.document
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.document.key
    }

    #[must_use]
    pub fn number(&self) -> &NumberPattern {
        &self.number
    }

    #[must_use]
    pub fn field(&self, key: &str) -> Option<&FieldDef> {
        self.document.fields.iter().find(|f| f.key == key)
    }

    #[must_use]
    pub fn state(&self, key: &str) -> Option<&StateDef> {
        self.document.states.iter().find(|s| s.key == key)
    }

    #[must_use]
    pub fn transition(&self, key: &str) -> Option<&TransitionDef> {
        self.document.transitions.iter().find(|t| t.key == key)
    }

    /// How this type contains akter of the type `key`, if it does.
    #[must_use]
    pub fn contains(&self, key: &str) -> Option<&ContainsDef> {
        self.document.contains.iter().find(|c| c.akt_type == key)
    }

    /// The akt types an akt of this type must be created in; empty if it can
    /// be created anywhere.
    #[must_use]
    pub fn created_in(&self) -> &[String] {
        &self.document.created_in
    }

    /// The board's configuration, if it has one.
    #[must_use]
    pub fn board(&self) -> Option<&BoardDef> {
        self.document.board.as_ref()
    }

    /// The state akter of this type are created in.
    ///
    /// # Panics
    ///
    /// Never: validation guarantees exactly one initial state.
    #[must_use]
    pub fn initial_state(&self) -> &StateDef {
        self.document
            .states
            .iter()
            .find(|s| s.initial)
            .expect("a validated akt type has an initial state")
    }
}

/// Validates `document`, returning every problem found.
// r[impl akt.type.validation]
pub fn validate(document: AktTypeDocument) -> Result<AktType, Vec<Problem>> {
    let mut problems = Problems::default();

    problems.key("key", &document.key);
    problems.name("name", &document.name);
    let number = NumberPattern::parse(&document.number)
        .map_err(|e| problems.add("number", e.to_string()))
        .ok();

    let fields = fields(&mut problems, &document);
    let states = problems.unique_keys(
        "states",
        document.states.iter().map(|s| s.key.as_str()),
        "key",
    );
    for (i, state) in document.states.iter().enumerate() {
        problems.name(&format!("states[{i}].name"), &state.name);
    }
    let initial = document.states.iter().filter(|s| s.initial).count();
    if initial != 1 {
        problems.add(
            "states",
            format!("exactly one state must be initial, not {initial}"),
        );
    }

    transitions(&mut problems, &document, &fields, &states);

    let contained = containment(&mut problems, &document);
    if let Some(board) = &document.board {
        board_columns(&mut problems, board, &contained);
    }
    // r[impl akt.number.pattern]
    if number.as_ref().is_some_and(NumberPattern::uses_parent) && document.created_in.is_empty() {
        problems.add(
            "number",
            "{parent} needs `created_in`, naming what the akt is created in",
        );
    }

    match (problems.0.is_empty(), number) {
        (true, Some(number)) => Ok(AktType { document, number }),
        _ => Err(problems.0),
    }
}

/// Checks the fields, and returns their keys.
fn fields<'a>(problems: &mut Problems, document: &'a AktTypeDocument) -> BTreeSet<&'a str> {
    let keys = problems.unique_keys(
        "fields",
        document.fields.iter().map(|f| f.key.as_str()),
        "key",
    );
    for (i, field) in document.fields.iter().enumerate() {
        let path = format!("fields[{i}]");
        problems.name(&format!("{path}.name"), &field.name);
        if field.kind.has_options() {
            if field.options.is_empty() {
                problems.add(
                    &format!("{path}.options"),
                    "a choice field needs at least one option",
                );
            }
            problems.unique_keys(
                &format!("{path}.options"),
                field.options.iter().map(|o| o.key.as_str()),
                "key",
            );
            for (j, option) in field.options.iter().enumerate() {
                problems.name(&format!("{path}.options[{j}].name"), &option.name);
            }
        } else if !field.options.is_empty() {
            problems.add(
                &format!("{path}.options"),
                "only choice fields have options",
            );
        }
        if let Some(draft) = &field.draft {
            draft_recipe(problems, &format!("{path}.draft"), field, draft);
        }
        if field.multiline && field.kind != FieldKind::Text {
            problems.add(
                &format!("{path}.multiline"),
                "only text fields are multiline",
            );
        }
    }
    keys
}

/// Checks the transitions against the type's fields and states.
fn transitions(
    problems: &mut Problems,
    document: &AktTypeDocument,
    fields: &BTreeSet<&str>,
    states: &BTreeSet<&str>,
) {
    problems.unique_keys(
        "transitions",
        document.transitions.iter().map(|t| t.key.as_str()),
        "key",
    );
    for (i, transition) in document.transitions.iter().enumerate() {
        let path = format!("transitions[{i}]");
        problems.name(&format!("{path}.name"), &transition.name);
        if transition.from.is_empty() {
            problems.add(
                &format!("{path}.from"),
                "a transition starts from at least one state",
            );
        }
        for (j, from) in transition.from.iter().enumerate() {
            match document.states.iter().find(|s| &s.key == from) {
                None => problems.add(&format!("{path}.from[{j}]"), format!("no state `{from}`")),
                Some(state) if state.is_final => problems.add(
                    &format!("{path}.from[{j}]"),
                    format!("`{from}` is final, and nothing moves on from a final state"),
                ),
                Some(_) => {}
            }
        }
        if !states.contains(transition.to.as_str()) {
            problems.add(
                &format!("{path}.to"),
                format!("no state `{}`", transition.to),
            );
        }
        // r[impl authz.transition-action]
        if let Some(action) = &transition.action
            && let Err(message) = check_action(action)
        {
            problems.add(&format!("{path}.action"), message);
        }
        for (j, required) in transition.requires.iter().enumerate() {
            if !fields.contains(required.as_str()) {
                problems.add(
                    &format!("{path}.requires[{j}]"),
                    format!("no field `{required}`"),
                );
            }
        }
    }
}

/// Checks a field's draft recipe.
// r[impl draft.recipe]
fn draft_recipe(problems: &mut Problems, path: &str, field: &FieldDef, draft: &DraftDef) {
    if field.kind != FieldKind::Text || !field.multiline {
        problems.add(path, "only multiline text fields can be drafted");
    }
    if !(1..=MAX_DEPTH).contains(&draft.depth) {
        problems.add(
            &format!("{path}.depth"),
            format!("a depth is from 1 to {MAX_DEPTH}"),
        );
    }
    if draft.types.is_empty() {
        problems.add(
            &format!("{path}.types"),
            "a draft lists at least one akt type",
        );
    }
    problems.unique_keys(
        &format!("{path}.types"),
        draft.types.iter().map(String::as_str),
        "",
    );
    problems.unique_keys(
        &format!("{path}.through"),
        draft.through.iter().map(String::as_str),
        "",
    );
    if let Err(message) = LineTemplate::parse(&draft.line) {
        problems.add(&format!("{path}.line"), message);
    }
    for (akt_type, states) in &draft.states {
        let at = format!("{path}.states.{akt_type}");
        problems.key(&at, akt_type);
        problems.unique_keys(&at, states.iter().map(String::as_str), "");
    }
}

/// Checks `contains` and `created_in`, and returns the contained akt types'
/// keys.
// r[impl container.contains]
// r[impl container.created-in]
fn containment<'a>(problems: &mut Problems, document: &'a AktTypeDocument) -> BTreeSet<&'a str> {
    let contained = problems.unique_keys(
        "contains",
        document.contains.iter().map(|c| c.akt_type.as_str()),
        "type",
    );
    // r[impl container.while]
    for (i, contains) in document.contains.iter().enumerate() {
        for (j, state) in contains.accepting.iter().enumerate() {
            if !document.states.iter().any(|s| &s.key == state) {
                problems.add(
                    &format!("contains[{i}].while[{j}]"),
                    format!("no state `{state}`"),
                );
            }
        }
    }
    problems.unique_keys(
        "created_in",
        document.created_in.iter().map(String::as_str),
        "",
    );
    contained
}

/// Checks a board's columns against the akt types the type contains.
// r[impl board.columns]
fn board_columns(problems: &mut Problems, board: &BoardDef, contained: &BTreeSet<&str>) {
    problems.unique_keys(
        "board.columns",
        board.columns.iter().map(|c| c.key.as_str()),
        "key",
    );
    // Which column each state of each akt type already fell in.
    let mut placed: BTreeMap<(&str, &str), &str> = BTreeMap::new();
    for (i, column) in board.columns.iter().enumerate() {
        let path = format!("board.columns[{i}]");
        if column.key == "unmapped" {
            problems.add(
                &format!("{path}.key"),
                "`unmapped` is the column for members no column names",
            );
        }
        problems.name(&format!("{path}.name"), &column.name);
        for (akt_type, states) in &column.states {
            let path = format!("{path}.states.{akt_type}");
            if !contained.contains(akt_type.as_str()) {
                problems.add(&path, format!("the akt type doesn't contain `{akt_type}`"));
            }
            for (j, state) in states.iter().enumerate() {
                problems.key(&format!("{path}[{j}]"), state);
                if let Some(other) = placed.insert((akt_type, state), &column.key) {
                    problems.add(
                        &format!("{path}[{j}]"),
                        format!("`{akt_type}.{state}` is already in the column `{other}`"),
                    );
                }
            }
        }
    }
}

/// Whether `action` can name a Konklav action: colon-delimited parts of
/// lowercase letters, digits, `_` and `-`, outside the `konklav:` namespace.
fn check_action(action: &str) -> Result<(), String> {
    let parts: Vec<&str> = action.split(':').collect();
    let well_formed = parts.len() >= 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        });
    if !well_formed {
        return Err(format!(
            "`{action}` must be colon-delimited lowercase parts, such as `capa:approve_closure`"
        ));
    }
    if parts[0] == "konklav" {
        return Err(format!(
            "`{action}` is in the `konklav:` namespace, which is Konklav's own"
        ));
    }
    Ok(())
}

#[derive(Default)]
struct Problems(Vec<Problem>);

impl Problems {
    fn add(&mut self, path: &str, message: impl Into<String>) {
        self.0.push(Problem {
            path: path.to_owned(),
            message: message.into(),
        });
    }

    fn key(&mut self, path: &str, key: &str) {
        let mut chars = key.chars();
        let valid = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && key.len() <= 64;
        if !valid {
            self.add(
                path,
                format!("`{key}` must be up to 64 lowercase letters, digits and `_`, starting with a letter"),
            );
        }
    }

    fn name(&mut self, path: &str, name: &str) {
        if name.trim().is_empty() {
            self.add(path, "a name can't be empty");
        }
    }

    /// Checks each key and that none repeats, and returns them. Each key's
    /// path is `path[i].member`, or `path[i]` when `member` is empty.
    fn unique_keys<'a>(
        &mut self,
        path: &str,
        keys: impl Iterator<Item = &'a str>,
        member: &str,
    ) -> BTreeSet<&'a str> {
        let mut seen = BTreeSet::new();
        for (i, key) in keys.enumerate() {
            let at = if member.is_empty() {
                format!("{path}[{i}]")
            } else {
                format!("{path}[{i}].{member}")
            };
            self.key(&at, key);
            if !seen.insert(key) {
                self.add(&at, format!("`{key}` is used twice"));
            }
        }
        seen
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn capa() -> serde_json::Value {
        json!({
            "key": "capa",
            "name": "CAPA",
            "number": "CAPA-{year}-{seq:4}",
            "fields": [
                { "key": "title", "name": "Title", "kind": "text", "required": true },
                { "key": "severity", "name": "Severity", "kind": "choice",
                  "options": [{ "key": "major", "name": "Major" }, { "key": "minor", "name": "Minor" }] },
                { "key": "root_cause", "name": "Root cause", "kind": "text", "multiline": true }
            ],
            "states": [
                { "key": "open", "name": "Open", "initial": true },
                { "key": "closed", "name": "Closed", "final": true }
            ],
            "transitions": [
                { "key": "close", "name": "Close", "from": ["open"], "to": "closed",
                  "requires": ["root_cause"] }
            ]
        })
    }

    fn problems(document: serde_json::Value) -> Vec<String> {
        let document: AktTypeDocument = serde_json::from_value(document).unwrap();
        match validate(document) {
            Ok(_) => Vec::new(),
            Err(problems) => problems.iter().map(ToString::to_string).collect(),
        }
    }

    // r[verify akt.type.validation]
    #[test]
    fn accepts_a_valid_document() {
        assert_eq!(problems(capa()), Vec::<String>::new());
    }

    // r[verify akt.type.document]
    #[test]
    fn refuses_unknown_members() {
        let mut document = capa();
        document["colour"] = json!("red");
        assert!(serde_json::from_value::<AktTypeDocument>(document).is_err());
    }

    // r[verify akt.type.validation]
    #[test]
    fn refuses_bad_and_repeated_keys() {
        let mut document = capa();
        document["key"] = json!("Capa");
        document["fields"][1]["key"] = json!("title");
        assert_eq!(
            problems(document),
            vec![
                "key: `Capa` must be up to 64 lowercase letters, digits and `_`, starting with a letter",
                "fields[1].key: `title` is used twice",
            ]
        );
    }

    // r[verify akt.type.validation]
    #[test]
    fn needs_exactly_one_initial_state() {
        let mut document = capa();
        document["states"][1]["initial"] = json!(true);
        assert_eq!(
            problems(document),
            vec!["states: exactly one state must be initial, not 2"]
        );
    }

    // r[verify akt.type.validation]
    #[test]
    fn checks_what_transitions_name() {
        let mut document = capa();
        document["transitions"] = json!([
            { "key": "reopen", "name": "Reopen", "from": ["closed"], "to": "opened",
              "requires": ["owner"] },
            { "key": "nowhere", "name": "Nowhere", "from": [], "to": "open" }
        ]);
        assert_eq!(
            problems(document),
            vec![
                "transitions[0].from[0]: `closed` is final, and nothing moves on from a final state",
                "transitions[0].to: no state `opened`",
                "transitions[0].requires[0]: no field `owner`",
                "transitions[1].from: a transition starts from at least one state",
            ]
        );
    }

    // r[verify authz.transition-action]
    #[test]
    fn checks_the_action_a_transition_names() {
        let mut document = capa();
        document["transitions"][0]["action"] = json!("capa:approve_closure");
        assert_eq!(problems(document.clone()), Vec::<String>::new());
        for (action, message) in [
            ("approve", "must be colon-delimited lowercase parts"),
            ("capa:Approve", "must be colon-delimited lowercase parts"),
            ("capa::x", "must be colon-delimited lowercase parts"),
            ("konklav:control:all", "is in the `konklav:` namespace"),
        ] {
            document["transitions"][0]["action"] = json!(action);
            let found = problems(document.clone());
            assert_eq!(found.len(), 1, "{action}");
            assert!(found[0].starts_with("transitions[0].action: "), "{action}");
            assert!(found[0].contains(message), "{action}: {}", found[0]);
        }
    }

    // r[verify akt.type.validation]
    #[test]
    fn checks_options_and_the_number_pattern() {
        let mut document = capa();
        document["fields"][1]["options"] = json!([]);
        document["fields"][0]["options"] = json!([{ "key": "a", "name": "A" }]);
        document["number"] = json!("CAPA");
        assert_eq!(
            problems(document),
            vec![
                "number: the pattern needs exactly one {seq} or {seq:N}, not 0",
                "fields[0].options: only choice fields have options",
                "fields[1].options: a choice field needs at least one option",
            ]
        );
    }

    fn release_candidate() -> serde_json::Value {
        json!({
            "key": "release_candidate",
            "name": "Release candidate",
            "number": "{parent}-RC-{seq:4}",
            "created_in": ["release"],
            "contains": [{ "type": "epic" }, { "type": "backlog_item", "exclusive": true }],
            "states": [
                { "key": "testing", "name": "Testing", "initial": true },
                { "key": "accepted", "name": "Accepted", "final": true }
            ],
            "board": { "columns": [
                { "key": "todo", "name": "To do",
                  "states": { "backlog_item": ["open"], "epic": ["open"] } },
                { "key": "done", "name": "Done",
                  "states": { "backlog_item": ["done"], "epic": ["done"] } }
            ] }
        })
    }

    // r[verify container.contains]
    // r[verify container.created-in]
    // r[verify board.columns]
    #[test]
    fn accepts_a_container_with_a_board() {
        let document: AktTypeDocument = serde_json::from_value(release_candidate()).unwrap();
        let ty = validate(document).unwrap();
        assert!(ty.contains("backlog_item").unwrap().exclusive);
        assert!(!ty.contains("epic").unwrap().exclusive);
        assert!(ty.contains("release").is_none());
        assert_eq!(ty.created_in(), ["release".to_owned()]);
        assert_eq!(ty.board().unwrap().columns.len(), 2);
    }

    // r[verify container.contains]
    #[test]
    fn checks_what_a_type_contains() {
        let mut document = release_candidate();
        document["contains"] =
            json!([{ "type": "Epic" }, { "type": "story" }, { "type": "story" }]);
        document["board"] = json!(null);
        assert_eq!(
            problems(document.clone()),
            vec![
                "contains[0].type: `Epic` must be up to 64 lowercase letters, digits and `_`, starting with a letter",
                "contains[2].type: `story` is used twice",
            ]
        );
        document["contains"] = json!([{ "type": "story", "max": 1 }]);
        assert!(serde_json::from_value::<AktTypeDocument>(document).is_err());
    }

    // r[verify container.created-in]
    // r[verify akt.number.pattern]
    #[test]
    fn parent_numbers_need_created_in() {
        let mut document = release_candidate();
        document["created_in"] = json!([]);
        assert_eq!(
            problems(document.clone()),
            vec!["number: {parent} needs `created_in`, naming what the akt is created in"]
        );
        document["created_in"] = json!(["release", "release"]);
        assert_eq!(
            problems(document),
            vec!["created_in[1]: `release` is used twice"]
        );
    }

    // r[verify board.columns]
    #[test]
    fn checks_board_columns() {
        let mut document = release_candidate();
        document["board"] = json!({ "columns": [
            { "key": "unmapped", "name": "Other" },
            { "key": "todo", "name": " ",
              "states": { "bug": ["new"], "epic": ["open", "open"] } },
            { "key": "todo", "name": "Again", "states": { "epic": ["open"] } }
        ] });
        assert_eq!(
            problems(document),
            vec![
                "board.columns[2].key: `todo` is used twice",
                "board.columns[0].key: `unmapped` is the column for members no column names",
                "board.columns[1].name: a name can't be empty",
                "board.columns[1].states.bug: the akt type doesn't contain `bug`",
                "board.columns[1].states.epic[1]: `epic.open` is already in the column `todo`",
                "board.columns[2].states.epic[0]: `epic.open` is already in the column `todo`",
            ]
        );
    }

    fn notes() -> serde_json::Value {
        json!({
            "key": "release_notes",
            "name": "Release notes",
            "number": "{parent}-NOTES",
            "created_in": ["release"],
            "fields": [{ "key": "text", "name": "Text", "kind": "text", "multiline": true,
                         "draft": { "through": ["release_candidate", "hotfix"], "depth": 2,
                                    "types": ["epic", "backlog_item"],
                                    "line": "{number} {field:title}", "group_by": "type" } }],
            "states": [{ "key": "draft", "name": "Draft", "initial": true }]
        })
    }

    // r[verify draft.recipe]
    #[test]
    fn accepts_a_draft_recipe() {
        let mut document = notes();
        document["number"] = json!("{parent}-NOTES-{seq}");
        assert_eq!(problems(document), Vec::<String>::new());
    }

    // r[verify draft.recipe]
    #[test]
    fn checks_draft_recipes() {
        let mut document = notes();
        document["number"] = json!("{parent}-NOTES-{seq}");
        document["fields"][0]["multiline"] = json!(false);
        document["fields"][0]["draft"]["depth"] = json!(6);
        document["fields"][0]["draft"]["types"] = json!([]);
        document["fields"][0]["draft"]["through"] = json!(["Hotfix"]);
        document["fields"][0]["draft"]["line"] = json!("{title}");
        let found = problems(document.clone());
        assert_eq!(found.len(), 5, "{found:?}");
        for (expected, problem) in [
            "fields[0].draft: only multiline text fields can be drafted",
            "fields[0].draft.depth: a depth is from 1 to 5",
            "fields[0].draft.types: a draft lists at least one akt type",
            "fields[0].draft.through[0]: `Hotfix` must be",
            "fields[0].draft.line: `{title}` is not a placeholder",
        ]
        .iter()
        .zip(&found)
        {
            assert!(problem.starts_with(expected), "{problem}");
        }
        document["fields"][0]["draft"]["colour"] = json!("red");
        assert!(serde_json::from_value::<AktTypeDocument>(document).is_err());
    }

    // r[verify container.while]
    #[test]
    fn checks_the_states_a_container_accepts_in() {
        let mut document = release_candidate();
        document["contains"][0]["while"] = json!(["testing", "shipped"]);
        assert_eq!(
            problems(document.clone()),
            vec!["contains[0].while[1]: no state `shipped`"]
        );
        document["contains"][0]["while"] = json!(["testing"]);
        let document: AktTypeDocument = serde_json::from_value(document).unwrap();
        let ty = validate(document).unwrap();
        let epic = ty.contains("epic").unwrap();
        assert!(epic.accepts_in("testing"));
        assert!(!epic.accepts_in("accepted"));
        assert!(ty.contains("backlog_item").unwrap().accepts_in("accepted"));
    }

    // r[verify draft.states]
    #[test]
    fn checks_a_drafts_states() {
        let mut document = notes();
        document["number"] = json!("{parent}-NOTES-{seq}");
        document["fields"][0]["draft"]["states"] =
            json!({ "release_candidate": ["accepted", "accepted"], "Hotfix": ["deployed"] });
        let found = problems(document.clone());
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("fields[0].draft.states.Hotfix: `Hotfix` must be"));
        assert!(found[1].contains("`accepted` is used twice"));
        document["fields"][0]["draft"]["states"] = json!({ "release_candidate": ["accepted"] });
        let document: AktTypeDocument = serde_json::from_value(document).unwrap();
        let ty = validate(document).unwrap();
        let recipe = ty.field("text").unwrap().draft.as_ref().unwrap();
        assert!(recipe.counts("release_candidate", "accepted"));
        assert!(!recipe.counts("release_candidate", "rejected"));
        assert!(recipe.counts("hotfix", "open"));
    }
}
