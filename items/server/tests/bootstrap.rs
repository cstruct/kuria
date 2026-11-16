//! Bootstrapping: a bundle imported into a fresh tenant as the server
//! starts, and refused into one that isn't (`bundle.bootstrap.*`).

mod support;

use kuria_server::bundles::{Bootstrap, BundleError, bootstrap};
use kuria_store::people::{self, Person};
use serde_json::{Value, json};
use uuid::Uuid;

use support::decider::FakeDecider;
use support::world::{World, scratch, write_bundle};
use support::{COLLEAGUE, OTHER_TENANT, SUBJECT, TENANT};

fn note() -> Value {
    json!({
        "key": "note", "name": "Note", "number": "NOTE-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["open"], "to": "done" }]
    })
}

fn decider() -> std::sync::Arc<FakeDecider> {
    let decider = FakeDecider::allowing_tenant();
    decider.allow_everything("other-tenant");
    decider.allow_everything("colleague");
    decider
}

/// A bundle of notes, two of them, written to a directory.
async fn notes() -> std::path::PathBuf {
    let source = World::new(decider()).await;
    record(&source, SUBJECT, "Ada Lovelace").await;
    source.define("tenant", &note()).await;
    let first = source
        .create(
            "tenant",
            "note",
            &[],
            json!({ "title": "Buy milk" }),
            "Shopping",
        )
        .await;
    source
        .create(
            "tenant",
            "note",
            &[],
            json!({ "title": "Call Ada" }),
            "Calls",
        )
        .await;
    source
        .transition("tenant", &first, "finish", None, "Bought it")
        .await;
    let dir = scratch("notes").join("bundle");
    write_bundle(
        &source.export("tenant", "types=note&akter=true").await,
        &dir,
    );
    dir
}

async fn record(world: &World, subject: &str, name: &str) {
    let mut conn = world.db.pool().get().await.unwrap();
    people::record(
        &mut conn,
        &Person {
            subject_id: subject.parse().unwrap(),
            name: Some(name.to_owned()),
            email: None,
            updated_at: chrono::Utc::now(),
        },
    )
    .await
    .unwrap();
}

async fn author(world: &World, number: &str) -> String {
    world
        .ok(
            "colleague",
            "query($n: String!) { akt(number: $n) { revisions { by { name } } } }",
            json!({ "n": number }),
        )
        .await["akt"]["revisions"][0]["by"]["name"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn into(bundle: &std::path::Path, tenant: &str) -> Bootstrap {
    Bootstrap {
        bundle: bundle.to_owned(),
        tenant: tenant.parse().unwrap(),
        subjects: None,
    }
}

async fn titles(world: &World, token: &str) -> Vec<(String, String, String)> {
    let mut titles: Vec<_> = world
        .ok(
            token,
            "{ akter(type: \"note\") { nodes { number title state { key } } } }",
            json!({}),
        )
        .await["akter"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["number"].as_str().unwrap().to_owned(),
                a["title"].as_str().unwrap().to_owned(),
                a["state"]["key"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    titles.sort();
    titles
}

// r[verify bundle.bootstrap.import]
// r[verify bundle.bootstrap.fresh]
// r[verify bundle.bootstrap.recorded]
#[tokio::test(flavor = "multi_thread")]
async fn a_fresh_tenant_starts_with_the_bundle() {
    let bundle = notes().await;
    let target = World::new(decider()).await;
    let imported = bootstrap(target.db.pool(), &target.storage, &into(&bundle, TENANT))
        .await
        .unwrap();
    assert_eq!((imported.akt_types, imported.akter), (1, 2));
    assert_eq!(
        titles(&target, "tenant").await,
        [
            (
                "NOTE-1".to_owned(),
                "Buy milk".to_owned(),
                "done".to_owned()
            ),
            (
                "NOTE-2".to_owned(),
                "Call Ada".to_owned(),
                "open".to_owned()
            ),
        ]
    );
    // Numbered after the bundle's, and recorded as imported by no one.
    let next = target
        .create(
            "tenant",
            "note",
            &[],
            json!({ "title": "Water plants" }),
            "Garden",
        )
        .await;
    assert_eq!(next, "NOTE-3");
    let mut conn = target.db.pool().get().await.unwrap();
    let by: Vec<Uuid> = {
        use diesel::sql_query;
        use diesel::sql_types::Uuid as SqlUuid;
        use diesel_async::RunQueryDsl as _;
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = SqlUuid)]
            imported_by: Uuid,
        }
        sql_query("select imported_by from bundle_import")
            .load::<Row>(&mut conn)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.imported_by)
            .collect()
    };
    assert_eq!(by, [Uuid::nil()]);

    // From its tar archive, too.
    let archive = target.export("tenant", "types=note&akter=true").await;
    let file = scratch("archive").join("notes.tar");
    std::fs::write(&file, archive).unwrap();
    let other = World::new(decider()).await;
    bootstrap(other.db.pool(), &other.storage, &into(&file, TENANT))
        .await
        .unwrap();
    assert_eq!(titles(&other, "tenant").await.len(), 3);
}

// r[verify bundle.bootstrap.fresh]
#[tokio::test(flavor = "multi_thread")]
async fn a_tenant_with_data_refuses_and_keeps_what_it_has() {
    let bundle = notes().await;
    let target = World::new(decider()).await;
    target.define("tenant", &note()).await;
    target
        .create(
            "tenant",
            "note",
            &[],
            json!({ "title": "Mine" }),
            "Already here",
        )
        .await;
    let Err(refused) = bootstrap(target.db.pool(), &target.storage, &into(&bundle, TENANT)).await
    else {
        panic!("bootstrapped into a tenant holding akter");
    };
    assert!(matches!(refused, BundleError::NotFresh(t) if t.to_string() == TENANT));
    let message = refused.to_string();
    assert!(
        message.contains(TENANT) && message.contains("KURIA_BOOTSTRAP_BUNDLE"),
        "{message}"
    );
    assert_eq!(
        titles(&target, "tenant").await,
        [("NOTE-1".to_owned(), "Mine".to_owned(), "open".to_owned())]
    );

    // A second start with the variable still set refuses as well.
    let fresh = World::new(decider()).await;
    bootstrap(fresh.db.pool(), &fresh.storage, &into(&bundle, TENANT))
        .await
        .unwrap();
    assert!(matches!(
        bootstrap(fresh.db.pool(), &fresh.storage, &into(&bundle, TENANT)).await,
        Err(BundleError::NotFresh(_))
    ));
    assert_eq!(titles(&fresh, "tenant").await.len(), 2);

    // A contact alone makes a tenant not fresh.
    let contacts = World::new(decider()).await;
    contacts
        .ok(
            "tenant",
            "mutation { addContact(input: { name: \"Ada\", email: \"ada@example.com\" }) { id } }",
            json!({}),
        )
        .await;
    assert!(matches!(
        bootstrap(
            contacts.db.pool(),
            &contacts.storage,
            &into(&bundle, TENANT)
        )
        .await,
        Err(BundleError::NotFresh(_))
    ));
}

// r[verify bundle.bootstrap.fresh]
#[tokio::test(flavor = "multi_thread")]
async fn other_tenants_data_does_not_count() {
    let bundle = notes().await;
    let target = World::new(decider()).await;
    target.define("other-tenant", &note()).await;
    target
        .create(
            "other-tenant",
            "note",
            &[],
            json!({ "title": "Theirs" }),
            "Elsewhere",
        )
        .await;
    bootstrap(target.db.pool(), &target.storage, &into(&bundle, TENANT))
        .await
        .unwrap();
    assert_eq!(titles(&target, "tenant").await.len(), 2);
    assert_eq!(titles(&target, "other-tenant").await.len(), 1);
    assert!(matches!(
        bootstrap(
            target.db.pool(),
            &target.storage,
            &into(&bundle, OTHER_TENANT)
        )
        .await,
        Err(BundleError::NotFresh(_))
    ));
}

// r[verify bundle.bootstrap.variables]
// r[verify bundle.bootstrap.import]
// r[verify bundle.import.subjects]
#[tokio::test(flavor = "multi_thread")]
async fn maps_subjects_and_fails_on_a_bundle_it_cannot_read() {
    let bundle = notes().await;
    let target = World::new(decider()).await;
    let subjects = scratch("subjects").join("subjects.json");
    std::fs::write(&subjects, json!({ SUBJECT: COLLEAGUE }).to_string()).unwrap();
    let imported = bootstrap(
        target.db.pool(),
        &target.storage,
        &Bootstrap {
            subjects: Some(subjects),
            ..into(&bundle, TENANT)
        },
    )
    .await
    .unwrap();
    assert_eq!(imported.aliases, 1);
    // Shown by the bundle's name until the user it maps to signs in.
    assert_eq!(author(&target, "NOTE-1").await, "Ada Lovelace");
    record(&target, COLLEAGUE, "Ada King").await;
    assert_eq!(author(&target, "NOTE-1").await, "Ada King");

    let empty = World::new(decider()).await;
    let missing = scratch("missing").join("bundle");
    assert!(matches!(
        bootstrap(empty.db.pool(), &empty.storage, &into(&missing, TENANT)).await,
        Err(BundleError::Invalid(_))
    ));
    // A bundle the import refuses is refused the same way.
    let broken = scratch("broken");
    std::fs::write(broken.join("bundle.json"), r#"{"format": 99}"#).unwrap();
    let Err(BundleError::Invalid(message)) =
        bootstrap(empty.db.pool(), &empty.storage, &into(&broken, TENANT)).await
    else {
        panic!("bootstrapped an unknown format");
    };
    assert!(message.contains("format is 99"), "{message}");
    assert_eq!(titles(&empty, "tenant").await.len(), 0);
}
