//! Contacts through GraphQL, against a fresh database for each test and a
//! fake relying host.

mod support;

use std::sync::Arc;

use axum::Router;
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use kuria_server::authorize::{MANAGE_CONTACTS, READ_CONTACTS};
use kuria_store::schema::contact;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};
use uuid::Uuid;

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

const MANAGER: &str = "tenant";
const READER: &str = "tenant-reader";
const NOBODY: &str = "tenant-nobody";
const OTHER: &str = "other-tenant";

const CONTACT: &str = "id displayName name email user { id } createdBy { id }";

struct World {
    db: TestDatabase,
    router: Router,
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        decider.grant(READER, &[READ_CONTACTS]);
        decider.grant(OTHER, &[MANAGE_CONTACTS]);
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        Self { db, router }
    }

    async fn run(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = send(&self.router, graphql_with(token, query, &variables)).await;
        assert_eq!(answer.status, 200, "{}", answer.body);
        answer.json()
    }

    async fn add(&self, token: &str, name: &str, email: Option<&str>) -> Value {
        self.run(
            token,
            &format!("mutation($n: String!, $e: String) {{ addContact(input: {{ name: $n, email: $e }}) {{ {CONTACT} }} }}"),
            json!({ "n": name, "e": email }),
        )
        .await
    }

    async fn added(&self, name: &str, email: Option<&str>) -> Value {
        let answer = self.add(MANAGER, name, email).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"]["addContact"].clone()
    }

    async fn edit(&self, id: &Value, name: &str, email: Option<&str>) -> Value {
        self.run(
            MANAGER,
            &format!("mutation($i: ID!, $n: String!, $e: String) {{ editContact(input: {{ id: $i, name: $n, email: $e }}) {{ {CONTACT} }} }}"),
            json!({ "i": id, "n": name, "e": email }),
        )
        .await
    }

    async fn list(&self, token: &str) -> Value {
        self.run(token, &format!("{{ contacts {{ {CONTACT} }} }}"), json!({}))
            .await
    }

    /// Links the contact `id` to `subject`, as a sign-in will.
    async fn link(&self, id: &Value, subject: Uuid) {
        let id = Uuid::parse_str(id.as_str().unwrap()).unwrap();
        let mut conn = self.db.pool().get().await.unwrap();
        diesel::update(contact::table.filter(contact::id.eq(id)))
            .set((
                contact::linked_subject.eq(Some(subject)),
                contact::linked_at.eq(Some(Utc::now())),
            ))
            .execute(&mut conn)
            .await
            .unwrap();
    }
}

fn code(answer: &Value) -> &str {
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
}

// r[verify contact.add]
// r[verify contact.list]
#[tokio::test(flavor = "multi_thread")]
async fn managers_add_contacts_and_readers_list_them() {
    let world = World::new().await;
    let bob = world
        .added(" Bob Builder ", Some("bob@supplier.example"))
        .await;
    assert_eq!(bob["name"], "Bob Builder");
    assert_eq!(bob["displayName"], "Bob Builder");
    assert_eq!(bob["email"], "bob@supplier.example");
    assert_eq!(bob["user"], Value::Null);
    assert_eq!(bob["createdBy"]["id"], support::SUBJECT);
    world.added("Anna", None).await;

    let listed = world.list(READER).await;
    assert!(listed.get("errors").is_none(), "{listed}");
    let names: Vec<_> = listed["data"]["contacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["name"].clone(), c["email"].clone()))
        .collect();
    assert_eq!(
        names,
        [
            (json!("Anna"), Value::Null),
            (json!("Bob Builder"), json!("bob@supplier.example"))
        ]
    );

    assert_eq!(code(&world.list(NOBODY).await), "FORBIDDEN");
    assert_eq!(code(&world.add(READER, "Carl", None).await), "FORBIDDEN");
    assert_eq!(code(&world.add(NOBODY, "Carl", None).await), "FORBIDDEN");
}

// r[verify contact.add]
#[tokio::test(flavor = "multi_thread")]
async fn a_contact_needs_a_name_and_a_real_email() {
    let world = World::new().await;
    for (name, email) in [
        (" ", None),
        ("Bob", Some("bob")),
        ("Bob", Some("b ob@x.example")),
    ] {
        assert_eq!(
            code(&world.add(MANAGER, name, email).await),
            "INVALID_CONTACT",
            "{name:?} {email:?}"
        );
    }
    let blank = world.added("Bob", Some("  ")).await;
    assert_eq!(blank["email"], Value::Null);
}

// r[verify contact.unique-email]
#[tokio::test(flavor = "multi_thread")]
async fn one_contact_per_email_per_tenant() {
    let world = World::new().await;
    world.added("Bob", Some("bob@supplier.example")).await;
    assert_eq!(
        code(
            &world
                .add(MANAGER, "Robert", Some("BOB@Supplier.example"))
                .await
        ),
        "DUPLICATE_EMAIL"
    );
    let anna = world.added("Anna", Some("anna@supplier.example")).await;
    assert_eq!(
        code(
            &world
                .edit(&anna["id"], "Anna", Some("Bob@supplier.example"))
                .await
        ),
        "DUPLICATE_EMAIL"
    );
    // Contacts without an email don't collide.
    world.added("Carl", None).await;
    world.added("Dora", None).await;
    // Another tenant has its own contacts.
    let other = world
        .run(
            OTHER,
            "mutation { addContact(input: { name: \"Bob\", email: \"bob@supplier.example\" }) { id } }",
            json!({}),
        )
        .await;
    assert!(other.get("errors").is_none(), "{other}");
    let theirs = world.list(OTHER).await;
    assert_eq!(theirs["data"]["contacts"].as_array().unwrap().len(), 1);
}

// r[verify contact.edit]
#[tokio::test(flavor = "multi_thread")]
async fn contacts_are_edited_until_they_are_linked() {
    let world = World::new().await;
    let bob = world.added("Bob", Some("bob@supplier.example")).await;
    let edited = world
        .edit(&bob["id"], "Bob Builder", Some("bob@builder.example"))
        .await;
    assert!(edited.get("errors").is_none(), "{edited}");
    assert_eq!(edited["data"]["editContact"]["name"], "Bob Builder");
    assert_eq!(
        edited["data"]["editContact"]["email"],
        "bob@builder.example"
    );
    // Keeping its own email is no collision.
    let same = world
        .edit(&bob["id"], "Bob", Some("bob@builder.example"))
        .await;
    assert!(same.get("errors").is_none(), "{same}");

    world.link(&bob["id"], Uuid::now_v7()).await;
    assert_eq!(
        code(&world.edit(&bob["id"], "Robert", None).await),
        "LINKED"
    );
    assert_eq!(
        code(
            &world
                .edit(&json!(Uuid::now_v7().to_string()), "X", None)
                .await
        ),
        "NOT_FOUND"
    );
    assert_eq!(
        code(&world.edit(&json!("nope"), "X", None).await),
        "NOT_FOUND"
    );
}

// r[verify contact.show]
#[tokio::test(flavor = "multi_thread")]
async fn a_linked_contact_is_shown_as_its_user() {
    let world = World::new().await;
    let bob = world.added("Bob", Some("bob@supplier.example")).await;
    let subject = Uuid::now_v7();
    world.link(&bob["id"], subject).await;
    // The user hasn't been recorded with a name yet: the contact's own.
    let listed = world.list(MANAGER).await;
    assert_eq!(listed["data"]["contacts"][0]["displayName"], "Bob");
    assert_eq!(
        listed["data"]["contacts"][0]["user"]["id"],
        subject.to_string()
    );

    let mut conn = world.db.pool().get().await.unwrap();
    kuria_store::people::record(
        &mut conn,
        &kuria_store::people::Person {
            subject_id: subject,
            name: Some("Robert Builder".to_owned()),
            email: None,
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();
    let listed = world.list(MANAGER).await;
    assert_eq!(
        listed["data"]["contacts"][0]["displayName"],
        "Robert Builder"
    );
}

// r[verify contact.erase]
#[tokio::test(flavor = "multi_thread")]
async fn erasing_a_contact_keeps_only_its_identifier() {
    let world = World::new().await;
    let bob = world.added("Bob", Some("bob@supplier.example")).await;
    let erase = |token: &'static str, id: Value| {
        let world = &world;
        async move {
            world
                .run(
                    token,
                    &format!("mutation($i: ID!) {{ eraseContact(id: $i) {{ {CONTACT} }} }}"),
                    json!({ "i": id }),
                )
                .await
        }
    };
    assert_eq!(code(&erase(READER, bob["id"].clone()).await), "FORBIDDEN");
    let erased = erase(MANAGER, bob["id"].clone()).await;
    assert!(erased.get("errors").is_none(), "{erased}");
    let erased = &erased["data"]["eraseContact"];
    assert_eq!(erased["name"], Value::Null);
    assert_eq!(erased["email"], Value::Null);
    assert_eq!(erased["displayName"], bob["id"]);
    // Its email is free again.
    world.added("Robert", Some("bob@supplier.example")).await;
    assert_eq!(
        code(&erase(MANAGER, json!(Uuid::now_v7().to_string())).await),
        "NOT_FOUND"
    );
}
