//! Akt types and akter through GraphQL, against a fresh database for each
//! test and a fake relying host.

mod support;

use axum::Router;
use chrono::{Datelike, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use kuria_store::schema::{akt_revision, change_event};
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use support::{FakeHost, SUBJECT, graphql_with, router_with, send};

const AKT: &str = "number type { key version } state { key } revision
    fields {
        __typename
        ... on TextValue { field text }
        ... on ChoiceValue { field option { key name } }
        ... on ChoicesValue { field options { key } }
        ... on DateValue { field date }
        ... on UserValue { field user { ... on User { id } } }
        ... on ReferenceValue { field url label }
    }";

struct World {
    db: TestDatabase,
    router: Router,
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let router = router_with(
            &FakeHost::new(),
            db.pool().clone(),
            support::decider::FakeDecider::allowing_tenant(),
        );
        Self { db, router }
    }

    /// Runs `query` as a subject of the main tenant, and returns its answer.
    async fn run(&self, query: &str, variables: Value) -> Value {
        self.run_as("tenant", query, variables).await
    }

    async fn run_as(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = send(&self.router, graphql_with(token, query, &variables)).await;
        assert_eq!(answer.status, 200, "{}", answer.body);
        answer.json()
    }

    /// Runs `query` and returns its data, failing on any error.
    async fn data(&self, query: &str, variables: Value) -> Value {
        let answer = self.run(query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn define(&self, document: Value) -> Value {
        self.data(
            "mutation($d: JSON!) { defineAktType(document: $d) { key version } }",
            json!({ "d": document }),
        )
        .await["defineAktType"]
            .clone()
    }

    async fn create(&self, fields: Value) -> Value {
        self.data(
            &format!("mutation($f: JSON) {{ createAkt(input: {{ type: \"capa\", fields: $f }}) {{ {AKT} }} }}"),
            json!({ "f": fields }),
        )
        .await["createAkt"]
            .clone()
    }

    async fn revision_rows(&self) -> i64 {
        let mut conn = self.db.pool().get().await.unwrap();
        akt_revision::table
            .count()
            .get_result(&mut conn)
            .await
            .unwrap()
    }
}

fn capa() -> Value {
    json!({
        "key": "capa",
        "name": "CAPA",
        "number": "CAPA-{year}-{seq:4}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "severity", "name": "Severity", "kind": "choice",
              "options": [{ "key": "major", "name": "Major" }, { "key": "minor", "name": "Minor" }] },
            { "key": "root_cause", "name": "Root cause", "kind": "text", "multiline": true },
            { "key": "owner", "name": "Owner", "kind": "user" },
            { "key": "evidence", "name": "Evidence", "kind": "reference" }
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

fn code(answer: &Value) -> &str {
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
}

fn year() -> i32 {
    Utc::now().year()
}

// r[verify akt.type.document]
#[tokio::test(flavor = "multi_thread")]
async fn each_submission_is_the_next_version() {
    let world = World::new().await;
    assert_eq!(
        world.define(capa()).await,
        json!({ "key": "capa", "version": 1 })
    );
    let mut changed = capa();
    changed["name"] = json!("Corrective and preventive action");
    assert_eq!(
        world.define(changed).await,
        json!({ "key": "capa", "version": 2 })
    );

    let data = world
        .data(
            "{ first: aktType(key: \"capa\", version: 1) { name } latest: aktType(key: \"capa\") { name version } aktTypes { key version } }",
            json!({}),
        )
        .await;
    assert_eq!(data["first"]["name"], "CAPA");
    assert_eq!(data["latest"]["version"], 2);
    assert_eq!(data["aktTypes"], json!([{ "key": "capa", "version": 2 }]));
}

// r[verify akt.type.validation]
#[tokio::test(flavor = "multi_thread")]
async fn an_invalid_document_is_refused_with_its_problems() {
    let world = World::new().await;
    let mut document = capa();
    document["states"][0]["initial"] = json!(false);
    let answer = world
        .run(
            "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
            json!({ "d": document }),
        )
        .await;
    assert_eq!(code(&answer), "INVALID_AKT_TYPE");
    assert_eq!(
        answer["errors"][0]["extensions"]["problems"],
        json!([{ "path": "states", "message": "exactly one state must be initial, not 0" }])
    );
}

// r[verify akt.type.tenant]
#[tokio::test(flavor = "multi_thread")]
async fn akt_types_and_akter_stay_in_their_tenant() {
    let world = World::new().await;
    world.define(capa()).await;
    let akt = world.create(json!({ "title": "Missed review" })).await;
    let number = akt["number"].as_str().unwrap();

    let other = world
        .run_as(
            "other-tenant",
            &format!(
                "{{ aktTypes {{ key }} akt(number: \"{number}\") {{ number }} akter {{ nodes {{ number }} }} }}"
            ),
            json!({}),
        )
        .await;
    assert_eq!(
        other["data"],
        json!({ "aktTypes": [], "akt": null, "akter": { "nodes": [] } })
    );
    let create = world
        .run_as(
            "other-tenant",
            "mutation { createAkt(input: { type: \"capa\", fields: { title: \"T\" } }) { number } }",
            json!({}),
        )
        .await;
    assert_eq!(code(&create), "UNKNOWN_AKT_TYPE");
}

// r[verify akt.create]
// r[verify akt.number.sequence]
#[tokio::test(flavor = "multi_thread")]
async fn creates_akter_numbered_in_sequence() {
    let world = World::new().await;
    world.define(capa()).await;
    let first = world
        .create(json!({ "title": "Missed review", "severity": "major" }))
        .await;
    assert_eq!(first["number"], format!("CAPA-{}-0001", year()));
    assert_eq!(first["state"]["key"], "open");
    assert_eq!(first["revision"], 1);
    assert_eq!(first["type"], json!({ "key": "capa", "version": 1 }));
    assert_eq!(
        first["fields"],
        json!([
            { "__typename": "TextValue", "field": "title", "text": "Missed review" },
            { "__typename": "ChoiceValue", "field": "severity", "option": { "key": "major", "name": "Major" } }
        ])
    );
    let second = world.create(json!({ "title": "Late audit" })).await;
    assert_eq!(second["number"], format!("CAPA-{}-0002", year()));
}

// r[verify akt.create]
// r[verify akt.field.kinds]
// r[verify akt.field.required]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_values_that_dont_fit_and_stores_nothing() {
    let world = World::new().await;
    world.define(capa()).await;
    for fields in [
        json!({ "severity": "major" }),
        json!({ "title": "T", "severity": "catastrophic" }),
        json!({ "title": "T", "colour": "red" }),
        json!({ "title": "T", "evidence": { "url": "javascript:alert(1)" } }),
    ] {
        let answer = world
            .run(
                "mutation($f: JSON) { createAkt(input: { type: \"capa\", fields: $f }) { number } }",
                json!({ "f": fields }),
            )
            .await;
        assert_eq!(code(&answer), "REFUSED", "{fields}");
    }
    assert_eq!(world.revision_rows().await, 0);
}

// r[verify akt.edit]
// r[verify akt.revision.record]
#[tokio::test(flavor = "multi_thread")]
async fn edits_and_records_each_revision() {
    let world = World::new().await;
    world.define(capa()).await;
    let akt = world.create(json!({ "title": "Missed review" })).await;
    let number = akt["number"].as_str().unwrap();

    let edited = world
        .data(
            &format!("mutation($n: String!) {{ editAkt(input: {{ number: $n, revision: 1, fields: {{ severity: \"minor\", owner: \"{SUBJECT}\" }}, reason: \"Triaged\" }}) {{ {AKT} }} }}"),
            json!({ "n": number }),
        )
        .await;
    assert_eq!(edited["editAkt"]["revision"], 2);

    let revisions = world
        .data(
            "query($n: String!) { akt(number: $n) { revisions { revision author transition reason changes state hash } } }",
            json!({ "n": number }),
        )
        .await["akt"]["revisions"]
        .clone();
    assert_eq!(revisions[0]["revision"], 1);
    assert_eq!(revisions[0]["changes"], json!({ "title": "Missed review" }));
    assert_eq!(revisions[1]["author"], SUBJECT);
    assert_eq!(revisions[1]["reason"], "Triaged");
    assert_eq!(revisions[1]["transition"], Value::Null);
    assert_eq!(
        revisions[1]["changes"],
        json!({ "owner": SUBJECT, "severity": "minor" })
    );
    // The hash covers the whole resulting state, as canonical JSON.
    let canonical = format!(
        r#"{{"fields":{{"owner":"{SUBJECT}","severity":"minor","title":"Missed review"}},"state":"open"}}"#
    );
    let expected = Sha256::digest(canonical.as_bytes())
        .iter()
        .fold(String::new(), |mut hex, b| {
            use std::fmt::Write;
            let _ = write!(hex, "{b:02x}");
            hex
        });
    assert_eq!(revisions[1]["hash"], expected);
}

// r[verify akt.revision.concurrency]
#[tokio::test(flavor = "multi_thread")]
async fn refuses_a_change_based_on_an_old_revision() {
    let world = World::new().await;
    world.define(capa()).await;
    let number = world.create(json!({ "title": "T" })).await["number"]
        .as_str()
        .unwrap()
        .to_owned();
    let edit = |revision: i32, title: &str| {
        format!(
            "mutation {{ editAkt(input: {{ number: \"{number}\", revision: {revision}, fields: {{ title: \"{title}\" }} }}) {{ revision }} }}"
        )
    };
    assert_eq!(
        world.data(&edit(1, "U"), json!({})).await["editAkt"]["revision"],
        2
    );
    let stale = world.run(&edit(1, "V"), json!({})).await;
    assert_eq!(code(&stale), "STALE");
    assert_eq!(stale["errors"][0]["extensions"]["currentRevision"], 2);
    assert_eq!(world.revision_rows().await, 2);
}

// r[verify akt.transition]
#[tokio::test(flavor = "multi_thread")]
async fn transitions_with_their_required_fields_then_refuses_edits() {
    let world = World::new().await;
    world.define(capa()).await;
    let number = world.create(json!({ "title": "T" })).await["number"]
        .as_str()
        .unwrap()
        .to_owned();
    let transition = |fields: &str| {
        format!(
            "mutation {{ performTransition(input: {{ number: \"{number}\", revision: 1, transition: \"close\", fields: {fields} }}) {{ state {{ key final }} revision }} }}"
        )
    };
    let missing = world.run(&transition("{}"), json!({})).await;
    assert_eq!(code(&missing), "REFUSED");
    let closed = world
        .data(&transition("{ root_cause: \"No reviewer\" }"), json!({}))
        .await;
    assert_eq!(
        closed["performTransition"],
        json!({ "state": { "key": "closed", "final": true }, "revision": 2 })
    );

    let edit = world
        .run(
            &format!("mutation {{ editAkt(input: {{ number: \"{number}\", revision: 2, fields: {{ title: \"U\" }} }}) {{ revision }} }}"),
            json!({}),
        )
        .await;
    assert_eq!(code(&edit), "REFUSED");
}

// r[verify akt.version-pinned]
#[tokio::test(flavor = "multi_thread")]
async fn an_akt_keeps_the_version_it_was_created_with() {
    let world = World::new().await;
    world.define(capa()).await;
    let number = world.create(json!({ "title": "T" })).await["number"]
        .as_str()
        .unwrap()
        .to_owned();
    // Version 2 drops the severity field.
    let mut v2 = capa();
    v2["fields"].as_array_mut().unwrap().remove(1);
    world.define(v2).await;

    let edited = world
        .data(
            &format!("mutation {{ editAkt(input: {{ number: \"{number}\", revision: 1, fields: {{ severity: \"major\" }} }}) {{ type {{ version }} }} }}"),
            json!({}),
        )
        .await;
    assert_eq!(edited["editAkt"]["type"]["version"], 1);
    let new = world
        .run(
            "mutation { createAkt(input: { type: \"capa\", fields: { title: \"T\", severity: \"major\" } }) { number } }",
            json!({}),
        )
        .await;
    assert_eq!(code(&new), "REFUSED");
}

// r[verify akt.revision.record]
#[tokio::test(flavor = "multi_thread")]
async fn revisions_cannot_be_changed_or_removed() {
    let world = World::new().await;
    world.define(capa()).await;
    world.create(json!({ "title": "T" })).await;
    let mut conn = world.db.pool().get().await.unwrap();
    let update = diesel::update(akt_revision::table)
        .set(akt_revision::reason.eq("rewritten"))
        .execute(&mut conn)
        .await;
    assert!(update.is_err(), "an update was allowed");
    let delete = diesel::delete(akt_revision::table).execute(&mut conn).await;
    assert!(delete.is_err(), "a delete was allowed");
}

// r[verify akt.change-event]
// r[verify akt.revision.atomic]
#[tokio::test(flavor = "multi_thread")]
async fn every_revision_has_a_change_event_in_order() {
    let world = World::new().await;
    world.define(capa()).await;
    let number = world.create(json!({ "title": "T" })).await["number"]
        .as_str()
        .unwrap()
        .to_owned();
    world.create(json!({ "title": "U" })).await;
    world
        .data(
            &format!("mutation {{ editAkt(input: {{ number: \"{number}\", revision: 1, fields: {{ title: \"V\" }} }}) {{ revision }} }}"),
            json!({}),
        )
        .await;
    // A refused change leaves neither a revision nor an event.
    world
        .run(
            &format!("mutation {{ editAkt(input: {{ number: \"{number}\", revision: 1, fields: {{ title: \"W\" }} }}) {{ revision }} }}"),
            json!({}),
        )
        .await;

    let mut conn = world.db.pool().get().await.unwrap();
    let events: Vec<(i64, Option<i32>)> = change_event::table
        .order(change_event::seq)
        .select((change_event::seq, change_event::revision))
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(events, vec![(1, Some(1)), (2, Some(1)), (3, Some(2))]);
    assert_eq!(world.revision_rows().await, 3);
}

// r[verify akt.number.sequence]
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_creates_get_distinct_numbers() {
    let world = World::new().await;
    world.define(capa()).await;
    let mut tasks = Vec::new();
    for i in 0..10 {
        let router = world.router.clone();
        tasks.push(tokio::spawn(async move {
            send(
                &router,
                graphql_with(
                    "tenant",
                    "mutation($f: JSON) { createAkt(input: { type: \"capa\", fields: $f }) { number } }",
                    &json!({ "f": { "title": format!("Akt {i}") } }),
                ),
            )
            .await
            .json()["data"]["createAkt"]["number"]
                .as_str()
                .unwrap()
                .to_owned()
        }));
    }
    let mut numbers = Vec::new();
    for task in tasks {
        numbers.push(task.await.unwrap());
    }
    numbers.sort();
    let expected: Vec<String> = (1..=10)
        .map(|n| format!("CAPA-{}-{n:04}", year()))
        .collect();
    assert_eq!(numbers, expected);
}

// r[verify akt.number.random]
#[tokio::test(flavor = "multi_thread")]
async fn random_numbers_are_free_until_none_is() {
    let world = World::new().await;
    let mut digit = capa();
    digit["number"] = json!("QR-{rand:1}");
    world.define(digit).await;
    // Created at once, ten akter take the ten numbers `QR-{rand:1}` gives.
    let mut tasks = Vec::new();
    for i in 0..10 {
        let router = world.router.clone();
        tasks.push(tokio::spawn(async move {
            send(
                &router,
                graphql_with(
                    "tenant",
                    "mutation($f: JSON) { createAkt(input: { type: \"capa\", fields: $f }) { number } }",
                    &json!({ "f": { "title": format!("Akt {i}") } }),
                ),
            )
            .await
            .json()["data"]["createAkt"]["number"]
                .as_str()
                .unwrap()
                .to_owned()
        }));
    }
    let mut numbers = Vec::new();
    for task in tasks {
        numbers.push(task.await.unwrap());
    }
    numbers.sort();
    let every: Vec<String> = (0..10).map(|n| format!("QR-{n}")).collect();
    assert_eq!(numbers, every);
    let refused = world
        .run(
            "mutation { createAkt(input: { type: \"capa\", fields: { title: \"One more\" } }) { number } }",
            json!({}),
        )
        .await;
    assert_eq!(code(&refused), "NUMBERS_EXHAUSTED", "{refused}");
    assert!(
        refused["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("QR-{rand:1}"),
        "{refused}"
    );

    // Beside a sequence, the drawn digits count on their own.
    let mut both = capa();
    both["number"] = json!("LOT-{rand:3}-{seq:2}");
    world.define(both).await;
    let number = world.create(json!({ "title": "Lot" })).await["number"]
        .as_str()
        .unwrap()
        .to_owned();
    let (digits, sequence) = number["LOT-".len()..].split_once('-').unwrap();
    assert_eq!(digits.len(), 3, "{number}");
    assert!(digits.bytes().all(|b| b.is_ascii_digit()), "{number}");
    assert_eq!(sequence, "01", "{number}");
}

// r[verify akt.number.shared]
#[tokio::test(flavor = "multi_thread")]
async fn akt_types_naming_one_sequence_share_its_numbers() {
    let world = World::new().await;
    let named = |key: &str, sequence: Option<&str>| {
        let mut ty = capa();
        ty["key"] = json!(key);
        ty["number"] = json!("QMS-{seq:4}");
        if let Some(sequence) = sequence {
            ty["sequence"] = json!(sequence);
        }
        ty
    };
    world.define(named("capa", Some("qms"))).await;
    world.define(named("procedure", Some("qms"))).await;
    let create = |key: &'static str| {
        let world = &world;
        async move {
            world
                .data(
                    "mutation($t: String!) { createAkt(input: { type: $t, fields: { title: \"T\" } }) { number } }",
                    json!({ "t": key }),
                )
                .await["createAkt"]["number"]
                .as_str()
                .unwrap()
                .to_owned()
        }
    };
    assert_eq!(create("capa").await, "QMS-0001");
    assert_eq!(create("procedure").await, "QMS-0002");
    assert_eq!(create("capa").await, "QMS-0003");

    // The same pattern without the sequence is refused, as is the sequence
    // with another pattern.
    let refused = world
        .run(
            "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
            json!({ "d": named("form", None) }),
        )
        .await;
    assert_eq!(code(&refused), "INVALID_AKT_TYPE", "{refused}");
    assert_eq!(
        refused["errors"][0]["extensions"]["problems"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mut elsewhere = named("form", Some("qms"));
    elsewhere["number"] = json!("FORM-{seq:4}");
    let refused = world
        .run(
            "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
            json!({ "d": elsewhere }),
        )
        .await;
    assert_eq!(
        refused["errors"][0]["extensions"]["problems"][0]["path"], "sequence",
        "{refused}"
    );
}

async fn record_person(world: &World, name: Option<&str>, email: Option<&str>) {
    let mut conn = world.db.pool().get().await.unwrap();
    kuria_store::people::record(
        &mut conn,
        &kuria_store::people::Person {
            subject_id: SUBJECT.parse().unwrap(),
            name: name.map(str::to_owned),
            email: email.map(str::to_owned),
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();
}

// r[verify person.name]
#[tokio::test(flavor = "multi_thread")]
async fn shows_users_by_name() {
    let world = World::new().await;
    world.define(capa()).await;
    let query = format!(
        "mutation {{ createAkt(input: {{ type: \"capa\", fields: {{ title: \"T\", owner: \"{SUBJECT}\" }} }}) {{ number fields {{ ... on UserValue {{ field user {{ ... on User {{ id name email }} }} }} }} revisions {{ by {{ name }} }} }} }}"
    );
    // Never signed in: shown by identifier.
    let created = world.data(&query, json!({})).await["createAkt"].clone();
    let owner = created["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["field"] == "owner")
        .unwrap();
    assert_eq!(
        owner["user"],
        json!({ "id": SUBJECT, "name": SUBJECT, "email": null })
    );

    record_person(&world, Some("Bob Builder"), Some("bob@example.com")).await;
    let number = created["number"].as_str().unwrap();
    let data = world
        .data(
            "query($n: String!) { akt(number: $n) { fields { ... on UserValue { user { ... on User { name email } } } } revisions { by { name } } } }",
            json!({ "n": number }),
        )
        .await;
    let fields = data["akt"]["fields"].as_array().unwrap();
    let owner = fields.iter().find(|f| f.get("user").is_some()).unwrap();
    assert_eq!(
        owner["user"],
        json!({ "name": "Bob Builder", "email": "bob@example.com" })
    );
    assert_eq!(data["akt"]["revisions"][0]["by"]["name"], "Bob Builder");
}

// r[verify akt.field.checkbox]
// r[verify akt.field.kinds]
#[tokio::test(flavor = "multi_thread")]
async fn checkboxes_record_the_caller_and_git_and_images_round_trip() {
    let world = World::new().await;
    world
        .define(json!({
            "key": "build", "name": "Build", "number": "BUILD-{seq}",
            "fields": [
                { "key": "verified", "name": "Verified", "kind": "checkbox" },
                { "key": "source", "name": "Source", "kind": "git" },
                { "key": "image", "name": "Image", "kind": "docker" }
            ],
            "states": [{ "key": "open", "name": "Open", "initial": true }]
        }))
        .await;
    record_person(&world, Some("Alice Example"), None).await;
    let digest = format!("sha256:{}", "ab".repeat(32));
    let fields = "fields { __typename ... on CheckboxValue { field checked by { name } at } ... on GitValue { repository sha tag } ... on DockerValue { repository digest tag } }";
    let created = world
        .data(
            &format!("mutation($f: JSON) {{ createAkt(input: {{ type: \"build\", fields: $f }}) {{ number revision {fields} }} }}"),
            json!({ "f": {
                "verified": true,
                "source": { "repository": "git@git.example:team/app.git", "tag": "v4.2.0" },
                "image": { "repository": "registry.example/team/app", "digest": digest, "tag": "4.2.0" }
            } }),
        )
        .await["createAkt"]
        .clone();
    let values = created["fields"].as_array().unwrap();
    let checkbox = values
        .iter()
        .find(|v| v["__typename"] == "CheckboxValue")
        .unwrap();
    assert_eq!(checkbox["checked"], true);
    assert_eq!(checkbox["by"]["name"], "Alice Example");
    let checked_at = checkbox["at"].clone();
    assert!(checked_at.is_string());
    assert!(values.contains(&json!({ "__typename": "GitValue", "repository": "git@git.example:team/app.git", "sha": null, "tag": "v4.2.0" })));
    assert!(values.contains(&json!({ "__typename": "DockerValue", "repository": "registry.example/team/app", "digest": digest, "tag": "4.2.0" })));

    // Checking it again changes nothing; clearing it does.
    let number = created["number"].as_str().unwrap();
    let edit = |value: bool, revision: i64| {
        format!(
            "mutation {{ editAkt(input: {{ number: \"{number}\", revision: {revision}, fields: {{ verified: {value} }} }}) {{ revision {fields} }} }}"
        )
    };
    let again = world.run(&edit(true, 1), json!({})).await;
    assert_eq!(code(&again), "REFUSED", "an edit that changes nothing");
    let cleared = world.data(&edit(false, 1), json!({})).await;
    assert!(
        !cleared["editAkt"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["__typename"] == "CheckboxValue")
    );
}
