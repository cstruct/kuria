use std::collections::BTreeMap;

use kuria_config::{AktType, AktTypeDocument, validate};
use serde_json::{Map, Value as Json, json};

use super::*;

fn capa() -> AktType {
    let document: AktTypeDocument = serde_json::from_value(json!({
        "key": "capa",
        "name": "CAPA",
        "number": "CAPA-{year}-{seq:4}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "severity", "name": "Severity", "kind": "choice",
              "options": [{ "key": "major", "name": "Major" }, { "key": "minor", "name": "Minor" }] },
            { "key": "areas", "name": "Areas", "kind": "choices",
              "options": [{ "key": "safety", "name": "Safety" }, { "key": "security", "name": "Security" }] },
            { "key": "root_cause", "name": "Root cause", "kind": "text", "multiline": true },
            { "key": "due", "name": "Due", "kind": "date" },
            { "key": "owner", "name": "Owner", "kind": "subject" },
            { "key": "evidence", "name": "Evidence", "kind": "reference" }
        ],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "investigating", "name": "Investigating" },
            { "key": "closed", "name": "Closed", "final": true }
        ],
        "transitions": [
            { "key": "investigate", "name": "Investigate", "from": ["open"], "to": "investigating" },
            { "key": "close", "name": "Close", "from": ["investigating"], "to": "closed",
              "requires": ["root_cause", "severity"] }
        ]
    }))
    .unwrap();
    validate(document).unwrap()
}

fn input(json: Json) -> Map<String, Json> {
    match json {
        Json::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

fn current(state: &str, fields: Json) -> Current {
    Current {
        state: state.to_owned(),
        fields: fields_from_json(&capa(), &input(fields)).unwrap(),
    }
}

// r[verify akt.create]
#[test]
fn creates_in_the_initial_state_with_the_values_given() {
    let outcome = create(
        &capa(),
        &input(json!({ "title": "Missed review", "severity": "major" })),
    )
    .unwrap();
    assert_eq!(outcome.state, "open");
    assert_eq!(
        fields_to_json(&outcome.fields),
        input(json!({ "title": "Missed review", "severity": "major" }))
    );
    assert_eq!(outcome.changes.len(), 2);
    assert_eq!(outcome.hash, state_hash("open", &outcome.fields));
}

// r[verify akt.create]
#[test]
fn refuses_fields_the_type_doesnt_define() {
    assert_eq!(
        create(&capa(), &input(json!({ "title": "T", "colour": "red" }))),
        Err(Refusal::UnknownField("colour".to_owned()))
    );
}

// r[verify akt.field.required]
#[test]
fn required_fields_are_set_on_creation_and_never_cleared() {
    assert_eq!(
        create(&capa(), &input(json!({ "severity": "major" }))),
        Err(Refusal::Required("title".to_owned()))
    );
    let akt = current("open", json!({ "title": "T" }));
    assert_eq!(
        edit(&capa(), &akt, &input(json!({ "title": null }))),
        Err(Refusal::Required("title".to_owned()))
    );
}

// r[verify akt.field.kinds]
#[test]
fn accepts_values_of_each_kind() {
    let outcome = create(
        &capa(),
        &input(json!({
            "title": "T",
            "severity": "minor",
            "areas": ["safety", "security"],
            "root_cause": "line one\nline two",
            "due": "2026-12-31",
            "owner": "01900000-0000-7000-8000-0000000000aa",
            "evidence": { "url": "https://git.example/pr/1", "label": "PR 1" }
        })),
    )
    .unwrap();
    assert_eq!(outcome.fields.len(), 7);
}

// r[verify akt.field.kinds]
#[test]
fn refuses_values_that_dont_fit() {
    for (field, value) in [
        ("title", json!(42)),
        ("title", json!("two\nlines")),
        ("severity", json!("catastrophic")),
        ("areas", json!(["safety", "safety"])),
        ("areas", json!("safety")),
        ("due", json!("2026-02-30")),
        ("due", json!("2026-1-5")),
        ("owner", json!("alice")),
        ("evidence", json!({ "url": "ftp://files.example/x" })),
        ("evidence", json!({ "url": "not a url" })),
        (
            "evidence",
            json!({ "url": "https://x.example", "colour": "red" }),
        ),
    ] {
        let mut values = Map::new();
        values.insert("title".to_owned(), json!("T"));
        values.insert(field.to_owned(), value.clone());
        assert!(
            matches!(create(&capa(), &values), Err(Refusal::InvalidValue { .. })),
            "{field}: {value}"
        );
    }
}

// r[verify akt.edit]
#[test]
fn edits_only_what_is_given_and_null_clears() {
    let akt = current(
        "open",
        json!({ "title": "T", "severity": "major", "due": "2026-12-31" }),
    );
    let outcome = edit(
        &capa(),
        &akt,
        &input(json!({ "severity": "minor", "due": null })),
    )
    .unwrap();
    assert_eq!(
        fields_to_json(&outcome.fields),
        input(json!({ "title": "T", "severity": "minor" }))
    );
    assert_eq!(
        outcome.changes,
        BTreeMap::from([
            ("due".to_owned(), None),
            (
                "severity".to_owned(),
                Some(Value::Choice("minor".to_owned()))
            ),
        ])
    );
    assert_eq!(outcome.state, "open");
}

// r[verify akt.edit]
#[test]
fn refuses_an_edit_that_changes_nothing_or_a_final_akt() {
    let akt = current("open", json!({ "title": "T" }));
    assert_eq!(
        edit(&capa(), &akt, &input(json!({ "title": "T", "due": null }))),
        Err(Refusal::NoChange)
    );
    let closed = current("closed", json!({ "title": "T" }));
    assert_eq!(
        edit(&capa(), &closed, &input(json!({ "title": "U" }))),
        Err(Refusal::Final)
    );
}

// r[verify akt.transition]
#[test]
fn transitions_from_its_states_only() {
    let akt = current("open", json!({ "title": "T" }));
    assert_eq!(
        transition(&capa(), &akt, "close", &Map::new()),
        Err(Refusal::NotFrom {
            transition: "close".to_owned(),
            state: "open".to_owned()
        })
    );
    assert_eq!(
        transition(&capa(), &akt, "reopen", &Map::new()),
        Err(Refusal::UnknownTransition("reopen".to_owned()))
    );
    let outcome = transition(&capa(), &akt, "investigate", &Map::new()).unwrap();
    assert_eq!(outcome.state, "investigating");
    assert!(
        outcome.changes.is_empty(),
        "a transition alone changes no field"
    );
}

// r[verify akt.transition]
#[test]
fn a_transition_needs_its_required_fields_afterwards() {
    let akt = current(
        "investigating",
        json!({ "title": "T", "severity": "major" }),
    );
    assert_eq!(
        transition(&capa(), &akt, "close", &Map::new()),
        Err(Refusal::Requires {
            transition: "close".to_owned(),
            fields: vec!["root_cause".to_owned()]
        })
    );
    let outcome = transition(
        &capa(),
        &akt,
        "close",
        &input(json!({ "root_cause": "No reviewer was assigned" })),
    )
    .unwrap();
    assert_eq!(outcome.state, "closed");
    assert_eq!(outcome.changes.len(), 1);
}
