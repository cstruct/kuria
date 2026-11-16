use std::collections::BTreeMap;

use kuria_config::{AktType, AktTypeDocument, validate};
use serde_json::{Map, Value as Json, json};
use uuid::Uuid;

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
            { "key": "image", "name": "Image", "kind": "docker" },
            { "key": "report", "name": "Report", "kind": "attachment", "format": "sbom" }
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
        transition(&capa(), &akt, "close", &Map::new(), change(), None),
        Err(Refusal::NotFrom {
            transition: "close".to_owned(),
            state: "open".to_owned()
        })
    );
    assert_eq!(
        transition(&capa(), &akt, "reopen", &Map::new(), change(), None),
        Err(Refusal::UnknownTransition("reopen".to_owned()))
    );
    let outcome = transition(&capa(), &akt, "investigate", &Map::new(), change(), None).unwrap();
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
        transition(&capa(), &akt, "close", &Map::new(), change(), None),
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
        None,
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
        on_behalf_of: None,
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
            change(),
            None
        ),
        Err(Refusal::Requires { .. })
    ));
    let released = transition(
        &ty,
        &akt,
        "release",
        &input(json!({ "go": true })),
        change(),
        None,
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
    let text =
        |key: &str| outcome.fields[key].display(ty.field(key).unwrap(), &|_| "Alice".to_owned());
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

// r[verify akt.field.kinds]
#[test]
fn holds_an_attachment_by_its_identifier() {
    let id = "01900000-0000-7000-8000-0000000000f1";
    let outcome = create(
        &capa(),
        &input(json!({ "title": "T", "report": id })),
        change(),
    )
    .unwrap();
    assert_eq!(
        outcome.fields["report"],
        Value::Attachment(Uuid::parse_str(id).unwrap())
    );
    assert_eq!(outcome.fields["report"].to_json(), json!(id));
    for value in [json!("report.json"), json!({ "attachment": id }), json!(1)] {
        let refused = create(
            &capa(),
            &input(json!({ "title": "T", "report": value.clone() })),
            change(),
        );
        assert!(
            matches!(refused, Err(Refusal::InvalidValue { ref field, .. }) if field == "report"),
            "{value}: {refused:?}"
        );
    }
}

const BOB: &str = "01900000-0000-7000-8000-0000000000b0";
const CONTACT: &str = "01900000-0000-7000-8000-0000000000c1";

fn someone_named(someone: &Someone) -> String {
    match someone {
        Someone::User(id) if id.to_string() == BOB => "Bob".to_owned(),
        Someone::User(_) => "Alice".to_owned(),
        Someone::Contact(_) => "Carl".to_owned(),
    }
}

// r[verify akt.field.checkbox]
#[test]
fn a_box_is_checked_for_someone_else() {
    let ty = capa();
    for (whom, someone) in [
        (json!(BOB), Someone::User(Uuid::parse_str(BOB).unwrap())),
        (
            json!({ "contact": CONTACT }),
            Someone::Contact(Uuid::parse_str(CONTACT).unwrap()),
        ),
    ] {
        let created = create(
            &ty,
            &input(json!({ "title": "T", "verified": { "for": whom } })),
            change(),
        )
        .unwrap();
        let checked = &created.fields["verified"];
        assert_eq!(
            checked,
            &Value::Checkbox {
                by: change().by,
                at: change().at,
                on_behalf_of: Some(someone),
            }
        );
        assert_eq!(checked.named(), vec![Someone::User(change().by), someone]);
        // Stored with `for`, and read back.
        let stored = fields_to_json(&created.fields);
        assert_eq!(stored["verified"]["for"], someone.to_json());
        assert_eq!(fields_from_json(&ty, &stored).unwrap(), created.fields);
    }
    // For the checker is the same as `true`.
    let mine = create(
        &ty,
        &input(json!({ "title": "T", "verified": { "for": change().by.to_string() } })),
        change(),
    )
    .unwrap();
    assert!(matches!(
        mine.fields["verified"],
        Value::Checkbox {
            on_behalf_of: None,
            ..
        }
    ));
    assert!(
        fields_to_json(&mine.fields)["verified"]
            .get("for")
            .is_none()
    );
    for bad in [
        json!({}),
        json!({ "for": "Bob" }),
        json!({ "for": BOB, "by": BOB }),
        json!({ "for": { "contact": "nope" } }),
    ] {
        assert!(
            create(
                &ty,
                &input(json!({ "title": "T", "verified": bad.clone() })),
                change()
            )
            .is_err(),
            "{bad}"
        );
    }
}

// r[verify akt.field.kinds]
// r[verify contact.reference]
#[test]
fn a_user_field_names_a_user_or_a_contact() {
    let ty = capa();
    let user = create(&ty, &input(json!({ "title": "T", "owner": BOB })), change()).unwrap();
    // A plain subject identifier, as values were stored before contacts.
    assert_eq!(fields_to_json(&user.fields)["owner"], json!(BOB));
    let contact = create(
        &ty,
        &input(json!({ "title": "T", "owner": { "contact": CONTACT } })),
        change(),
    )
    .unwrap();
    assert_eq!(
        contact.fields["owner"],
        Value::User(Someone::Contact(Uuid::parse_str(CONTACT).unwrap()))
    );
    assert_eq!(
        fields_to_json(&contact.fields)["owner"],
        json!({ "contact": CONTACT })
    );
    for bad in [
        json!({ "contact": CONTACT, "name": "Carl" }),
        json!({ "user": BOB }),
    ] {
        assert!(
            create(
                &ty,
                &input(json!({ "title": "T", "owner": bad.clone() })),
                change()
            )
            .is_err(),
            "{bad}"
        );
    }
}

// r[verify draft.render]
#[test]
fn shows_whom_a_box_was_checked_for() {
    let ty = capa();
    let text = |fields: Json| {
        let outcome = create(&ty, &input(fields), change()).unwrap();
        let key = if outcome.fields.contains_key("verified") {
            "verified"
        } else {
            "owner"
        };
        outcome.fields[key].display(ty.field(key).unwrap(), &someone_named)
    };
    assert_eq!(text(json!({ "title": "T", "verified": true })), "yes");
    assert_eq!(
        text(json!({ "title": "T", "verified": { "for": BOB } })),
        "yes, for Bob (checked by Alice)"
    );
    assert_eq!(
        text(json!({ "title": "T", "verified": { "for": { "contact": CONTACT } } })),
        "yes, for Carl (checked by Alice)"
    );
    assert_eq!(
        text(json!({ "title": "T", "owner": { "contact": CONTACT } })),
        "Carl"
    );
}

fn test_type() -> AktType {
    let document: AktTypeDocument = serde_json::from_value(json!({
        "key": "test", "name": "Test", "number": "T-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "testers", "name": "Testers", "kind": "user", "multiple": true },
            { "key": "verdict", "name": "Verdict", "kind": "verdict", "assignees": "testers" }
        ],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "closed", "name": "Closed", "final": true }
        ]
    }))
    .unwrap();
    validate(document).unwrap()
}

fn subject(n: u128) -> String {
    Uuid::from_u128(n).to_string()
}

fn by(n: u128) -> Change {
    Change {
        by: Uuid::from_u128(n),
        ..change()
    }
}

/// `(assignee, verdict, given by)` of an outcome's verdicts.
fn verdicts(fields: &BTreeMap<String, Value>) -> Vec<(Someone, &'static str, Uuid)> {
    match fields.get("verdict") {
        Some(Value::Verdicts(given)) => given
            .iter()
            .map(|g| (g.assignee, g.verdict.as_str(), g.by))
            .collect(),
        None => Vec::new(),
        other => panic!("{other:?}"),
    }
}

/// A test of `test_type()` assigned to 1 and 2.
fn assigned() -> Current {
    let made = create(
        &test_type(),
        &input(json!({ "title": "Boots", "testers": [subject(1), subject(2)] })),
        change(),
    )
    .unwrap();
    Current {
        state: made.state,
        fields: made.fields,
    }
}

/// Gives `raw` as `who`, moving `now` on if it is taken.
fn give(now: &mut Current, raw: &Json, who: u128) -> Result<(), Refusal> {
    let o = edit(
        &test_type(),
        now,
        &input(json!({ "verdict": raw })),
        by(who),
    )?;
    *now = Current {
        state: o.state,
        fields: o.fields,
    };
    Ok(())
}

fn outcome_of(now: &Current) -> Option<outcomes::Outcome> {
    outcomes::of_fields(&test_type(), &now.fields)
}

// r[verify akt.field.verdict]
// r[verify akt.outcome]
#[test]
fn assignees_give_verdicts_and_the_outcome_follows() {
    let (a, b, outsider) = (1, 2, 3);
    let mut now = assigned();
    assert_eq!(outcome_of(&now), Some(outcomes::Outcome::Pending));
    give(&mut now, &json!("pass"), a).unwrap();
    assert!(
        matches!(give(&mut now, &json!("pass"), a), Err(Refusal::NoChange)),
        "the same again changes nothing"
    );
    // Not an assignee: not one's own, and not for someone else either.
    for (raw, who) in [
        (json!("fail"), outsider),
        (json!({ "verdict": "skip", "for": subject(outsider) }), a),
        (Json::Null, a),
    ] {
        let refused = give(&mut now, &raw, who).unwrap_err();
        assert!(
            matches!(refused, Refusal::InvalidValue { .. }),
            "{raw}: {refused:?}"
        );
    }
    // For an assignee, by anyone, recorded as given by them.
    give(
        &mut now,
        &json!({ "verdict": "skip", "for": subject(b) }),
        outsider,
    )
    .unwrap();
    assert_eq!(
        verdicts(&now.fields),
        [
            (
                Someone::User(Uuid::from_u128(a)),
                "pass",
                Uuid::from_u128(a)
            ),
            (
                Someone::User(Uuid::from_u128(b)),
                "skip",
                Uuid::from_u128(outsider)
            ),
        ]
    );
    assert_eq!(outcome_of(&now), Some(outcomes::Outcome::Pass));
    give(&mut now, &json!("fail"), b).unwrap();
    assert_eq!(outcome_of(&now), Some(outcomes::Outcome::Fail));
    give(&mut now, &json!({ "clear": true }), b).unwrap();
    assert_eq!(outcome_of(&now), Some(outcomes::Outcome::Pending));
}

// r[verify akt.field.multiple]
// r[verify akt.field.verdict]
#[test]
fn an_assignee_removed_takes_their_verdict_with_them() {
    let ty = test_type();
    let (a, b, newcomer) = (1, 2, 4);
    let mut now = assigned();
    give(&mut now, &json!("pass"), a).unwrap();
    let dropped = edit(
        &ty,
        &now,
        &input(json!({ "testers": [subject(b)] })),
        change(),
    )
    .unwrap();
    assert_eq!(dropped.changes.get("verdict"), Some(&None));
    assert_eq!(verdicts(&dropped.fields), []);
    let none = edit(&ty, &now, &input(json!({ "testers": [] })), change()).unwrap();
    assert!(
        !none.fields.contains_key("testers"),
        "an empty list is no value"
    );
    assert!(
        edit(
            &ty,
            &now,
            &input(json!({ "testers": [subject(a), subject(a)] })),
            change()
        )
        .is_err(),
        "someone named twice"
    );
    // Assignees and a verdict in one change: the verdict sees the new ones.
    let both = edit(
        &ty,
        &now,
        &input(json!({ "testers": [subject(a), subject(newcomer)], "verdict": "pass" })),
        by(newcomer),
    )
    .unwrap();
    assert_eq!(verdicts(&both.fields).len(), 2);
}

fn gated_test_type() -> AktType {
    let mut document = test_type().document().clone();
    document.transitions = serde_json::from_value(json!([
        { "key": "finish", "name": "Finish", "from": ["open"], "to": "closed",
          "outcomes": ["pass", "fail", "skip"] }
    ]))
    .unwrap();
    validate(document).unwrap()
}

// r[verify akt.transition.outcomes]
#[test]
fn a_transition_may_require_an_outcome() {
    let ty = gated_test_type();
    let finish = |now: &Current, held| transition(&ty, now, "finish", &Map::new(), change(), held);
    let mut now = assigned();
    let pending = finish(&now, None).unwrap_err();
    assert_eq!(
        pending.to_string(),
        "`finish` requires the outcome to be pass, fail or skip, and it is pending"
    );
    // Its own verdicts decide, not what the caller says its members hold.
    assert!(finish(&now, Some(outcomes::Outcome::Pass)).is_err());
    give(&mut now, &json!("pass"), 1).unwrap();
    assert!(finish(&now, None).is_err(), "2 hasn't answered");
    give(&mut now, &json!("fail"), 2).unwrap();
    assert_eq!(finish(&now, None).unwrap().state, "closed");
    // A verdict the transition gives counts.
    let mut later = assigned();
    give(&mut later, &json!("pass"), 1).unwrap();
    let given = transition(
        &ty,
        &later,
        "finish",
        &input(json!({ "verdict": "skip" })),
        by(2),
        None,
    );
    assert_eq!(given.unwrap().state, "closed");
}

// r[verify akt.transition.outcomes]
#[test]
fn a_container_s_transition_goes_by_what_its_members_hold() {
    let document: AktTypeDocument = serde_json::from_value(json!({
        "key": "protocol", "name": "Protocol", "number": "P-{seq}",
        "fields": [],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["open"], "to": "done",
                          "outcomes": ["pass"] }],
        "contains": [{ "type": "test" }]
    }))
    .unwrap();
    let ty = validate(document).unwrap();
    let open = current("open", json!({}));
    let finish = |held| transition(&ty, &open, "finish", &Map::new(), change(), held);
    assert_eq!(finish(Some(outcomes::Outcome::Pass)).unwrap().state, "done");
    assert!(matches!(
        finish(Some(outcomes::Outcome::Skip)),
        Err(Refusal::Outcome {
            outcome: Some(outcomes::Outcome::Skip),
            ..
        })
    ));
    assert_eq!(
        finish(None).unwrap_err().to_string(),
        "`finish` requires the outcome to be pass, and the akt has no outcome"
    );
}

// r[verify akt.edit]
#[test]
fn says_whether_an_edit_changes_anything_also_of_a_final_akt() {
    let closed = current("closed", json!({ "title": "T" }));
    let ty = capa();
    assert!(!changes_anything(&ty, &closed, &input(json!({ "title": "T" })), change()).unwrap());
    assert!(changes_anything(&ty, &closed, &input(json!({ "title": "U" })), change()).unwrap());
    assert_eq!(
        edit(&ty, &closed, &input(json!({ "title": "U" })), change()),
        Err(Refusal::Final)
    );
}

fn document_type() -> AktType {
    let document: AktTypeDocument = serde_json::from_value(json!({
        "key": "document", "name": "Document", "number": "D-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "body", "name": "Body", "kind": "markdown", "references": "refers_to" },
            { "key": "appendix", "name": "Appendix", "kind": "markdown" }
        ],
        "states": [{ "key": "draft", "name": "Draft", "initial": true }],
        "links": [{ "key": "refers_to", "name": "Refers to", "inverse": "Referred to by", "to": ["document"] }]
    }))
    .unwrap();
    validate(document).unwrap()
}

// r[verify akt.field.markdown]
#[test]
fn markdown_fields_get_heading_ids_unique_across_the_akt() {
    let ty = document_type();
    let made = create(
        &ty,
        &input(json!({ "title": "Guide", "body": "# Scope\n\nText.\n", "appendix": "# Scope\n" })),
        change(),
    )
    .unwrap();
    assert_eq!(
        made.fields["body"],
        Value::Markdown("# Scope {#scope}\n\nText.\n".to_owned())
    );
    assert_eq!(
        made.fields["appendix"],
        Value::Markdown("# Scope {#scope-2}\n".to_owned())
    );
    let now = Current {
        state: made.state,
        fields: made.fields,
    };
    // The same text, sent without the id it was given, changes nothing.
    assert_eq!(
        edit(
            &ty,
            &now,
            &input(json!({ "body": "# Scope\n\nText.\n" })),
            change()
        ),
        Err(Refusal::NoChange)
    );
    // A renamed heading keeps its id.
    let renamed = edit(
        &ty,
        &now,
        &input(json!({ "body": "# Purpose {#scope}\n\nText.\n" })),
        change(),
    )
    .unwrap();
    assert_eq!(renamed.changes.keys().collect::<Vec<_>>(), ["body"]);
    // Empty Markdown clears; an id given twice is refused by field.
    let cleared = edit(&ty, &now, &input(json!({ "appendix": "  \n" })), change()).unwrap();
    assert!(!cleared.fields.contains_key("appendix"));
    assert_eq!(
        edit(
            &ty,
            &now,
            &input(json!({ "appendix": "# A {#scope}\n" })),
            change()
        ),
        Err(Refusal::InvalidValue {
            field: "appendix".to_owned(),
            reason: "two headings have the id `scope`".to_owned()
        })
    );
}
