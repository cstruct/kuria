//! Containers, memberships, creating inside containers and moves, through
//! GraphQL against a fresh database, as a subject allowed everything.

mod support;

use axum::Router;
use chrono::{Datelike, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use kuria_store::schema::change_event;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

struct World {
    db: TestDatabase,
    router: Router,
    decider: std::sync::Arc<FakeDecider>,
}

const ALICE: &str = "tenant-alice";

fn states(final_state: &str) -> Value {
    json!([
        { "key": "open", "name": "Open", "initial": true },
        { "key": final_state, "name": "Done", "final": true }
    ])
}

fn finish(final_state: &str) -> Value {
    json!([{ "key": "finish", "name": "Finish", "from": ["open"], "to": final_state }])
}

/// The akt types the tests use.
fn types() -> Vec<Value> {
    let title = json!([{ "key": "title", "name": "Title", "kind": "text" }]);
    vec![
        json!({ "key": "item", "name": "Item", "number": "ITEM-{seq}", "fields": title,
                "states": states("done"), "transitions": finish("done") }),
        json!({ "key": "epic", "name": "Epic", "number": "EPIC-{seq}", "fields": title,
                "states": states("done"), "transitions": finish("done"),
                "contains": [{ "type": "item" }] }),
        json!({ "key": "backlog", "name": "Backlog", "number": "BACKLOG-{seq}", "fields": title,
                "states": states("closed"), "transitions": finish("closed"),
                "contains": [{ "type": "item", "exclusive": true }, { "type": "epic" }] }),
        json!({ "key": "group", "name": "Group", "number": "GROUP-{seq}", "fields": title,
                "states": states("closed"), "contains": [{ "type": "group" }] }),
        json!({ "key": "release", "name": "Release", "number": "RELEASE-{year}-{seq:4}",
                "fields": title, "states": states("released"), "transitions": finish("released"),
                "contains": [{ "type": "backlog" }, { "type": "release_candidate" },
                             { "type": "hotfix" }] }),
        json!({ "key": "release_candidate", "name": "Release candidate",
                "number": "{parent}-RC-{seq:4}", "created_in": ["release"], "fields": title,
                "states": states("accepted"), "contains": [{ "type": "epic" }, { "type": "item" }] }),
        json!({ "key": "sprint", "name": "Sprint", "number": "SPRINT-{seq}", "fields": title,
                "states": [
                    { "key": "planning", "name": "Planning", "initial": true },
                    { "key": "active", "name": "Active" },
                    { "key": "done", "name": "Done", "final": true }
                ],
                "transitions": [{ "key": "start", "name": "Start", "from": ["planning"], "to": "active" }],
                "contains": [{ "type": "item", "while": ["planning"] }] }),
        json!({ "key": "hotfix", "name": "Hotfix", "number": "{parent}-HOTFIX-{seq:4}",
                "created_in": ["release"], "fields": title, "states": states("deployed"),
                "contains": [{ "type": "item" }] }),
    ]
}

impl World {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider = FakeDecider::allowing_tenant();
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider.clone());
        let world = Self {
            db,
            router,
            decider,
        };
        for document in types() {
            world
                .ok(
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": document }),
                )
                .await;
        }
        world
    }

    async fn run(&self, query: &str, variables: Value) -> Value {
        self.run_as("tenant", query, variables).await
    }

    async fn run_as(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = send(&self.router, graphql_with(token, query, &variables)).await;
        assert_eq!(answer.status, 200, "{}", answer.body);
        answer.json()
    }

    async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = self.run(query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    /// Creates an akt of `key`, inside `inside`, and returns its number.
    async fn create(&self, key: &str, inside: &[&str]) -> String {
        self.try_create(key, inside).await["data"]["createAkt"]["number"]
            .as_str()
            .unwrap_or_else(|| panic!("creating a {key}"))
            .to_owned()
    }

    async fn try_create(&self, key: &str, inside: &[&str]) -> Value {
        self.run(
            "mutation($t: String!, $in: [String!]!) { createAkt(input: { type: $t, in: $in, fields: { title: \"T\" } }) { number } }",
            json!({ "t": key, "in": inside }),
        )
        .await
    }

    async fn add(&self, container: &str, member: &str) -> Value {
        self.run(
            "mutation($c: String!, $m: String!) { addToContainer(input: { container: $c, member: $m, reason: \"Planned\" }) { number } }",
            json!({ "c": container, "m": member }),
        )
        .await
    }

    async fn remove(&self, container: &str, member: &str) -> Value {
        self.run(
            "mutation($c: String!, $m: String!) { removeFromContainer(input: { container: $c, member: $m }) { number } }",
            json!({ "c": container, "m": member }),
        )
        .await
    }

    async fn moved(&self, number: &str, place: &str, other: &str) -> Value {
        self.run(
            &format!("mutation($n: String!, $o: String!) {{ moveAkt(input: {{ number: $n, {place}: $o }}) {{ number }} }}"),
            json!({ "n": number, "o": other }),
        )
        .await
    }

    async fn members(&self, container: &str) -> Vec<String> {
        let data = self
            .ok(
                "query($n: String!) { akt(number: $n) { members { number } } }",
                json!({ "n": container }),
            )
            .await;
        numbers(&data["akt"]["members"])
    }

    async fn containers(&self, number: &str) -> Vec<String> {
        let data = self
            .ok(
                "query($n: String!) { akt(number: $n) { containers { number } } }",
                json!({ "n": number }),
            )
            .await;
        numbers(&data["akt"]["containers"])
    }

    async fn revisions(&self, number: &str) -> Value {
        self.ok(
            "query($n: String!) { akt(number: $n) { revision revisions { revision reason membership hash } } }",
            json!({ "n": number }),
        )
        .await["akt"]
            .clone()
    }

    async fn finish(&self, number: &str) {
        self.finish_with(number, "finish").await;
    }

    async fn finish_with(&self, number: &str, transition: &str) {
        let revision = self.revisions(number).await["revision"].as_i64().unwrap();
        self.ok(
            "mutation($n: String!, $r: Int!, $t: String!) { performTransition(input: { number: $n, revision: $r, transition: $t }) { number } }",
            json!({ "n": number, "r": revision, "t": transition }),
        )
        .await;
    }
}

fn numbers(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|a| a["number"].as_str().unwrap().to_owned())
        .collect()
}

fn code(answer: &Value) -> &str {
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
}

// r[verify container.many]
// r[verify container.add]
// r[verify container.contains]
#[tokio::test(flavor = "multi_thread")]
async fn an_akt_can_be_in_many_containers() {
    let world = World::new().await;
    let backlog = world.create("backlog", &[]).await;
    let epic = world.create("epic", &[]).await;
    let item = world.create("item", &[]).await;
    assert!(world.add(&backlog, &item).await.get("errors").is_none());
    assert!(world.add(&epic, &item).await.get("errors").is_none());
    assert!(world.add(&backlog, &epic).await.get("errors").is_none());
    assert_eq!(
        world.containers(&item).await,
        [backlog.clone(), epic.clone()]
    );
    assert_eq!(world.members(&backlog).await, [epic.clone(), item.clone()]);

    assert_eq!(code(&world.add(&backlog, &item).await), "ALREADY_MEMBER");
    assert_eq!(code(&world.add(&item, &epic).await), "NOT_CONTAINABLE");
    assert_eq!(code(&world.add("BACKLOG-9", &item).await), "NOT_FOUND");
}

// r[verify container.remove]
#[tokio::test(flavor = "multi_thread")]
async fn removing_ends_one_membership() {
    let world = World::new().await;
    let backlog = world.create("backlog", &[]).await;
    let epic = world.create("epic", &[]).await;
    let item = world.create("item", &[]).await;
    world.add(&backlog, &item).await;
    world.add(&epic, &item).await;
    assert!(world.remove(&backlog, &item).await.get("errors").is_none());
    assert_eq!(world.containers(&item).await, [epic]);
    assert_eq!(code(&world.remove(&backlog, &item).await), "NOT_MEMBER");
}

// r[verify container.exclusive]
#[tokio::test(flavor = "multi_thread")]
async fn an_exclusive_type_holds_an_akt_once() {
    let world = World::new().await;
    let first = world.create("backlog", &[]).await;
    let second = world.create("backlog", &[]).await;
    let item = world.create("item", &[]).await;
    let epic = world.create("epic", &[]).await;
    world.add(&first, &item).await;
    assert_eq!(code(&world.add(&second, &item).await), "EXCLUSIVE");
    // Epics are not exclusive in backlogs.
    world.add(&first, &epic).await;
    assert!(world.add(&second, &epic).await.get("errors").is_none());
    // Moving between backlogs is a remove and an add.
    world.remove(&first, &item).await;
    assert!(world.add(&second, &item).await.get("errors").is_none());
}

// r[verify container.acyclic]
#[tokio::test(flavor = "multi_thread")]
async fn containment_never_closes_a_cycle() {
    let world = World::new().await;
    let [a, b, c] = [
        world.create("group", &[]).await,
        world.create("group", &[]).await,
        world.create("group", &[]).await,
    ];
    world.add(&a, &b).await;
    world.add(&b, &c).await;
    assert_eq!(code(&world.add(&c, &a).await), "CYCLE");
    assert_eq!(code(&world.add(&b, &a).await), "CYCLE");
    assert_eq!(code(&world.add(&a, &a).await), "CYCLE");
    assert!(
        world.add(&a, &c).await.get("errors").is_none(),
        "a diamond is no cycle"
    );
}

// r[verify container.frozen]
#[tokio::test(flavor = "multi_thread")]
async fn a_final_container_is_frozen_but_final_members_move_freely() {
    let world = World::new().await;
    let release = world.create("release", &[]).await;
    let backlog = world.create("backlog", &[]).await;
    world.add(&release, &backlog).await;
    world.finish(&release).await;
    let late = world.create("backlog", &[]).await;
    assert_eq!(code(&world.add(&release, &late).await), "FROZEN");
    assert_eq!(code(&world.remove(&release, &backlog).await), "FROZEN");
    assert_eq!(
        code(&world.try_create("hotfix", &[&release]).await),
        "FROZEN"
    );

    let epic = world.create("epic", &[]).await;
    let item = world.create("item", &[]).await;
    world.finish(&item).await;
    assert!(world.add(&epic, &item).await.get("errors").is_none());
}

// r[verify container.revisions]
// r[verify container.unversioned]
#[tokio::test(flavor = "multi_thread")]
async fn both_akter_record_each_membership() {
    let world = World::new().await;
    let epic = world.create("epic", &[]).await;
    let item = world.create("item", &[]).await;
    // Someone edits the item meanwhile; adding it needs no revision.
    world
        .ok(
            "mutation($n: String!) { editAkt(input: { number: $n, revision: 1, fields: { title: \"U\" } }) { revision } }",
            json!({ "n": item }),
        )
        .await;
    world.add(&epic, &item).await;
    world.remove(&epic, &item).await;

    let item_history = world.revisions(&item).await;
    assert_eq!(item_history["revision"], 4);
    let revisions = item_history["revisions"].as_array().unwrap();
    assert_eq!(revisions[2]["reason"], "Planned");
    assert_eq!(revisions[2]["membership"]["added"][0]["number"], epic);
    assert_eq!(revisions[3]["membership"]["removed"][0]["number"], epic);
    // State and fields didn't change, so neither did the hash.
    assert_eq!(revisions[2]["hash"], revisions[1]["hash"]);
    assert_eq!(revisions[3]["hash"], revisions[1]["hash"]);

    let epic_history = world.revisions(&epic).await;
    let revisions = epic_history["revisions"].as_array().unwrap();
    assert_eq!(
        revisions[1]["membership"]["added_member"][0]["number"],
        item
    );
    assert_eq!(
        revisions[2]["membership"]["removed_member"][0]["number"],
        item
    );
}

// r[verify container.created-in]
// r[verify container.create-inside]
// r[verify akt.number.sequence]
#[tokio::test(flavor = "multi_thread")]
async fn release_candidates_and_hotfixes_number_inside_their_release() {
    let world = World::new().await;
    let year = Utc::now().year();
    let first = world.create("release", &[]).await;
    let second = world.create("release", &[]).await;
    assert_eq!(first, format!("RELEASE-{year}-0001"));
    let rc1 = world.create("release_candidate", &[&first]).await;
    let rc2 = world.create("release_candidate", &[&first]).await;
    let other = world.create("release_candidate", &[&second]).await;
    let hotfix = world.create("hotfix", &[&first]).await;
    assert_eq!(rc1, format!("RELEASE-{year}-0001-RC-0001"));
    assert_eq!(rc2, format!("RELEASE-{year}-0001-RC-0002"));
    assert_eq!(other, format!("RELEASE-{year}-0002-RC-0001"));
    assert_eq!(hotfix, format!("RELEASE-{year}-0001-HOTFIX-0001"));
    assert_eq!(world.members(&first).await, [rc1.clone(), rc2, hotfix]);

    // Its first revision records where it was created; the release records
    // the new member.
    let history = world.revisions(&rc1).await;
    assert_eq!(
        history["revisions"][0]["membership"]["added"][0]["number"],
        first
    );
    let release = world.revisions(&first).await;
    assert_eq!(
        release["revisions"][1]["membership"]["added_member"][0]["number"],
        rc1
    );

    assert_eq!(
        code(&world.try_create("release_candidate", &[]).await),
        "CREATED_IN"
    );
    let backlog = world.create("backlog", &[]).await;
    assert_eq!(
        code(&world.try_create("release_candidate", &[&backlog]).await),
        "CREATED_IN"
    );
    // An item can be created inside a release candidate, which contains items.
    let item = world.create("item", &[&rc1]).await;
    assert_eq!(world.containers(&item).await, [rc1]);
}

// r[verify container.create-inside]
#[tokio::test(flavor = "multi_thread")]
async fn creating_inside_checks_every_container() {
    let world = World::new().await;
    let first = world.create("backlog", &[]).await;
    let second = world.create("backlog", &[]).await;
    let epic = world.create("epic", &[]).await;
    assert_eq!(
        code(&world.try_create("item", &[&first, &second]).await),
        "EXCLUSIVE"
    );
    assert_eq!(
        code(&world.try_create("epic", &[&epic]).await),
        "NOT_CONTAINABLE"
    );
    assert_eq!(
        code(&world.try_create("item", &[&first, &first]).await),
        "ALREADY_MEMBER"
    );
    let item = world.create("item", &[&first, &epic]).await;
    assert_eq!(world.containers(&item).await, [first, epic]);
}

// r[verify order.move]
// r[verify order.everywhere]
#[tokio::test(flavor = "multi_thread")]
async fn a_move_in_the_backlog_shows_in_the_epic() {
    let world = World::new().await;
    let backlog = world.create("backlog", &[]).await;
    let epic = world.create("epic", &[]).await;
    let [a, b, c, d] = [
        world.create("item", &[&backlog, &epic]).await,
        world.create("item", &[&backlog]).await,
        world.create("item", &[&backlog, &epic]).await,
        world.create("item", &[&backlog]).await,
    ];
    assert_eq!(world.members(&epic).await, [a.clone(), c.clone()]);

    // In the backlog, move C to the top.
    world.moved(&c, "before", &a).await;
    assert_eq!(
        world.members(&backlog).await,
        [c.clone(), a.clone(), b.clone(), d.clone()]
    );
    assert_eq!(world.members(&epic).await, [c.clone(), a.clone()]);

    // In the epic, move C back after A: in the backlog it lands right after
    // A, ahead of B, disturbing nothing else.
    world.moved(&c, "after", &a).await;
    assert_eq!(world.members(&epic).await, [a.clone(), c.clone()]);
    assert_eq!(
        world.members(&backlog).await,
        [a.clone(), c.clone(), b.clone(), d.clone()]
    );

    // Moving to where it already is changes nothing.
    world.moved(&c, "before", &b).await;
    assert_eq!(world.members(&backlog).await, [a.clone(), c.clone(), b, d]);
    assert_eq!(code(&world.moved(&c, "before", &c).await), "BAD_REQUEST");
    assert_eq!(
        code(&world.moved(&c, "before", "ITEM-99").await),
        "NOT_FOUND"
    );
}

// r[verify order.not-recorded]
#[tokio::test(flavor = "multi_thread")]
async fn a_move_is_a_change_event_not_a_revision() {
    let world = World::new().await;
    let a = world.create("item", &[]).await;
    let b = world.create("item", &[]).await;
    world.moved(&b, "before", &a).await;
    assert_eq!(world.revisions(&b).await["revision"], 1);
    let mut conn = world.db.pool().get().await.unwrap();
    let kinds: Vec<(String, Option<i32>)> = change_event::table
        .order(change_event::seq)
        .select((change_event::kind, change_event::revision))
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(kinds.last().unwrap(), &("rank".to_owned(), None));
}

// r[verify order.concurrent]
// r[verify order.rank]
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_moves_keep_ranks_unique() {
    let world = World::new().await;
    let backlog = world.create("backlog", &[]).await;
    let mut items = Vec::new();
    for _ in 0..8 {
        items.push(world.create("item", &[&backlog]).await);
    }
    // Everyone moves their item to the top at once.
    let mut tasks = Vec::new();
    for item in items.iter().skip(1) {
        let router = world.router.clone();
        let (item, top) = (item.clone(), items[0].clone());
        tasks.push(tokio::spawn(async move {
            send(
                &router,
                graphql_with(
                    "tenant",
                    "mutation($n: String!, $o: String!) { moveAkt(input: { number: $n, before: $o }) { rank } }",
                    &json!({ "n": item, "o": top }),
                ),
            )
            .await
            .json()
        }));
    }
    for task in tasks {
        let answer = task.await.unwrap();
        assert!(answer.get("errors").is_none(), "{answer}");
    }
    let order = world.members(&backlog).await;
    assert_eq!(order.len(), 8);
    assert_eq!(
        order.last().unwrap(),
        &items[0],
        "the first item is now last"
    );
}

const ADD: &str = "mutation($c: String!, $m: String!) { addToContainer(input: { container: $c, member: $m }) { number } }";
const MOVE: &str =
    "mutation($n: String!, $o: String!) { moveAkt(input: { number: $n, before: $o }) { number } }";
const CREATE_IN: &str =
    "mutation($in: [String!]!) { createAkt(input: { type: \"item\", in: $in }) { number } }";

// r[verify authz.membership]
#[tokio::test(flavor = "multi_thread")]
async fn arranging_needs_edit_at_the_container_and_read_at_the_member() {
    use kuria_server::authorize::{EDIT, READ};
    let world = World::new().await;
    let epic = world.create("epic", &[]).await;
    let item = world.create("item", &[]).await;
    let pair = json!({ "c": epic, "m": item });

    world.decider.grant_at(ALICE, "epic", &[READ, EDIT]);
    assert_eq!(
        code(&world.run_as(ALICE, ADD, pair.clone()).await),
        "NOT_FOUND",
        "can't see the item"
    );
    world.decider.grant_at(ALICE, "item", &[READ]);
    assert!(
        world
            .run_as(ALICE, ADD, pair.clone())
            .await
            .get("errors")
            .is_none()
    );

    let other = world.create("epic", &[]).await;
    world.decider.grant_at("tenant-bob", "item", &[READ, EDIT]);
    world.decider.grant_at("tenant-bob", "epic", &[READ]);
    let answer = world
        .run_as("tenant-bob", ADD, json!({ "c": other, "m": item }))
        .await;
    assert_eq!(
        code(&answer),
        "FORBIDDEN",
        "may read the epic but not edit it"
    );
}

// r[verify authz.move]
#[tokio::test(flavor = "multi_thread")]
async fn moving_needs_edit_at_the_moved_akt() {
    use kuria_server::authorize::{EDIT, READ};
    let world = World::new().await;
    let a = world.create("item", &[]).await;
    let b = world.create("item", &[]).await;
    let epic = world.create("epic", &[]).await;
    let pair = json!({ "n": b, "o": a });
    assert_eq!(
        code(&world.run_as(ALICE, MOVE, pair.clone()).await),
        "NOT_FOUND"
    );
    world.decider.grant_at(ALICE, "item", &[READ]);
    assert_eq!(
        code(&world.run_as(ALICE, MOVE, pair.clone()).await),
        "FORBIDDEN"
    );
    world.decider.grant_at(ALICE, "item", &[EDIT]);
    assert!(
        world
            .run_as(ALICE, MOVE, pair)
            .await
            .get("errors")
            .is_none()
    );
    // Next to an akt they can't see is as if it didn't exist.
    let answer = world
        .run_as(ALICE, MOVE, json!({ "n": b, "o": epic }))
        .await;
    assert_eq!(code(&answer), "NOT_FOUND");
}

// r[verify authz.create-inside]
#[tokio::test(flavor = "multi_thread")]
async fn creating_inside_needs_edit_at_each_container() {
    use kuria_server::authorize::{CREATE, EDIT, READ};
    let world = World::new().await;
    let epic = world.create("epic", &[]).await;
    world.decider.grant_at(ALICE, "item", &[READ, CREATE]);
    let inside = json!({ "in": [epic] });
    assert_eq!(
        code(&world.run_as(ALICE, CREATE_IN, inside.clone()).await),
        "NOT_FOUND"
    );
    world.decider.grant_at(ALICE, "epic", &[READ]);
    assert_eq!(
        code(&world.run_as(ALICE, CREATE_IN, inside.clone()).await),
        "FORBIDDEN"
    );
    world.decider.grant_at(ALICE, "epic", &[EDIT]);
    assert!(
        world
            .run_as(ALICE, CREATE_IN, inside)
            .await
            .get("errors")
            .is_none()
    );
}

// r[verify authz.members]
#[tokio::test(flavor = "multi_thread")]
async fn lists_leave_out_what_the_subject_cant_read() {
    use kuria_server::authorize::READ;
    let world = World::new().await;
    let backlog = world.create("backlog", &[]).await;
    let epic = world.create("epic", &[&backlog]).await;
    let item = world.create("item", &[&backlog, &epic]).await;
    world.decider.grant_at(ALICE, "backlog", &[READ]);
    world.decider.grant_at(ALICE, "item", &[READ]);
    let data = world
        .run_as(
            ALICE,
            "query($b: String!, $i: String!) { b: akt(number: $b) { members { number } } i: akt(number: $i) { containers { number } } }",
            json!({ "b": backlog, "i": item }),
        )
        .await;
    assert_eq!(data["data"]["b"]["members"], json!([{ "number": item }]));
    assert_eq!(
        data["data"]["i"]["containers"],
        json!([{ "number": backlog }])
    );
}

const TRANSFER: &str = "mutation($m: String!, $f: String!, $t: String!) { transferMember(input: { member: $m, from: $f, to: $t, reason: \"Replanned\" }) { number } }";

impl World {
    async fn transfer(&self, token: &str, member: &str, from: &str, to: &str) -> Value {
        self.run_as(token, TRANSFER, json!({ "m": member, "f": from, "t": to }))
            .await
    }
}

// r[verify container.transfer]
#[tokio::test(flavor = "multi_thread")]
async fn transfers_between_exclusive_containers_in_one_step() {
    let world = World::new().await;
    let first = world.create("backlog", &[]).await;
    let second = world.create("backlog", &[]).await;
    let item = world.create("item", &[&first]).await;
    // Adding would break exclusivity; transferring doesn't.
    assert_eq!(code(&world.add(&second, &item).await), "EXCLUSIVE");
    let answer = world.transfer("tenant", &item, &first, &second).await;
    assert!(answer.get("errors").is_none(), "{answer}");
    assert_eq!(world.containers(&item).await, std::slice::from_ref(&second));

    let history = world.revisions(&item).await;
    let last = &history["revisions"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(last["reason"], "Replanned");
    assert_eq!(last["membership"]["removed"][0]["number"], first);
    assert_eq!(last["membership"]["added"][0]["number"], second);
    assert_eq!(
        history["revision"], 2,
        "one revision for the whole transfer"
    );
    let source = world.revisions(&first).await;
    assert_eq!(
        source["revisions"].as_array().unwrap().last().unwrap()["membership"]["removed_member"][0]
            ["number"],
        item
    );
    let target = world.revisions(&second).await;
    assert_eq!(
        target["revisions"].as_array().unwrap().last().unwrap()["membership"]["added_member"][0]["number"],
        item
    );
}

// r[verify container.transfer]
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_transfer_changes_nothing() {
    let world = World::new().await;
    let backlog = world.create("backlog", &[]).await;
    let epic = world.create("epic", &[]).await;
    let group = world.create("group", &[]).await;
    let item = world.create("item", &[&backlog]).await;
    assert_eq!(
        code(&world.transfer("tenant", &item, &epic, &backlog).await),
        "NOT_MEMBER"
    );
    assert_eq!(
        code(&world.transfer("tenant", &item, &backlog, &group).await),
        "NOT_CONTAINABLE"
    );
    assert_eq!(
        code(&world.transfer("tenant", &item, &backlog, &backlog).await),
        "ALREADY_MEMBER"
    );
    assert_eq!(
        world.containers(&item).await,
        std::slice::from_ref(&backlog)
    );
    world.finish(&backlog).await;
    assert_eq!(
        code(&world.transfer("tenant", &item, &backlog, &epic).await),
        "FROZEN"
    );
    assert_eq!(world.revisions(&item).await["revision"], 1);
}

// r[verify container.while]
#[tokio::test(flavor = "multi_thread")]
async fn a_container_accepts_members_only_while_configured() {
    let world = World::new().await;
    let sprint = world.create("sprint", &[]).await;
    let planned = world.create("item", &[&sprint]).await;
    let late = world.create("item", &[]).await;
    world.finish_with(&sprint, "start").await;
    assert_eq!(code(&world.add(&sprint, &late).await), "NOT_ACCEPTING");
    assert_eq!(
        code(&world.try_create("item", &[&sprint]).await),
        "NOT_ACCEPTING"
    );
    let other = world.create("sprint", &[]).await;
    assert_eq!(
        code(&world.transfer("tenant", &planned, &other, &sprint).await),
        "NOT_MEMBER"
    );
    world.add(&other, &late).await;
    assert_eq!(
        code(&world.transfer("tenant", &late, &other, &sprint).await),
        "NOT_ACCEPTING"
    );
    // Members can still leave.
    assert!(
        world
            .remove(&sprint, &planned)
            .await
            .get("errors")
            .is_none()
    );
}

// r[verify authz.transfer]
#[tokio::test(flavor = "multi_thread")]
async fn transferring_needs_edit_at_both_containers() {
    use kuria_server::authorize::{EDIT, READ};
    let world = World::new().await;
    let first = world.create("backlog", &[]).await;
    let second = world.create("backlog", &[]).await;
    let item = world.create("item", &[&first]).await;
    world.decider.grant(ALICE, &[READ]);
    world.decider.grant_at(ALICE, "backlog", &[EDIT]);
    let epic = world.create("epic", &[]).await;
    world.add(&first, &epic).await;
    // Alice may edit backlogs but not epics.
    let answer = world.transfer(ALICE, &epic, &first, &second).await;
    assert!(answer.get("errors").is_none(), "{answer}");
    let other_epic = world.create("epic", &[]).await;
    world.add(&other_epic, &item).await;
    assert_eq!(
        code(&world.transfer(ALICE, &item, &other_epic, &second).await),
        "FORBIDDEN"
    );
    let bob = "tenant-bob";
    world.decider.grant_at(bob, "backlog", &[READ, EDIT]);
    assert_eq!(
        code(&world.transfer(bob, &item, &first, &second).await),
        "NOT_FOUND",
        "can't see the item"
    );
}
