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
            { "key": "owner", "name": "Owner", "kind": "user" },
            { "key": "evidence", "name": "Evidence", "kind": "reference" },
            { "key": "verified", "name": "Verified", "kind": "checkbox" },
            { "key": "source", "name": "Source", "kind": "git" },
            { "key": "image", "name": "Image", "kind": "docker" }
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

fn change() -> Change {
    Change {
        by: uuid::Uuid::from_u128(7),
        at: chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap(),
    }
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
        change(),
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
        create(
            &capa(),
            &input(json!({ "title": "T", "colour": "red" })),
            change()
        ),
        Err(Refusal::UnknownField("colour".to_owned()))
    );
}

// r[verify akt.field.required]
#[test]
fn required_fields_are_set_on_creation_and_never_cleared() {
    assert_eq!(
        create(&capa(), &input(json!({ "severity": "major" })), change()),
        Err(Refusal::Required("title".to_owned()))
    );
    let akt = current("open", json!({ "title": "T" }));
    assert_eq!(
        edit(&capa(), &akt, &input(json!({ "title": null })), change()),
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
        change(),
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
            matches!(
                create(&capa(), &values, change()),
                Err(Refusal::InvalidValue { .. })
            ),
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
        change(),
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
        edit(
            &capa(),
            &akt,
            &input(json!({ "title": "T", "due": null })),
            change()
        ),
        Err(Refusal::NoChange)
    );
    let closed = current("closed", json!({ "title": "T" }));
    assert_eq!(
        edit(&capa(), &closed, &input(json!({ "title": "U" })), change()),
        Err(Refusal::Final)
    );
}

// r[verify akt.transition]
#[test]
fn transitions_from_its_states_only() {
    let akt = current("open", json!({ "title": "T" }));
    assert_eq!(
        transition(&capa(), &akt, "close", &Map::new(), change()),
        Err(Refusal::NotFrom {
            transition: "close".to_owned(),
            state: "open".to_owned()
        })
    );
    assert_eq!(
        transition(&capa(), &akt, "reopen", &Map::new(), change()),
        Err(Refusal::UnknownTransition("reopen".to_owned()))
    );
    let outcome = transition(&capa(), &akt, "investigate", &Map::new(), change()).unwrap();
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
        transition(&capa(), &akt, "close", &Map::new(), change()),
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
        change(),
    )
    .unwrap();
    assert_eq!(outcome.state, "closed");
    assert_eq!(outcome.changes.len(), 1);
}

const SHA: &str = "3f9a1c2b4d5e6f708192a3b4c5d6e7f809182736";
const DIGEST: &str = "sha256:9b2c1d7e3f4a5b6c7d8e9f0a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e";

// r[verify akt.field.checkbox]
#[test]
fn checking_records_who_and_when_and_keeps_the_first() {
    let created = create(
        &capa(),
        &input(json!({ "title": "T", "verified": true })),
        change(),
    )
    .unwrap();
    let checked = Value::Checkbox {
        by: change().by,
        at: change().at,
    };
    assert_eq!(created.fields["verified"], checked);

    let later = Change {
        by: uuid::Uuid::from_u128(8),
        at: change().at + chrono::TimeDelta::hours(1),
    };
    let akt = Current {
        state: "open".to_owned(),
        fields: created.fields,
    };
    // Checking it again changes nothing: the first record stays.
    assert_eq!(
        edit(&capa(), &akt, &input(json!({ "verified": true })), later),
        Err(Refusal::NoChange)
    );
    let cleared = edit(&capa(), &akt, &input(json!({ "verified": false })), later).unwrap();
    assert!(!cleared.fields.contains_key("verified"));
    assert_eq!(cleared.changes["verified"], None);
    assert!(matches!(
        create(
            &capa(),
            &input(json!({ "title": "T", "verified": "yes" })),
            change()
        ),
        Err(Refusal::InvalidValue { .. })
    ));
    // Input can't claim someone else checked it.
    assert!(matches!(
        create(
            &capa(),
            &input(
                json!({ "title": "T", "verified": { "checked_by": SUBJECT_B, "checked_at": "2026-01-01T00:00:00Z" } })
            ),
            change()
        ),
        Err(Refusal::InvalidValue { .. })
    ));
}

const SUBJECT_B: &str = "01900000-0000-7000-8000-0000000000bb";

// r[verify akt.field.checkbox]
#[test]
fn a_transition_requiring_a_checkbox_needs_it_checked() {
    let document: AktTypeDocument = serde_json::from_value(json!({
        "key": "release", "name": "Release", "number": "R-{seq}",
        "fields": [{ "key": "go", "name": "Go", "kind": "checkbox" }],
        "states": [{ "key": "ready", "name": "Ready", "initial": true },
                   { "key": "released", "name": "Released", "final": true }],
        "transitions": [{ "key": "release", "name": "Release", "from": ["ready"],
                          "to": "released", "requires": ["go"] }]
    }))
    .unwrap();
    let ty = validate(document).unwrap();
    let akt = Current {
        state: "ready".to_owned(),
        fields: BTreeMap::new(),
    };
    assert!(matches!(
        transition(
            &ty,
            &akt,
            "release",
            &input(json!({ "go": false })),
            change()
        ),
        Err(Refusal::Requires { .. })
    ));
    let released = transition(
        &ty,
        &akt,
        "release",
        &input(json!({ "go": true })),
        change(),
    )
    .unwrap();
    assert_eq!(released.state, "released");
}

// r[verify akt.field.kinds]
#[test]
fn accepts_git_references_and_images() {
    for source in [
        json!({ "repository": "https://git.example/team/app.git", "sha": SHA }),
        json!({ "repository": "git@git.example:team/app.git", "tag": "v4.2.0" }),
        json!({ "repository": "ssh://git@git.example/team/app.git", "sha": SHA, "tag": "release/4.2" }),
    ] {
        let outcome = create(
            &capa(),
            &input(json!({ "title": "T", "source": source.clone() })),
            change(),
        );
        assert!(outcome.is_ok(), "{source}: {outcome:?}");
    }
    for image in [
        json!({ "repository": "registry.example/team/backend", "digest": DIGEST, "tag": "4.2.0" }),
        json!({ "repository": "localhost:5000/backend", "digest": DIGEST }),
        json!({ "repository": "backend", "digest": DIGEST, "tag": "latest" }),
    ] {
        let outcome = create(
            &capa(),
            &input(json!({ "title": "T", "image": image.clone() })),
            change(),
        );
        assert!(outcome.is_ok(), "{image}: {outcome:?}");
    }
}

// r[verify akt.field.kinds]
#[test]
fn refuses_git_references_and_images_that_dont_fit() {
    for (field, value) in [
        (
            "source",
            json!({ "repository": "https://git.example/app.git" }),
        ),
        (
            "source",
            json!({ "repository": "ftp://git.example/app.git", "sha": SHA }),
        ),
        ("source", json!({ "repository": "not a repo", "sha": SHA })),
        (
            "source",
            json!({ "repository": "https://git.example/app.git", "sha": "3F9A1C2B" }),
        ),
        (
            "source",
            json!({ "repository": "https://git.example/app.git", "tag": "bad tag" }),
        ),
        (
            "source",
            json!({ "repository": "https://git.example/app.git", "tag": "v1", "branch": "main" }),
        ),
        (
            "image",
            json!({ "repository": "registry.example/team/backend", "tag": "4.2.0" }),
        ),
        (
            "image",
            json!({ "repository": "registry.example/Team/Backend", "digest": DIGEST }),
        ),
        (
            "image",
            json!({ "repository": "backend", "digest": "sha256:abc" }),
        ),
        (
            "image",
            json!({ "repository": "backend", "digest": DIGEST, "tag": "-bad" }),
        ),
    ] {
        let mut values = Map::new();
        values.insert("title".to_owned(), json!("T"));
        values.insert(field.to_owned(), value.clone());
        assert!(
            matches!(
                create(&capa(), &values, change()),
                Err(Refusal::InvalidValue { .. })
            ),
            "{field}: {value}"
        );
    }
}

// r[verify draft.render]
#[test]
fn shows_the_new_kinds_as_text() {
    let ty = capa();
    let outcome = create(
        &ty,
        &input(json!({
            "title": "T",
            "verified": true,
            "source": { "repository": "https://git.example/app.git", "sha": SHA, "tag": "v4.2.0" },
            "image": { "repository": "registry.example/backend", "digest": DIGEST, "tag": "4.2.0" },
        })),
        change(),
    )
    .unwrap();
    let text = |key: &str| outcome.fields[key].display(ty.field(key).unwrap());
    assert_eq!(text("verified"), "yes");
    assert_eq!(
        text("source"),
        "https://git.example/app.git@v4.2.0 (3f9a1c2b4d5e)"
    );
    assert_eq!(
        text("image"),
        format!("registry.example/backend:4.2.0@{DIGEST}")
    );
    // And the stored form reads back as the same value.
    let stored = fields_to_json(&outcome.fields);
    assert_eq!(fields_from_json(&ty, &stored).unwrap(), outcome.fields);
}
