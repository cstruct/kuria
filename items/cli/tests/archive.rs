//! Archiving against a Kuria server on a local port (`cli.archive`):
//! `archive` and `unarchive`, and lists, files and `show` leaving archived
//! akter out unless asked for, saying they are archived when read.

mod support;

use kuria_cli::reads::{self, Archived, Listing, Source, WithMembers};
use kuria_cli::writes;
use serde_json::{Value, json};

use support::Server;

/// Plans whose tasks are archived with them.
fn types() -> [Value; 2] {
    [
        json!({
            "key": "task", "name": "Task", "number": "TASK-{seq}",
            "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
            "states": [{ "key": "todo", "name": "To do", "initial": true }],
            "archive": { "rules": [{ "with_container": "plan" }] }
        }),
        json!({
            "key": "plan", "name": "Plan", "number": "PLAN-{seq:3}",
            "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
            "states": [{ "key": "open", "name": "Open", "initial": true }],
            "contains": [{ "type": "task" }]
        }),
    ]
}

fn numbers(answer: &Value) -> Vec<String> {
    let mut numbers: Vec<String> = answer["akter"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["number"].as_str().unwrap().to_owned())
        .collect();
    numbers.sort();
    numbers
}

fn listing(archived: Archived) -> Listing {
    Listing {
        limit: 50,
        archived,
        ..Listing::default()
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "archiving and its reads, told in one place"
)]
// r[verify cli.archive]
#[tokio::test(flavor = "multi_thread")]
async fn archives_in_order_and_lists_archived_akter_only_when_asked() {
    let server = Server::start().await;
    for ty in types() {
        server
            .ok(
                "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                json!({ "d": ty }),
            )
            .await;
    }
    let create = "mutation($t: String!, $f: JSON, $in: [String!]!) {
        createAkt(input: { type: $t, fields: $f, in: $in }) { number } }";
    for (ty, title, inside) in [
        ("plan", "Autumn", vec![]),
        ("task", "First", vec!["PLAN-001"]),
        ("task", "Second", vec!["PLAN-001"]),
        ("plan", "Winter", vec![]),
    ] {
        server
            .ok(
                create,
                json!({ "t": ty, "f": { "title": title }, "in": inside }),
            )
            .await;
    }
    let session = &server.session;
    let source = Source::Server(session);
    let names = |n: &[&str]| n.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();

    // In order, stopping at the first refused, what came before archived.
    let refused = writes::archive::archive(
        session,
        &names(&["PLAN-001", "NOPE-1", "PLAN-002"]),
        Some("Done with"),
        true,
    )
    .await
    .unwrap_err();
    assert_eq!(refused.code, "NOT_FOUND");
    assert!(
        refused
            .message
            .starts_with("Archived PLAN-001 (revision 4).\n"),
        "{}",
        refused.message
    );

    let live = reads::ls(source, &listing(Archived::Live)).await.unwrap();
    assert_eq!(numbers(&live.json), ["PLAN-002"]);
    let gone = reads::ls(source, &listing(Archived::Archived))
        .await
        .unwrap();
    assert_eq!(numbers(&gone.json), ["PLAN-001", "TASK-1", "TASK-2"]);
    assert!(
        gone.markdown.contains("Open (archived)"),
        "{}",
        gone.markdown
    );
    let plan = gone.json["akter"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["number"] == "PLAN-001")
        .unwrap();
    assert_eq!(plan["archived"]["reason"], "Done with");
    assert!(plan["archived"]["rule"].is_null());
    assert_eq!(
        numbers(
            &reads::ls(source, &listing(Archived::All))
                .await
                .unwrap()
                .json
        )
        .len(),
        4
    );
    let members = |archived| Listing {
        inside: Some("PLAN-001".to_owned()),
        ..listing(archived)
    };
    let none = reads::ls(source, &members(Archived::Live)).await.unwrap();
    assert_eq!(numbers(&none.json), Vec::<String>::new());
    assert_eq!(
        numbers(
            &reads::ls(source, &members(Archived::Archived))
                .await
                .unwrap()
                .json
        ),
        ["TASK-1", "TASK-2"]
    );
    let found = |archived| reads::find(source, "Autumn", 50, archived);
    assert_eq!(
        numbers(&found(Archived::Live).await.unwrap().json),
        Vec::<String>::new()
    );
    assert_eq!(
        numbers(&found(Archived::All).await.unwrap().json),
        ["PLAN-001"]
    );

    // Read by its number, it says it is archived; its members follow only
    // when asked.
    let numbers_of = |n: &[&str]| names(n);
    let file = reads::cat(
        source,
        &numbers_of(&["PLAN-001"]),
        None,
        WithMembers::All,
        None,
        Archived::Live,
    )
    .await
    .unwrap();
    assert!(
        file.markdown.contains("archived: {at: "),
        "{}",
        file.markdown
    );
    assert!(file.markdown.contains("reason: Done with}"));
    assert_eq!(file.json["akter"].as_array().unwrap().len(), 1);
    let all = reads::cat(
        source,
        &numbers_of(&["PLAN-001"]),
        None,
        WithMembers::All,
        None,
        Archived::All,
    )
    .await
    .unwrap();
    assert_eq!(all.json["akter"].as_array().unwrap().len(), 3);
    assert_eq!(all.json["akter"][1]["archived"]["rule"], "with_container");
    let shown = reads::show(source, "PLAN-001", false, None, Archived::Live)
        .await
        .unwrap();
    assert!(
        shown.markdown.contains(": Done with\n"),
        "{}",
        shown.markdown
    );
    assert!(
        shown
            .markdown
            .contains("TASK-1 · First (Task, To do, archived)"),
        "an archived container's board shows its members: {}",
        shown.markdown
    );

    // As an akt directory, archived akter are listed in
    // `akter/archived.md`, each kept in `akter/archived/`, and read the
    // same.
    let files = writes::bundles::fetch(session, "types=plan,task&akter=true")
        .await
        .unwrap();
    let bundle = writes::bundles::bundle_json(&files).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("plans");
    kuria_cli::local::convert::to_dir(
        &kuria_cli::local::convert::Unpacked::of(bundle, &files),
        &dir,
    )
    .unwrap();
    let local = reads::Local::open(&dir, None).unwrap();
    let there = reads::ls(Source::Local(&local), &listing(Archived::Archived))
        .await
        .unwrap();
    assert_eq!(numbers(&there.json), numbers(&gone.json));
    let how = |answer: &Value| {
        let mut how: Vec<(String, Value, Value)> = answer["akter"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    a["number"].as_str().unwrap().to_owned(),
                    a["archived"]["rule"].clone(),
                    a["archived"]["reason"].clone(),
                )
            })
            .collect();
        how.sort_by(|a, b| a.0.cmp(&b.0));
        how
    };
    assert_eq!(how(&there.json), how(&gone.json));
    assert!(dir.join("akter/archived/TASK-1.md").is_file());

    // Read-only until unarchived, which brings its tasks back.
    let refused = writes::set(session, "TASK-1", &names(&["title=Again"]), None)
        .await
        .unwrap_err();
    assert_eq!(refused.code, "ARCHIVED");
    let back = writes::archive::archive(session, &names(&["PLAN-001"]), None, false)
        .await
        .unwrap();
    assert_eq!(
        back.json,
        json!({ "unarchived": [{ "number": "PLAN-001", "revision": 5 }] })
    );
    assert_eq!(back.markdown, "Unarchived PLAN-001 (revision 5).");
    let live = reads::ls(source, &listing(Archived::Live)).await.unwrap();
    assert_eq!(live.json["akter"].as_array().unwrap().len(), 4);
    assert!(live.json["akter"][0]["archived"].is_null());
    writes::set(session, "TASK-1", &names(&["title=Again"]), None)
        .await
        .unwrap();
}
