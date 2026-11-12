//! Validating an akt type document.

use std::collections::BTreeSet;

use crate::document::{AktTypeDocument, FieldDef, FieldKind, StateDef, TransitionDef};
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

    let fields = problems.unique_keys("fields", document.fields.iter().map(|f| f.key.as_str()));
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
        if field.multiline && field.kind != FieldKind::Text {
            problems.add(
                &format!("{path}.multiline"),
                "only text fields are multiline",
            );
        }
    }

    let states = problems.unique_keys("states", document.states.iter().map(|s| s.key.as_str()));
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

    problems.unique_keys(
        "transitions",
        document.transitions.iter().map(|t| t.key.as_str()),
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

    match (problems.0.is_empty(), number) {
        (true, Some(number)) => Ok(AktType { document, number }),
        _ => Err(problems.0),
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

    /// Checks each key and that none repeats, and returns them.
    fn unique_keys<'a>(
        &mut self,
        path: &str,
        keys: impl Iterator<Item = &'a str>,
    ) -> BTreeSet<&'a str> {
        let mut seen = BTreeSet::new();
        for (i, key) in keys.enumerate() {
            self.key(&format!("{path}[{i}].key"), key);
            if !seen.insert(key) {
                self.add(
                    &format!("{path}[{i}].key"),
                    format!("`{key}` is used twice"),
                );
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
}
