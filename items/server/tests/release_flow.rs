//! A release from planning to closed, with the example release-management
//! configuration in `docs/examples/release-management`, a developer and a
//! release manager, through GraphQL against a fresh database.

mod support;

use axum::Router;
use chrono::{Datelike, Utc};
use kuria_server::authorize::{CREATE, EDIT, READ, TRANSITION};
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

const ADMIN: &str = "tenant";
const DEV: &str = "tenant-dev";
const RM: &str = "tenant-release-manager";

const DOCUMENTS: [&str; 7] = [
    include_str!("../../../docs/examples/release-management/backlog_item.json"),
    include_str!("../../../docs/examples/release-management/epic.json"),
    include_str!("../../../docs/examples/release-management/backlog.json"),
    include_str!("../../../docs/examples/release-management/release.json"),
    include_str!("../../../docs/examples/release-management/release_candidate.json"),
    include_str!("../../../docs/examples/release-management/hotfix.json"),
    include_str!("../../../docs/examples/release-management/release_notes.json"),
];

struct World {
    _db: TestDatabase,
    router: Router,
}

impl World {
    /// The example configuration, and the two roles its README describes.
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let decider = FakeDecider::allowing_tenant();
        roles(&decider);
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let world = Self { _db: db, router };
        for document in DOCUMENTS {
            let document: Value = serde_json::from_str(document).unwrap();
            world
                .ok(
                    ADMIN,
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": document }),
                )
                .await;
        }
        world
    }

    async fn run(&self, token: &str, query: &str, variables: Value) -> Value {
        send(&self.router, graphql_with(token, query, &variables))
            .await
            .json()
    }

    async fn ok(&self, token: &str, query: &str, variables: Value) -> Value {
        let answer = self.run(token, query, variables).await;
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, token: &str, key: &str, inside: &[&str], fields: Value) -> String {
        self.ok(
            token,
            "mutation($t: String!, $in: [String!]!, $f: JSON) { createAkt(input: { type: $t, in: $in, fields: $f }) { number } }",
            json!({ "t": key, "in": inside, "f": fields }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn item(&self, inside: &[&str], title: &str, kind: &str) -> String {
        self.create(
            DEV,
            "backlog_item",
            inside,
            json!({ "title": title, "kind": kind }),
        )
        .await
    }

    async fn revision(&self, number: &str) -> i64 {
        self.ok(
            ADMIN,
            "query($n: String!) { akt(number: $n) { revision } }",
            json!({ "n": number }),
        )
        .await["akt"]["revision"]
            .as_i64()
            .unwrap()
    }

    /// Performs `transition` on `number`, setting `fields` on the way.
    async fn transition(
        &self,
        token: &str,
        number: &str,
        transition: &str,
        fields: Value,
    ) -> Value {
        let revision = self.revision(number).await;
        self.run(
            token,
            "mutation($n: String!, $r: Int!, $t: String!, $f: JSON) { performTransition(input: { number: $n, revision: $r, transition: $t, fields: $f }) { state { key } } }",
            json!({ "n": number, "r": revision, "t": transition, "f": fields }),
        )
        .await
    }

    async fn transitioned(&self, token: &str, number: &str, transition: &str, fields: Value) {
        let answer = self.transition(token, number, transition, fields).await;
        assert!(
            answer.get("errors").is_none(),
            "{number} {transition}: {answer}"
        );
    }

    /// Takes a backlog item through review to done.
    async fn finish(&self, item: &str) {
        self.transitioned(DEV, item, "start", json!({})).await;
        self.transitioned(
            DEV,
            item,
            "review",
            json!({ "change": { "url": format!("https://git.example/pr/{item}") } }),
        )
        .await;
        self.transitioned(DEV, item, "finish", json!({})).await;
    }

    async fn add(&self, token: &str, container: &str, member: &str) -> Value {
        self.run(
            token,
            "mutation($c: String!, $m: String!) { addToContainer(input: { container: $c, member: $m }) { number } }",
            json!({ "c": container, "m": member }),
        )
        .await
    }

    async fn added(&self, token: &str, container: &str, member: &str) {
        let answer = self.add(token, container, member).await;
        assert!(answer.get("errors").is_none(), "{answer}");
    }

    /// The board of `container`: each column's key and its members' numbers.
    async fn board(&self, container: &str) -> Vec<(String, Vec<String>)> {
        let data = self
            .ok(
                DEV,
                "query($n: String!) { akt(number: $n) { board { columns { key akter { number } } } } }",
                json!({ "n": container }),
            )
            .await;
        data["akt"]["board"]["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let numbers = c["akter"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| a["number"].as_str().unwrap().to_owned())
                    .collect();
                (c["key"].as_str().unwrap().to_owned(), numbers)
            })
            .collect()
    }

    async fn column(&self, container: &str, key: &str) -> Vec<String> {
        self.board(container)
            .await
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, numbers)| numbers)
            .unwrap_or_default()
    }
}

/// The developer and release manager roles of the example's README.
fn roles(decider: &FakeDecider) {
    decider.grant(DEV, &[READ]);
    for key in ["backlog", "epic", "backlog_item"] {
        decider.grant_at(DEV, key, &[CREATE, EDIT, TRANSITION]);
    }
    decider.grant(RM, &[READ]);
    for key in ["release", "release_candidate", "hotfix", "release_notes"] {
        decider.grant_at(RM, key, &[CREATE, EDIT, TRANSITION, "release:approve"]);
    }
}

fn code(answer: &Value) -> &str {
    answer["errors"][0]["extensions"]["code"]
        .as_str()
        .unwrap_or_default()
}

#[allow(
    clippy::too_many_lines,
    reason = "one release told from start to finish reads best in one place"
)]
#[tokio::test(flavor = "multi_thread")]
async fn a_release_from_planning_to_closed() {
    let world = World::new().await;
    let year = Utc::now().year();

    // Planning: a release, its backlog, an epic, and items in both.
    let release = world
        .create(RM, "release", &[], json!({ "title": "Spring" }))
        .await;
    assert_eq!(release, format!("RELEASE-{year}-0001"));
    let backlog = world
        .create(DEV, "backlog", &[], json!({ "title": "Spring backlog" }))
        .await;
    world.added(RM, &release, &backlog).await;
    let epic = world
        .create(DEV, "epic", &[&backlog], json!({ "title": "Sign-in" }))
        .await;
    let passkeys = world.item(&[&backlog, &epic], "Passkeys", "feature").await;
    let crash = world.item(&[&backlog], "Crash on save", "fix").await;
    let reset = world
        .item(&[&backlog, &epic], "Password reset", "feature")
        .await;
    let typo = world.item(&[&backlog], "Typo in the menu", "fix").await;

    // Reprioritising the backlog reorders the epic too.
    world
        .ok(
            DEV,
            "mutation($n: String!, $o: String!) { moveAkt(input: { number: $n, before: $o }) { number } }",
            json!({ "n": reset, "o": passkeys }),
        )
        .await;
    assert_eq!(
        world.column(&epic, "todo").await,
        [reset.clone(), passkeys.clone()]
    );
    assert_eq!(
        world.column(&backlog, "todo").await,
        [
            epic.clone(),
            reset.clone(),
            passkeys.clone(),
            crash.clone(),
            typo.clone()
        ]
    );

    // Testing: developers can't cut candidates; the release manager can.
    world.transitioned(RM, &release, "test", json!({})).await;
    let refused = world
        .run(
            DEV,
            "mutation($r: String!) { createAkt(input: { type: \"release_candidate\", in: [$r] }) { number } }",
            json!({ "r": release }),
        )
        .await;
    assert_eq!(code(&refused), "FORBIDDEN");
    let rc1 = world
        .create(RM, "release_candidate", &[&release], json!({}))
        .await;
    assert_eq!(rc1, format!("RELEASE-{year}-0001-RC-0001"));
    world.added(RM, &rc1, &epic).await;
    world.added(RM, &rc1, &crash).await;
    // Tried in the first candidate only, and dropped.
    let experiment = world
        .item(&[&backlog], "Experimental search", "feature")
        .await;
    world.added(RM, &rc1, &experiment).await;

    // Work moves across the candidate's board as it is done.
    world.finish(&crash).await;
    assert_eq!(
        world.column(&rc1, "done").await,
        std::slice::from_ref(&crash)
    );
    assert_eq!(
        world.column(&rc1, "todo").await,
        [epic.clone(), experiment.clone()]
    );

    // The first candidate fails; the second ships everything.
    world.transitioned(RM, &rc1, "reject", json!({})).await;
    let rc2 = world
        .create(RM, "release_candidate", &[&release], json!({}))
        .await;
    assert_eq!(rc2, format!("RELEASE-{year}-0001-RC-0002"));
    for member in [&epic, &crash, &typo] {
        world.added(RM, &rc2, member).await;
    }
    for item in [&passkeys, &reset, &typo] {
        world.finish(item).await;
    }
    assert_eq!(
        code(&world.transition(RM, &rc2, "accept", json!({})).await),
        "REFUSED",
        "acceptance needs its evidence"
    );
    world
        .transitioned(
            RM,
            &rc2,
            "accept",
            json!({
                "build": { "url": "https://ci.example/builds/812", "label": "Build 812" },
                "test_record": { "url": "https://kuria.example/akt/TR-17" }
            }),
        )
        .await;

    // A second release numbers its candidates from 1 again.
    let next = world
        .create(RM, "release", &[], json!({ "title": "Summer" }))
        .await;
    let next_rc = world
        .create(RM, "release_candidate", &[&next], json!({}))
        .await;
    assert_eq!(next_rc, format!("RELEASE-{year}-0002-RC-0001"));

    // Releasing needs the version and date, and release:approve.
    let released_fields = json!({ "version": "4.2.0", "released_on": "2026-04-01" });
    assert_eq!(
        code(
            &world
                .transition(DEV, &release, "release", released_fields.clone())
                .await
        ),
        "FORBIDDEN"
    );
    world
        .transitioned(RM, &release, "release", released_fields)
        .await;

    // Once released, it takes hotfixes but no more candidates.
    let late_rc = world
        .run(
            RM,
            "mutation($r: String!) { createAkt(input: { type: \"release_candidate\", in: [$r] }) { number } }",
            json!({ "r": release }),
        )
        .await;
    assert_eq!(code(&late_rc), "NOT_ACCEPTING");

    // The dropped experiment is replanned into the next release's backlog,
    // in one step.
    let next_backlog = world
        .create(DEV, "backlog", &[], json!({ "title": "Summer backlog" }))
        .await;
    let transferred = world
        .run(
            DEV,
            "mutation($m: String!, $f: String!, $t: String!) { transferMember(input: { member: $m, from: $f, to: $t, reason: \"Next release\" }) { number } }",
            json!({ "m": experiment, "f": backlog, "t": next_backlog }),
        )
        .await;
    assert!(transferred.get("errors").is_none(), "{transferred}");

    // A released release still takes hotfixes.
    let hotfix = world
        .create(
            RM,
            "hotfix",
            &[&release],
            json!({ "reason": "Sessions are lost on restart" }),
        )
        .await;
    assert_eq!(hotfix, format!("RELEASE-{year}-0001-HOTFIX-0001"));
    let sessions = world.item(&[&backlog], "Lost sessions", "fix").await;
    world.added(RM, &hotfix, &sessions).await;
    world.finish(&sessions).await;
    world
        .transitioned(
            RM,
            &hotfix,
            "deploy",
            json!({ "build": { "url": "https://ci.example/builds/815" } }),
        )
        .await;

    // Release notes, drafted from the candidates and the hotfix, then approved.
    let notes = world
        .create(RM, "release_notes", &[&release], json!({}))
        .await;
    let drafted = world
        .ok(
            RM,
            "mutation($n: String!) { draftField(input: { number: $n, revision: 1, field: \"text\" }) { fields { ... on TextValue { text } } } }",
            json!({ "n": notes }),
        )
        .await;
    assert_eq!(
        drafted["draftField"]["fields"][0]["text"],
        [
            "## Epic".to_owned(),
            format!("- {epic} Sign-in ()"),
            String::new(),
            "## Backlog item".to_owned(),
            format!("- {reset} Password reset (Feature)"),
            format!("- {passkeys} Passkeys (Feature)"),
            format!("- {crash} Crash on save (Fix)"),
            format!("- {typo} Typo in the menu (Fix)"),
            format!("- {sessions} Lost sessions (Fix)"),
        ]
        .join("\n")
    );
    world.transitioned(RM, &notes, "approve", json!({})).await;

    // The release board shows its candidates and hotfix by outcome.
    assert_eq!(
        world.column(&release, "accepted").await,
        [rc2.clone(), hotfix.clone()]
    );
    assert_eq!(
        world.column(&release, "rejected").await,
        std::slice::from_ref(&rc1)
    );

    // Closing the release freezes it.
    world.transitioned(RM, &release, "close", json!({})).await;
    let late = world
        .run(
            RM,
            "mutation($r: String!) { createAkt(input: { type: \"hotfix\", in: [$r], fields: { reason: \"Too late\" } }) { number } }",
            json!({ "r": release }),
        )
        .await;
    assert_eq!(code(&late), "FROZEN");

    // The crash fix's history shows each place it was put.
    let history = world
        .ok(
            ADMIN,
            "query($n: String!) { akt(number: $n) { revisions { membership } } }",
            json!({ "n": crash }),
        )
        .await;
    let placed: Vec<String> = history["akt"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| {
            r["membership"]["added"][0]["number"]
                .as_str()
                .map(str::to_owned)
        })
        .collect();
    assert_eq!(placed, [backlog, rc1, rc2]);
}
