//! The examples' bundles (`bundle.examples`): each example listed in
//! `EXAMPLES` holds `bundle/`, its akt types with akter made here
//! through GraphQL, as the people of the end-to-end seed would make them in
//! the app.
//!
//! - `every_example_s_bundle_imports_into_a_fresh_database` checks the
//!   bundles as they are checked in.
//! - `write_example_bundles`, ignored, writes them again:
//!   `mise run examples:bundles`.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use kuria_server::bundles::{Bootstrap, bootstrap};
use kuria_store::people::{self, Person};
use serde_json::{Value, json};

use support::TENANT;
use support::decider::FakeDecider;
use support::world::{World, write_bundle};

/// Each example, and its akt types in the order they are defined.
const EXAMPLES: [(&str, &[&str]); 3] = [
    (
        "release-management",
        &[
            "artifact",
            "backlog_item",
            "epic",
            "backlog",
            "release",
            "release_candidate",
            "hotfix",
            "release_notes",
            "test_case",
            "test_protocol",
            "problem_report",
            "capa",
            "capa_action",
        ],
    ),
    (
        "documents",
        &["image", "document", "external_document", "document_index"],
    ),
    ("planning", &["plan", "task", "gap", "decision"]),
];

/// The examples' people, with the emails of the end-to-end seed
/// (`e2e/scripts/setup.ts`), and the build pipeline. The example stack
/// (`example/scripts/setup.ts`) seeds Konklav with these very subjects.
const DEV: &str = "01900000-0000-7000-8000-00000000d001";
const RM: &str = "01900000-0000-7000-8000-00000000d002";
const HEDDA: &str = "01900000-0000-7000-8000-00000000d003";
const CI: &str = "01900000-0000-7000-8000-00000000d004";

const PEOPLE: [(&str, &str, Option<&str>); 4] = [
    (DEV, "Devi Larsen", Some("dev@shop.example")),
    (RM, "Rami Moberg", Some("rm@shop.example")),
    (HEDDA, "Hedda Lund", Some("hedda@shop.example")),
    (CI, "Build pipeline", None),
];

/// The token of `subject` (`as-<subject>`, which the fake host accepts).
fn t(subject: &str) -> String {
    format!("as-{subject}")
}

fn example_dir(example: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples")
        .join(example)
}

fn document(example: &str, key: &str) -> Value {
    let path = example_dir(example).join(format!("{key}.json"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// Everyone may do everything: the examples show data, not permissions.
fn decider() -> Arc<FakeDecider> {
    let decider = FakeDecider::allowing_tenant();
    for (subject, _, _) in PEOPLE {
        decider.allow_everything(&t(subject));
    }
    decider
}

/// A fresh Kuria with `example`'s akt types, defined by the release
/// manager, and its people's names recorded as their sign-ins would.
async fn example_world(example: &str, types: &[&str]) -> World {
    let world = World::new(decider()).await;
    let mut conn = world.db.pool().get().await.unwrap();
    for (subject, name, email) in PEOPLE {
        people::record(
            &mut conn,
            &Person {
                subject_id: subject.parse().unwrap(),
                name: Some(name.to_owned()),
                email: email.map(str::to_owned),
                updated_at: Utc::now(),
            },
        )
        .await
        .unwrap();
    }
    drop(conn);
    for key in types {
        world.define(&t(RM), &document(example, key)).await;
    }
    world
}

// r[verify bundle.examples]
#[tokio::test(flavor = "multi_thread")]
async fn every_example_s_bundle_imports_into_a_fresh_database() {
    for (example, types) in EXAMPLES {
        let dir = example_dir(example);
        let world = World::new(decider()).await;
        let imported = bootstrap(
            world.db.pool(),
            &world.storage,
            &Bootstrap {
                bundle: dir.join("bundle"),
                tenant: TENANT.parse().unwrap(),
                subjects: None,
            },
        )
        .await
        .unwrap_or_else(|e| {
            panic!("{example}'s bundle: {e}; write it again with `mise run examples:bundles`")
        });
        assert_eq!(imported.akt_types, types.len(), "{example}");
        assert!(imported.akter >= 5, "{example}: {imported:?}");

        // Its akt types are the example's documents, every one of them.
        let bundle: Value =
            serde_json::from_slice(&std::fs::read(dir.join("bundle/bundle.json")).unwrap())
                .unwrap();
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let name = e.unwrap().file_name().to_string_lossy().into_owned();
                name.strip_suffix(".json").map(str::to_owned)
            })
            .collect();
        files.sort_unstable();
        let mut keys: Vec<String> = types.iter().map(|k| (*k).to_owned()).collect();
        keys.sort_unstable();
        assert_eq!(files, keys, "{example}'s akt type files");
        for t in bundle["aktTypes"].as_array().unwrap() {
            let key = t["key"].as_str().unwrap();
            let latest = t["versions"]
                .as_array()
                .unwrap()
                .iter()
                .max_by_key(|v| v["version"].as_i64())
                .unwrap();
            let read = |v: &Value| -> kuria_config::AktTypeDocument {
                serde_json::from_value(v.clone()).unwrap()
            };
            assert_eq!(
                read(&latest["definition"]),
                read(&document(example, key)),
                "{example}'s bundle holds another `{key}` than {key}.json: write it again with `mise run examples:bundles`"
            );
        }

        // And everything reads back.
        let read = world
            .ok("tenant", "{ aktTypes { key akterCount } }", json!({}))
            .await;
        let counted: i64 = read["aktTypes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["akterCount"].as_i64().unwrap())
            .sum();
        // Archived akter aren't counted (`archive.read`).
        let archived = bundle["akter"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| !a["archived"].is_null())
            .count();
        assert_eq!(
            usize::try_from(counted).unwrap() + archived,
            imported.akter,
            "{example}"
        );
    }
}

// r[impl bundle.examples]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "writes docs/examples/*/bundle: run by `mise run examples:bundles`"]
async fn write_example_bundles() {
    for (example, types) in EXAMPLES {
        let world = example_world(example, types).await;
        match example {
            "release-management" => release_management(&world).await,
            "documents" => documents(&world).await,
            "planning" => planning(&world).await,
            other => panic!("no data for {other}"),
        }
        let archive = world
            .export(&t(RM), &format!("types={}&akter=true", types.join(",")))
            .await;
        write_bundle(&archive, &example_dir(example).join("bundle"));
    }
}

/// A pull request's reference.
fn pr(repository: &str, n: u32) -> Value {
    json!({ "url": format!("https://git.example/shop/{repository}/pulls/{n}"), "label": format!("{repository}#{n}") })
}

fn digest(digit: char) -> String {
    format!("sha256:{}", String::from(digit).repeat(64))
}

/// Two releases of a web shop: spring's, released with a rejected and an
/// accepted candidate, each with its run of the acceptance test, approved
/// notes and a hotfix, and closed; summer's, being planned, with a test
/// protocol in draft.
#[expect(clippy::too_many_lines, reason = "one release told from start to end")]
async fn release_management(w: &World) {
    let (dev, rm, ci) = (t(DEV), t(RM), t(CI));

    // Spring: its backlog, epics and items.
    let spring = w
        .create(
            &rm,
            "release",
            &[],
            json!({ "title": "Spring", "release_manager": RM }),
            "Plan the spring release",
        )
        .await;
    let backlog = w
        .create(
            &dev,
            "backlog",
            &[],
            json!({ "title": "Spring backlog" }),
            "What spring is to hold",
        )
        .await;
    w.add_to(&rm, &spring, &backlog, "Spring ships this backlog")
        .await;
    let checkout = w
        .create(
            &dev,
            "epic",
            &[&backlog],
            json!({ "title": "One-page checkout", "goal": "Customers pay on one page, without making an account." }),
            "From the checkout research",
        )
        .await;
    let sign_in = w
        .create(
            &dev,
            "epic",
            &[&backlog],
            json!({ "title": "Sign-in", "goal": "Returning customers sign in with a passkey or a reset password." }),
            "Support's top request",
        )
        .await;
    let item = async |inside: &[&str], title: &str, kind: &str, description: &str| {
        w.create(
            &dev,
            "backlog_item",
            inside,
            json!({ "title": title, "kind": kind, "description": description, "assignee": DEV }),
            "Added in backlog grooming",
        )
        .await
    };
    let address = item(
        &[&backlog, &checkout],
        "Address form remembers the last address",
        "feature",
        "Prefill the shipping address from the customer's last order.",
    )
    .await;
    let guest = item(
        &[&backlog, &checkout],
        "Pay as a guest",
        "feature",
        "Checkout without an account; offer one after payment.",
    )
    .await;
    let passkeys = item(
        &[&backlog, &sign_in],
        "Passkeys",
        "feature",
        "Sign in with a passkey, falling back to a password.",
    )
    .await;
    let reset = item(
        &[&backlog, &sign_in],
        "Password reset by email",
        "feature",
        "A link valid for 30 minutes, once.",
    )
    .await;
    let crash = item(
        &[&backlog],
        "Crash when saving an empty basket",
        "fix",
        "Saving a basket with no lines throws in the order service.",
    )
    .await;
    let deps = item(
        &[&backlog],
        "Update the TLS library",
        "chore",
        "The old version is out of support in May.",
    )
    .await;
    let experiment = item(
        &[&backlog],
        "Experimental search ranking",
        "feature",
        "Rank by what sold, behind a flag.",
    )
    .await;
    w.transition(&dev, &checkout, "start", None, "Work started")
        .await;
    w.transition(&dev, &sign_in, "start", None, "Work started")
        .await;
    let finish = async |number: &str, repository: &str, n: u32| {
        w.transition(&dev, number, "start", None, "Picked up").await;
        w.transition(
            &dev,
            number,
            "review",
            Some(json!({ "change": pr(repository, n) })),
            "Ready for review",
        )
        .await;
        w.transition(&dev, number, "finish", None, "Reviewed and merged")
            .await;
    };

    // The acceptance test every candidate runs, kept as a template.
    let acceptance = acceptance_template(w).await;

    // The first candidate: the pipeline's builds, a fix, and the experiment.
    w.transition(&rm, &spring, "test", None, "Feature freeze")
        .await;
    let rc1 = w
        .create(
            &rm,
            "release_candidate",
            &[&spring],
            json!({ "label": "4.2.0-rc.1" }),
            "First candidate",
        )
        .await;
    w.link(
        &rm,
        &rc1,
        "parent_board",
        &backlog,
        "Planned from the spring backlog",
    )
    .await;
    for member in [&checkout, &crash, &experiment] {
        w.add_to(&rm, &rc1, member, "In the first candidate").await;
    }
    let sbom = json!({
        "bomFormat": "CycloneDX", "specVersion": "1.6",
        "metadata": { "component": { "name": "backend", "version": "4.2.0-rc.1" } },
        "components": [
            { "type": "library", "name": "serde", "version": "1.0.228" },
            { "type": "library", "name": "tokio", "version": "1.48.0" },
            { "type": "library", "name": "axum", "version": "0.8.6" }
        ]
    });
    let sbom = w
        .upload(
            &ci,
            "backend.cdx.json",
            "application/vnd.cyclonedx+json",
            serde_json::to_vec_pretty(&sbom).unwrap(),
        )
        .await;
    let built = async |component: &str, version: &str, d: char, sbom: Option<&str>| {
        let mut fields = json!({
            "component": component,
            "change": "built",
            "version": version,
            "source": { "repository": format!("https://git.example/shop/{component}.git"), "tag": format!("v{version}") },
            "image": { "repository": format!("registry.example/shop/{component}"), "digest": digest(d), "tag": version },
        });
        if let Some(sbom) = sbom {
            fields["sbom"] = json!(sbom);
        }
        w.create(&ci, "artifact", &[], fields, "Built by the pipeline")
            .await
    };
    let backend_rc1 = built("backend", "4.2.0-rc.1", 'a', Some(&sbom)).await;
    let frontend = built("frontend", "4.2.0-rc.1", 'b', None).await;
    let search = built("search", "0.1.0", 'c', None).await;
    for artifact in [&backend_rc1, &frontend, &search] {
        w.add_to(&rm, &rc1, artifact, "Built for the first candidate")
            .await;
    }
    finish(&crash, "backend", 311).await;
    acceptance_run(
        w,
        &acceptance,
        &rc1,
        "4.2.0-rc.1",
        |title| match title {
            "Browse and search" => (
                ("fail", Some("Search took 5 s with the new ranking.")),
                ("fail", Some("The product list took several seconds.")),
            ),
            _ => (("pass", None), ("pass", None)),
        },
        "Failed: search is too slow; the candidate is rejected.",
    )
    .await;
    w.transition(
        &rm,
        &rc1,
        "reject",
        None,
        "Search ranking slows the product list down fivefold",
    )
    .await;

    // The second: everything else, the backend rebuilt, search dropped.
    let rc2 = w
        .create(
            &rm,
            "release_candidate",
            &[&spring],
            json!({ "label": "4.2.0-rc.2" }),
            "Second candidate, without search",
        )
        .await;
    w.link(
        &rm,
        &rc2,
        "parent_board",
        &backlog,
        "Planned from the spring backlog",
    )
    .await;
    for member in [&checkout, &sign_in, &crash, &deps] {
        w.add_to(&rm, &rc2, member, "In the second candidate").await;
    }
    let backend_rc2 = built("backend", "4.2.0-rc.2", 'd', None).await;
    let no_search = w
        .create(
            &ci,
            "artifact",
            &[],
            json!({ "component": "search", "change": "removed" }),
            "Removed from the deployment",
        )
        .await;
    for artifact in [&backend_rc2, &frontend, &no_search] {
        w.add_to(&rm, &rc2, artifact, "Built for the second candidate")
            .await;
    }
    for (number, repository, n) in [
        (&address, "frontend", 207),
        (&guest, "frontend", 212),
        (&passkeys, "backend", 318),
        (&reset, "backend", 320),
        (&deps, "backend", 322),
    ] {
        finish(number, repository, n).await;
    }
    w.transition(&dev, &checkout, "finish", None, "All its items are done")
        .await;
    w.transition(&dev, &sign_in, "finish", None, "All its items are done")
        .await;
    // The candidate's run passes: what lets it be accepted.
    acceptance_run(
        w,
        &acceptance,
        &rc2,
        "4.2.0-rc.2",
        |title| match title {
            "Sign in with a passkey" => (
                ("pass", Some("On a phone and a laptop.")),
                (
                    "skip",
                    Some("No passkey on my test phone; Devi covered it."),
                ),
            ),
            _ => (("pass", None), ("pass", None)),
        },
        "Passed: 4.2.0-rc.2 is fit to release.",
    )
    .await;
    w.transition(
        &rm,
        &rc2,
        "accept",
        Some(json!({
            "build": { "url": "https://ci.example/shop/builds/812", "label": "Build 812" }
        })),
        "Passed acceptance",
    )
    .await;

    // Notes, approved; the go, from the head of product; released.
    let notes = w
        .create(
            &rm,
            "release_notes",
            &[&spring],
            json!({}),
            "Notes for 4.2.0",
        )
        .await;
    w.edit(
        &rm,
        &notes,
        json!({
            "text": "Shop 4.2.0\n\n- Checkout on one page, as a guest if you like, with your last address filled in.\n- Sign in with a passkey, or reset your password by email.\n- Fixed: saving an empty basket no longer fails.",
            "artifacts": "backend 4.2.0-rc.2, frontend 4.2.0-rc.1; search removed."
        }),
        "Drafted from the candidate",
    )
    .await;
    w.transition(&rm, &notes, "approve", None, "Read and approved")
        .await;
    w.edit(
        &rm,
        &spring,
        json!({ "go": { "for": HEDDA } }),
        "Hedda gave the go in the release meeting",
    )
    .await;
    w.transition(
        &rm,
        &spring,
        "release",
        Some(json!({ "version": "4.2.0", "released_on": "2026-04-01" })),
        "Released to production",
    )
    .await;

    // A hotfix after the release, then closed.
    let lost = w
        .create(
            &dev,
            "backlog_item",
            &[&backlog],
            json!({ "title": "Sessions lost on restart", "kind": "fix", "assignee": DEV,
                    "description": "Signed-in customers are signed out whenever the backend restarts." }),
            "Reported by support after the release",
        )
        .await;
    let hotfix = w
        .create(
            &rm,
            "hotfix",
            &[&spring],
            json!({ "reason": "Customers are signed out on every deploy" }),
            "Hotfix for sessions",
        )
        .await;
    w.add_to(&rm, &hotfix, &lost, "What the hotfix fixes").await;
    finish(&lost, "backend", 331).await;
    let backend_hotfix = built("backend", "4.2.1", 'e', None).await;
    w.add_to(&rm, &hotfix, &backend_hotfix, "The fixed build")
        .await;
    w.transition(
        &rm,
        &hotfix,
        "deploy",
        Some(json!({ "build": { "url": "https://ci.example/shop/builds/815", "label": "Build 815" } })),
        "Deployed 4.2.1",
    )
    .await;
    w.transition(&rm, &spring, "close", None, "Nothing more to ship")
        .await;

    // Summer: being planned, the experiment carried over.
    let summer = w
        .create(
            &rm,
            "release",
            &[],
            json!({ "title": "Summer", "release_manager": RM }),
            "Plan the summer release",
        )
        .await;
    let summer_backlog = w
        .create(
            &dev,
            "backlog",
            &[],
            json!({ "title": "Summer backlog" }),
            "What summer is to hold",
        )
        .await;
    w.add_to(&rm, &summer, &summer_backlog, "Summer ships this backlog")
        .await;
    w.ok(
        &dev,
        "mutation($m: String!, $f: String!, $t: String!) { transferMember(input: { member: $m, from: $f, to: $t, reason: \"Try again in summer\" }) { number } }",
        json!({ "m": experiment, "f": backlog, "t": summer_backlog }),
    )
    .await;
    w.transition(&dev, &backlog, "close", None, "Spring is done")
        .await;
    let wishlist = w
        .create(
            &dev,
            "epic",
            &[&summer_backlog],
            json!({ "title": "Wish lists", "goal": "Customers keep what they want for later, and share it." }),
            "From the customer survey",
        )
        .await;
    let save = item(
        &[&summer_backlog, &wishlist],
        "Save a product for later",
        "feature",
        "A heart on every product page.",
    )
    .await;
    item(
        &[&summer_backlog, &wishlist],
        "Share a wish list by link",
        "feature",
        "Read-only, revocable.",
    )
    .await;
    item(
        &[&summer_backlog],
        "Faster product images",
        "chore",
        "Serve AVIF with a JPEG fallback.",
    )
    .await;
    w.transition(&dev, &wishlist, "start", None, "Work started")
        .await;
    w.transition(&dev, &save, "start", None, "Picked up").await;

    // Summer's own protocol, in draft until it has a candidate to run in.
    let wishes = w
        .create(
            &dev,
            "test_protocol",
            &[],
            json!({ "title": "Wish list test", "owner": DEV, "scope": "The summer release's wish lists." }),
            "Start the wish list protocol",
        )
        .await;
    w.create(
        &dev,
        "test_case",
        &[&wishes],
        json!({ "title": "Save a product for later", "steps": "1. Open a product.\n2. Tap the heart.", "expected": "It is in the wish list.", "testers": [DEV] }),
        "First case",
    )
    .await;
}

/// A quality manual with folders of documents, which refer to each other
/// and to a diagram, released; and an archived handbook.
#[expect(
    clippy::too_many_lines,
    reason = "one manual written from start to end"
)]
async fn documents(w: &World) {
    let (dev, rm, hedda) = (t(DEV), t(RM), t(HEDDA));

    let svg = include_str!("examples/release-process.svg");
    let file = w
        .upload(
            &rm,
            "release-process.svg",
            "image/svg+xml",
            svg.as_bytes().to_vec(),
        )
        .await;
    let diagram = w
        .create(
            &rm,
            "image",
            &[],
            json!({ "title": "Release process", "file": file, "caption": "From planning to a closed release." }),
            "The diagram for the release procedure",
        )
        .await;

    let manual = w
        .create(
            &hedda,
            "document_index",
            &[],
            json!({ "title": "Quality manual", "owner": HEDDA, "about": "How the shop plans, builds, tests and releases its software." }),
            "Start the quality manual",
        )
        .await;
    let doc = async |author: &str, title: &str, body: &str| {
        w.create(
            author,
            "document",
            &[&manual],
            json!({ "title": title, "owner": HEDDA, "body": body }),
            "First draft",
        )
        .await
    };
    let policy = doc(
        &hedda,
        "Quality policy",
        "# Quality policy\n\nWe ship small changes often, each one reviewed, tested and traceable from request to release.\n\n## Principles {#principles}\n\n- Every change starts as a backlog item.\n- Nothing ships without a passed acceptance test.\n- Releases are approved by the head of product.\n",
    )
    .await;
    w.release(&hedda, &policy, Some("Approved")).await;
    let control = doc(
        &rm,
        "Document control",
        &format!(
            "# Document control\n\nDocuments in this manual are drafted together and released as versions. A released version is never changed; a new one replaces it.\n\nThey follow the [principles](kuria:{policy}#principles) of the quality policy.\n\n## Review {{#review}}\n\nThe owner reviews each document once a year, and after every change to the process it describes.\n"
        ),
    )
    .await;
    w.release(&rm, &control, Some("First issue")).await;
    let procedure = doc(
        &rm,
        "Release procedure",
        &format!(
            "# Release procedure\n\n![The release process](kuria:{diagram})\n\n1. Plan the release and its backlog.\n2. Freeze features and cut a release candidate.\n3. Run the acceptance test protocol against the candidate.\n4. Write the release notes and have them approved.\n5. Get the go from the head of product, and release.\n\nDocuments are kept as [document control](kuria:{control}#review) says.\n"
        ),
    )
    .await;
    w.release(&rm, &procedure, Some("First issue")).await;
    w.edit(
        &rm,
        &procedure,
        json!({ "body": format!(
            "# Release procedure\n\n![The release process](kuria:{diagram})\n\n1. Plan the release and its backlog.\n2. Freeze features and cut a release candidate.\n3. Run the acceptance test protocol against the candidate; a failed test rejects it.\n4. Write the release notes and have them approved.\n5. Get the go from the head of product, and release.\n6. Close the release once its hotfixes are deployed.\n\nDocuments are kept as [document control](kuria:{control}#review) says.\n"
        ) }),
        "Rejection and closing, after the spring release",
    )
    .await;
    w.release(&rm, &procedure, Some("Second issue")).await;
    let onboarding = doc(
        &dev,
        "Developer onboarding",
        "# Developer onboarding\n\nYour first week: set up the development stack, take a small backlog item, and have it reviewed.\n\n<!-- pagebreak -->\n\n## Checklist\n\n- [ ] Accounts for git, CI and Kuria\n- [ ] The development stack running\n- [ ] A first pull request merged\n",
    )
    .await;
    w.release(&dev, &onboarding, None).await;
    let old = doc(
        &rm,
        "Release checklist (paper)",
        "# Release checklist\n\nPrint, tick and file in the release binder.\n",
    )
    .await;
    w.release(&rm, &old, None).await;
    w.transition(
        &rm,
        &old,
        "retire",
        None,
        "Replaced by the release procedure",
    )
    .await;
    // A supplier's policy, kept as the PDFs it publishes.
    let supplier = w
        .create(
            &rm,
            "external_document",
            &[&manual],
            json!({ "title": "Hosting supplier: information security policy", "owner": RM, "source": "The hosting supplier" }),
            "Keep the supplier's policy with ours",
        )
        .await;
    let issue = w
        .upload(
            &rm,
            "supplier-security-policy.pdf",
            "application/pdf",
            include_bytes!("examples/supplier-security-policy.pdf").to_vec(),
        )
        .await;
    w.release_file(&rm, &supplier, &issue, Some("Issue 3"))
        .await;
    w.edit(
        &hedda,
        &manual,
        json!({ "folders": [
            { "title": "Policy", "items": [policy] },
            { "title": "Procedures", "items": [control, procedure, { "title": "Retired", "collapsible": true, "items": [old] }] },
            { "title": "Engineering", "items": [onboarding] },
            { "title": "External", "items": [supplier] }
        ] }),
        "Sort the manual into folders",
    )
    .await;

    let handbook = w
        .create(
            &dev,
            "document_index",
            &[],
            json!({ "title": "Engineering handbook 2025", "owner": DEV, "about": "Last year's handbook, merged into the quality manual." }),
            "Last year's handbook",
        )
        .await;
    let style = w
        .create(
            &dev,
            "document",
            &[&handbook],
            json!({ "title": "Code style", "owner": DEV, "body": "# Code style\n\nFormat with the project's formatter; lint warnings fail the build.\n" }),
            "From the old wiki",
        )
        .await;
    w.release(&dev, &style, None).await;
    w.transition(
        &dev,
        &handbook,
        "archive",
        None,
        "Merged into the quality manual",
    )
    .await;
}

/// Plans: one active, with tasks in every state, gaps and decisions, one
/// finished, and one dropped and archived.
#[expect(clippy::too_many_lines, reason = "two plans told from start to end")]
async fn planning(w: &World) {
    let (dev, rm, hedda) = (t(DEV), t(RM), t(HEDDA));

    let plan = w
        .create(
            &rm,
            "plan",
            &[],
            json!({
                "title": "Self-service password reset",
                "owner": RM,
                "goal": "Customers reset a forgotten password themselves, by email, without calling support.\n\nSuccess: support's password tickets drop by half within a month of the release.",
                "design": "## Constraints\n\n- A reset link is valid once, for 30 minutes.\n- Nothing tells whether an email has an account.\n\n## Design\n\nA signed, single-use token in the link; the new password is checked against the breached-password list.",
                "verification": "- The backend's tests cover expiry and reuse.\n- The acceptance protocol has a case for each step."
            }),
            "Plan the password reset",
        )
        .await;
    w.transition(&rm, &plan, "activate", None, "Agreed in planning")
        .await;
    let task = async |title: &str, description: &str, acceptance: &str, assignee: &str| {
        w.create(
            &rm,
            "task",
            &[&plan],
            json!({ "title": title, "description": description, "acceptance": acceptance, "assignee": assignee }),
            "Add the plan's tasks",
        )
        .await
    };
    let spec = task(
        "Spec the reset flow",
        "Write the requirements: request, email, link, new password.",
        "The spec names each step and its failure cases.",
        RM,
    )
    .await;
    let tokens = task(
        "Single-use reset tokens",
        "Signed tokens with an expiry, stored hashed and spent on use.",
        "A spent or expired token is refused; tests cover both.",
        DEV,
    )
    .await;
    let email = task(
        "The reset email",
        "Plain text and HTML, in the shop's languages.",
        "Sent within a minute; the link opens the new-password form.",
        DEV,
    )
    .await;
    let form = task(
        "New-password form",
        "Checks the password against the breached list as you type.",
        "A breached password is refused with a reason.",
        DEV,
    )
    .await;
    let sms = task(
        "Reset by SMS",
        "A code by text message, for customers without email.",
        "",
        DEV,
    )
    .await;
    w.link(&rm, &tokens, "blocks", &email, "The email carries a token")
        .await;
    w.link(
        &rm,
        &email,
        "blocks",
        &form,
        "The form is where the link lands",
    )
    .await;
    w.transition(&rm, &spec, "start", None, "Taken").await;
    w.transition(
        &rm,
        &spec,
        "finish",
        Some(json!({ "change": { "url": "https://git.example/shop/docs/pulls/41", "label": "docs#41" } })),
        "Spec merged",
    )
    .await;
    w.transition(&dev, &tokens, "start", None, "Taken").await;
    w.edit(
        &dev,
        &tokens,
        json!({ "change": pr("backend", 320) }),
        "Opened the pull request",
    )
    .await;
    w.transition(&dev, &tokens, "review", None, "Ready for review")
        .await;
    w.transition(
        &rm,
        &tokens,
        "rework",
        None,
        "Store the tokens hashed, not in clear",
    )
    .await;
    w.transition(&dev, &tokens, "review", None, "Hashed now")
        .await;
    w.transition(&dev, &email, "start", None, "Taken").await;
    w.transition(
        &rm,
        &sms,
        "drop",
        None,
        "Out of scope: every account has an email",
    )
    .await;

    w.create(
            &dev,
            "gap",
            &[&plan],
            json!({ "title": "Which languages does the email need?", "description": "The shop sells in five countries; the templates exist in two languages." }),
            "Found writing the email",
        )
        .await;
    let rate = w
        .create(
            &dev,
            "gap",
            &[&plan],
            json!({ "title": "Limit reset requests", "description": "Nothing stops a script asking for thousands of resets." }),
            "Found in review",
        )
        .await;
    let decision = w
        .create(
            &rm,
            "decision",
            &[&plan],
            json!({ "title": "Tokens expire after 30 minutes" }),
            "Record the expiry",
        )
        .await;
    w.transition(
        &rm,
        &decision,
        "decide",
        Some(json!({
            "context": "Longer links are easier on customers; shorter ones leak less.",
            "decision": "A reset token expires 30 minutes after it is sent, and on first use.",
            "why": "What the bank's and the mail provider's own resets do; support saw no complaints there.",
            "decided_by": RM
        })),
        "Decided in planning",
    )
    .await;
    let limit = w
        .create(
            &dev,
            "decision",
            &[&plan],
            json!({ "title": "Five reset requests an hour per email" }),
            "Answer the rate-limit gap",
        )
        .await;
    w.transition(
        &rm,
        &limit,
        "decide",
        Some(json!({
            "context": "Reset requests send email, which costs money and can be abused.",
            "decision": "At most five reset requests an hour for one email, and fifty for one address.",
            "why": "No real customer needs more; the limit is the sign-in limit's.",
            "decided_by": RM
        })),
        "Decided with security",
    )
    .await;
    w.link(&dev, &rate, "resolved_by", &limit, "The limit answers it")
        .await;
    w.transition(
        &dev,
        &rate,
        "close",
        Some(json!({ "resolution": "Limited as the decision says." })),
        "Resolved",
    )
    .await;
    let longer = w
        .create(
            &rm,
            "decision",
            &[&plan],
            json!({ "title": "Tokens expire after an hour" }),
            "Support asked for longer links",
        )
        .await;
    w.transition(
        &rm,
        &longer,
        "decide",
        Some(json!({
            "context": "Customers on slow email providers got links that had already expired.",
            "decision": "A reset token expires an hour after it is sent, and on first use.",
            "why": "Support saw expired links daily at 30 minutes; an hour covers the slowest provider.",
            "decided_by": HEDDA
        })),
        "Decided with the head of product",
    )
    .await;
    w.link(&rm, &longer, "supersedes", &decision, "Longer links")
        .await;
    w.transition(&rm, &decision, "supersede", None, "Superseded by the hour")
        .await;

    // A plan done, and one dropped.
    let done = w
        .create(
            &hedda,
            "plan",
            &[],
            json!({
                "title": "Order confirmation emails in the customer's language",
                "owner": HEDDA,
                "goal": "Every order confirmation is in the language the customer shopped in."
            }),
            "Plan the translated confirmations",
        )
        .await;
    w.transition(&hedda, &done, "activate", None, "Agreed")
        .await;
    for (title, n) in [
        ("Store the shop language with the order", 290),
        ("Translate the confirmation templates", 291),
    ] {
        let task = w
            .create(
                &hedda,
                "task",
                &[&done],
                json!({ "title": title, "assignee": DEV }),
                "Add the plan's tasks",
            )
            .await;
        w.transition(&dev, &task, "start", None, "Taken").await;
        w.transition(
            &dev,
            &task,
            "finish",
            Some(json!({ "change": pr("backend", n) })),
            "Merged",
        )
        .await;
    }
    w.transition(&hedda, &done, "finish", None, "Shipped in 4.1")
        .await;
    let dropped = w
        .create(
            &dev,
            "plan",
            &[],
            json!({ "title": "A native mobile app", "owner": DEV }),
            "An idea",
        )
        .await;
    w.transition(
        &hedda,
        &dropped,
        "drop",
        None,
        "The web shop works well on phones",
    )
    .await;
    w.ok(
        &hedda,
        "mutation($n: String!) { archiveAkt(input: { number: $n, reason: \"Out of the way of the plans being worked\" }) { number } }",
        json!({ "n": dropped }),
    )
    .await;
}

/// The shop's acceptance test, kept as a template: five test cases in three
/// sections, for the developer and the head of product to run.
async fn acceptance_template(w: &World) -> String {
    let rm = t(RM);
    let template = w
        .create(
            &rm,
            "test_protocol",
            &[],
            json!({ "title": "Shop acceptance test", "owner": RM,
                    "scope": "A release candidate of the web shop, on staging, before it is accepted." }),
            "The protocol every candidate runs",
        )
        .await;
    let case = async |title: &str, steps: &str, expected: &str| {
        w.create(
            &rm,
            "test_case",
            &[&template],
            json!({ "title": title, "steps": steps, "expected": expected, "testers": [DEV, HEDDA] }),
            "Add the protocol's cases",
        )
        .await
    };
    let browse = case(
        "Browse and search",
        "1. Open the shop.\n2. Search for `mug`.\n3. Open a product.",
        "Results in under a second; the product page shows price and stock.",
    )
    .await;
    let basket = case(
        "Fill the basket",
        "1. Add two products.\n2. Change a quantity.\n3. Remove one.",
        "The total follows every change.",
    )
    .await;
    let guest = case(
        "Pay as a guest",
        "1. Check out without signing in.\n2. Pay with the test card.",
        "An order confirmation, by email too.",
    )
    .await;
    let passkey = case(
        "Sign in with a passkey",
        "1. Sign in.\n2. Choose the passkey.",
        "Signed in, without a password.",
    )
    .await;
    let reset = case(
        "Reset a password",
        "1. Ask for a reset.\n2. Follow the email's link.\n3. Set a new password.",
        "Signed in with the new password; the link no longer works.",
    )
    .await;
    w.edit(
        &rm,
        &template,
        json!({ "layout": [
            { "title": "Shopping", "items": [browse, basket] },
            { "title": "Checkout", "text": "With the payment provider's *test* card.", "items": [guest] },
            { "title": "Accounts", "collapsible": true, "items": [passkey, reset] }
        ] }),
        "Sections for the cases",
    )
    .await;
    w.transition(&rm, &template, "keep", None, "Reviewed: a template now")
        .await;
    template
}

/// A tester's verdict on a test case, and its description.
type Said = (&'static str, Option<&'static str>);

/// A run of `template` for `candidate`: a copy made in the candidate, named
/// after its `build`, started, with a verdict from each tester on every
/// test case, `verdicts` answering the developer's and the head of
/// product's for the test case of a title, each with its description, and
/// finished with `conclusion`.
/// Answers the run's number.
async fn acceptance_run(
    w: &World,
    template: &str,
    candidate: &str,
    build: &str,
    verdicts: impl Fn(&str) -> (Said, Said),
    conclusion: &str,
) -> String {
    let (dev, rm, hedda) = (t(DEV), t(RM), t(HEDDA));
    let run = w
        .ok(
            &rm,
            "mutation($n: String!, $in: [String!], $why: String) { copyAkt(input: { number: $n, in: $in, reason: $why }) { number } }",
            json!({ "n": template, "in": [candidate], "why": format!("Run it for {build}") }),
        )
        .await["copyAkt"]["number"]
        .as_str()
        .unwrap()
        .to_owned();
    w.edit(
        &rm,
        &run,
        json!({ "title": format!("Shop acceptance test: {build}") }),
        "Name the run",
    )
    .await;
    w.transition(&rm, &run, "start", None, "Testing on staging")
        .await;
    let members = w
        .ok(
            &rm,
            "query($n: String!) { akt(number: $n) { members { nodes { number title } } } }",
            json!({ "n": run }),
        )
        .await;
    for m in members["akt"]["members"]["nodes"].as_array().unwrap() {
        let number = m["number"].as_str().unwrap();
        let (devs, heddas) = verdicts(m["title"].as_str().unwrap());
        for (tester, (verdict, description)) in [(&dev, devs), (&hedda, heddas)] {
            let verdict = match description {
                Some(description) => json!({ "verdict": verdict, "description": description }),
                None => json!(verdict),
            };
            w.edit(tester, number, json!({ "verdict": verdict }), "Tested")
                .await;
        }
    }
    w.transition(
        &rm,
        &run,
        "finish",
        Some(json!({ "conclusion": conclusion })),
        "Every tester has answered",
    )
    .await;
    run
}
