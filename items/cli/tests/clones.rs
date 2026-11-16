//! Clones against a Kuria server on a local port (`clone.*`): the documents
//! example, its documents with live drafts, cloned, edited, staged, pushed
//! and pulled as `kuria` does, while the app edits a live draft meanwhile,
//! and with git as the person's own.

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use kuria_cli::clone::{sync, work};
use kuria_cli::error::CliError;
use serde_json::{Value, json};

use support::Server;

const INDEX: &str = "DOCS-1";
const SETUP: &str = "DOCS-1-D01";
const GUIDE: &str = "DOCS-1-D02";
const SETUP_FILE: &str = "guides/DOCS-1-D01-setup.md";
const GUIDE_FILE: &str = "DOCS-1-D02-guide.md";
const INDEX_FILE: &str = "DOCS-1-handbook.md";

/// Setup's body: paragraphs far enough apart to be hunks of their own.
const SETUP_BODY: &str = "## Install {#install}

Run the installer.

One.

Two.

Three.

Four.

## Use {#use}

Open it.
";

/// The documents example's akt types, documents with live drafts.
fn types() -> Vec<Value> {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/documents");
    ["image", "document", "document_index"]
        .iter()
        .map(|key| {
            let text = std::fs::read_to_string(example.join(format!("{key}.json"))).unwrap();
            let mut document: Value = serde_json::from_str(&text).unwrap();
            if *key == "document" {
                document["live_drafts"] = json!(true);
            }
            document
        })
        .collect()
}

/// A Kuria with the index `DOCS-1`, "Handbook", whose folder "Guides" holds
/// Setup, and Guide outside any folder; and a directory to clone into.
struct World {
    server: Server,
    dir: tempfile::TempDir,
}

impl World {
    async fn new() -> Self {
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
        let index = server
            .ok(
                create,
                json!({ "t": "document_index", "in": [], "f": { "title": "Handbook", "about": "What it is for." } }),
            )
            .await;
        assert_eq!(index["createAkt"]["number"], INDEX);
        for (title, body) in [("Setup", SETUP_BODY), ("Guide", "Read it.\n")] {
            server
                .ok(
                    create,
                    json!({ "t": "document", "in": [INDEX], "f": { "title": title, "body": body } }),
                )
                .await;
        }
        let world = Self {
            server,
            dir: tempfile::tempdir().unwrap(),
        };
        let revision = world.revision(INDEX).await;
        world
            .server
            .ok(
                "mutation($n: String!, $v: Int!, $f: JSON!) {
                   editAkt(input: { number: $n, revision: $v, fields: $f }) { revision } }",
                json!({ "n": INDEX, "v": revision, "f": { "folders": [{ "title": "Guides", "items": [SETUP] }, GUIDE] } }),
            )
            .await;
        world
    }

    async fn revision(&self, number: &str) -> i64 {
        self.server
            .ok(
                "query($n: String!) { akt(number: $n) { revision } }",
                json!({ "n": number }),
            )
            .await["akt"]["revision"]
            .as_i64()
            .unwrap()
    }

    /// The body of `number` as Kuria has it, its live draft's for a
    /// document.
    async fn body(&self, number: &str) -> String {
        let data = self
            .server
            .ok(
                "query($n: String!) { akt(number: $n) { fields { ... on MarkdownValue { field markdown } } } }",
                json!({ "n": number }),
            )
            .await;
        data["akt"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["field"] == "body")
            .map(|f| f["markdown"].as_str().unwrap().to_owned())
            .unwrap_or_default()
    }

    /// Edits `number`'s body as the app does, in its live draft.
    async fn edit_live(&self, number: &str, body: &str) {
        let revision = self.revision(number).await;
        self.server
            .ok(
                "mutation($n: String!, $v: Int!, $f: JSON!) {
                   editAkt(input: { number: $n, revision: $v, fields: $f }) { revision } }",
                json!({ "n": number, "v": revision, "f": { "body": body } }),
            )
            .await;
    }

    /// Clones the index, answering the clone's root.
    async fn clone(&self) -> PathBuf {
        let answer = sync::clone(&self.server.session, INDEX, None, self.dir.path())
            .await
            .unwrap();
        PathBuf::from(answer.json["dir"].as_str().unwrap())
    }
}

fn read(root: &Path, path: &str) -> String {
    std::fs::read_to_string(root.join(path)).unwrap()
}

/// Replaces `from` with `to` in the file at `path`.
fn edit(root: &Path, path: &str, from: &str, to: &str) {
    let text = read(root, path);
    assert!(text.contains(from), "{path} doesn't hold {from:?}:\n{text}");
    std::fs::write(root.join(path), text.replacen(from, to, 1)).unwrap();
}

fn paths(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|p| (*p).to_owned()).collect()
}

fn status(root: &Path) -> Value {
    work::status(root).unwrap().json
}

// r[verify clone.clone]
// r[verify clone.tree]
// r[verify clone.normal]
// r[verify clone.status]
#[tokio::test(flavor = "multi_thread")]
async fn clones_an_index_with_its_base() {
    let world = World::new().await;
    let session = &world.server.session;
    let root = world.clone().await;
    assert!(root.ends_with("DOCS-1-handbook"), "{}", root.display());
    let setup = read(&root, SETUP_FILE);
    assert!(
        setup.starts_with(
            "---\nnumber: DOCS-1-D01\ntype: document\nstate: draft\ntitle: Setup\n---\n"
        ),
        "{setup}"
    );
    assert!(setup.contains("## Install {#install}"), "{setup}");
    assert!(read(&root, GUIDE_FILE).contains("Read it."));
    assert!(read(&root, INDEX_FILE).contains("What it is for."));
    let bases: Value = serde_json::from_str(&read(&root, ".kuria/base.json")).unwrap();
    assert!(bases["akter"][SETUP]["live"]["base"].is_string(), "{bases}");
    assert!(bases["akter"][INDEX]["live"].is_null(), "{bases}");
    assert_eq!(read(&root, ".kuria/.gitignore"), "*\n");
    assert_eq!(read(&root, &format!(".kuria/base/{SETUP_FILE}")), setup);
    let clone_json: Value = serde_json::from_str(&read(&root, ".kuria/clone.json")).unwrap();
    assert_eq!(
        clone_json,
        json!({ "url": world.server.url, "number": INDEX })
    );
    assert_eq!(status(&root)["staged"], json!([]));
    // `kuria docs fmt` works in a clone, which is in normal form.
    let formatted = kuria_cli::clone::fmt::fmt(session, &root).await.unwrap();
    assert_eq!(
        formatted.json["changed"],
        json!([]),
        "{}",
        formatted.markdown
    );
    // A clone is of a container the subject may read.
    let refused = sync::clone(session, SETUP, None, world.dir.path())
        .await
        .unwrap_err();
    assert_eq!(refused.code, "NOT_A_CONTAINER");
    let refused = sync::clone(session, "DOCS-9", None, world.dir.path())
        .await
        .unwrap_err();
    assert_eq!(refused.code, "NOT_FOUND");
}

// r[verify clone.status]
// r[verify clone.push]
// r[verify clone.server]
#[tokio::test(flavor = "multi_thread")]
async fn pushes_live_drafts_and_revisions() {
    let world = World::new().await;
    let session = &world.server.session;
    let root = world.clone().await;
    // Edited: the live document, the index without a live draft.
    edit(
        &root,
        SETUP_FILE,
        "Run the installer.",
        "Run the installer twice.",
    );
    edit(&root, INDEX_FILE, "What it is for.", "What it is all for.");
    let s = status(&root);
    assert_eq!(s["unstaged"], json!([INDEX_FILE, SETUP_FILE]), "{s}");
    let diff = work::diff(&root, false, &[]).unwrap().markdown;
    assert!(diff.contains("+Run the installer twice."), "{diff}");
    // Nothing staged: nothing to push.
    let refused = sync::push(session, &root, "Nothing").await.unwrap_err();
    assert!(
        refused.message.contains("Nothing is staged"),
        "{}",
        refused.message
    );
    work::add(&root, &paths(&[SETUP_FILE, INDEX_FILE])).unwrap();
    assert_eq!(status(&root)["staged"], json!([INDEX_FILE, SETUP_FILE]));
    let index_revision = world.revision(INDEX).await;
    let pushed = sync::push(session, &root, "Tidy the setup").await.unwrap();
    assert_eq!(
        pushed.json["pushed"],
        json!([INDEX_FILE, SETUP_FILE]),
        "{}",
        pushed.markdown
    );

    // The live draft changed as one change with the message; the index got
    // a revision with it as its reason.
    assert!(world.body(SETUP).await.contains("Run the installer twice."));
    let changes = world
        .server
        .ok(
            "query($n: String!) { akt(number: $n) { draftChanges(all: true) { fields message } } }",
            json!({ "n": SETUP }),
        )
        .await;
    let last = changes["akt"]["draftChanges"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        last,
        json!({ "fields": ["body"], "message": "Tidy the setup" })
    );
    let index = world
        .server
        .ok(
            "query($n: String!) { akt(number: $n) { revision revisions { revision reason } fields { ... on TextValue { field text } } } }",
            json!({ "n": INDEX }),
        )
        .await;
    assert_eq!(index["akt"]["revision"], index_revision + 1, "{index}");
    let reason = index["akt"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["revision"] == index_revision + 1)
        .map(|r| r["reason"].clone());
    assert_eq!(reason, Some(json!("Tidy the setup")));
    // The base is what was pushed, and nothing is left to push.
    let s = status(&root);
    assert_eq!(
        (s["staged"].clone(), s["unstaged"].clone()),
        (json!([]), json!([])),
        "{s}"
    );
    edit(&root, SETUP_FILE, "Open it.", "Open it now.");
    work::add(&root, &paths(&[SETUP_FILE])).unwrap();
    sync::push(session, &root, "Open it now").await.unwrap();
    assert!(world.body(SETUP).await.contains("Open it now."));

    // New and removed files are refused: akter are made in Kuria.
    std::fs::write(root.join("new.md"), "---\nnumber: DOCS-1-D09\n---\n").unwrap();
    assert!(work::add(&root, &paths(&["new.md"])).is_err());
    std::fs::remove_file(root.join(GUIDE_FILE)).unwrap();
    assert!(work::add(&root, &paths(&[GUIDE_FILE])).is_err());
    assert_eq!(status(&root)["removed"], json!([GUIDE_FILE]));
}

// r[verify clone.add]
// r[verify clone.push]
#[tokio::test(flavor = "multi_thread")]
async fn pushes_only_the_hunk_staged() {
    let world = World::new().await;
    let session = &world.server.session;
    let root = world.clone().await;
    edit(
        &root,
        SETUP_FILE,
        "Run the installer.",
        "Run the installer, once.",
    );
    edit(&root, SETUP_FILE, "Open it.", "Open it, then close it.");
    let mut prompts = Vec::new();
    let staged = work::add_patch(&root, &[], &mut "y\nn\n".as_bytes(), &mut prompts).unwrap();
    assert_eq!(staged.json["staged"], json!([SETUP_FILE]));
    let prompts = String::from_utf8(prompts).unwrap();
    assert!(prompts.contains("(1/2) Stage this hunk"), "{prompts}");
    let s = status(&root);
    assert_eq!(
        (s["staged"].clone(), s["unstaged"].clone()),
        (json!([SETUP_FILE]), json!([SETUP_FILE])),
        "{s}"
    );
    sync::push(session, &root, "Run it once").await.unwrap();
    let body = world.body(SETUP).await;
    assert!(body.contains("Run the installer, once."), "{body}");
    assert!(
        body.contains("Open it.") && !body.contains("close it"),
        "{body}"
    );
    // What wasn't staged stays in the working file, to push next.
    assert!(read(&root, SETUP_FILE).contains("Open it, then close it."));
    let s = status(&root);
    assert_eq!(
        (s["staged"].clone(), s["unstaged"].clone()),
        (json!([]), json!([SETUP_FILE])),
        "{s}"
    );
    let diff = work::diff(&root, false, &paths(&[SETUP_FILE]))
        .unwrap()
        .markdown;
    assert!(
        diff.contains("+Open it, then close it.") && !diff.contains("once"),
        "{diff}"
    );
}

// r[verify clone.push]
// r[verify clone.pull]
#[tokio::test(flavor = "multi_thread")]
async fn a_push_behind_a_live_edit_is_refused_until_pulled() {
    let world = World::new().await;
    let session = &world.server.session;
    let root = world.clone().await;
    // Typed in the app meanwhile.
    world
        .edit_live(SETUP, &SETUP_BODY.replace("One.", "One, typed live."))
        .await;
    edit(&root, SETUP_FILE, "Four.", "Four, edited here.");
    work::add(&root, &paths(&[SETUP_FILE])).unwrap();
    let refused: CliError = sync::push(session, &root, "Four").await.unwrap_err();
    assert_eq!(refused.code, "STALE");
    assert!(
        refused.message.contains("kuria docs pull"),
        "{}",
        refused.message
    );
    assert!(!world.body(SETUP).await.contains("edited here"));
    assert_eq!(
        status(&root)["staged"],
        json!([SETUP_FILE]),
        "staging nothing away"
    );

    let pulled = sync::pull(session, &root).await.unwrap();
    assert_eq!(
        pulled.json["merged"],
        json!([SETUP_FILE]),
        "{}",
        pulled.markdown
    );
    assert_eq!(pulled.json["conflicted"], json!([]));
    let file = read(&root, SETUP_FILE);
    assert!(
        file.contains("One, typed live.") && file.contains("Four, edited here."),
        "{file}"
    );
    // Still staged, against the new base: only the change made here.
    assert_eq!(status(&root)["staged"], json!([SETUP_FILE]));
    let staged = work::diff(&root, true, &[]).unwrap().markdown;
    assert!(
        staged.contains("+Four, edited here.") && !staged.contains("+One"),
        "{staged}"
    );
    sync::push(session, &root, "Four").await.unwrap();
    let body = world.body(SETUP).await;
    assert!(
        body.contains("One, typed live.") && body.contains("Four, edited here."),
        "{body}"
    );
}

// r[verify clone.pull]
// r[verify clone.add]
#[tokio::test(flavor = "multi_thread")]
async fn a_conflict_is_marked_resolved_and_pushed() {
    let world = World::new().await;
    let session = &world.server.session;
    let root = world.clone().await;
    world
        .edit_live(SETUP, &SETUP_BODY.replace("Two.", "Two, as Kuria has it."))
        .await;
    edit(&root, SETUP_FILE, "Two.", "Two, as I have it.");
    work::add(&root, &paths(&[SETUP_FILE])).unwrap();
    let pulled = sync::pull(session, &root).await.unwrap();
    assert_eq!(
        pulled.json["conflicted"],
        json!([SETUP_FILE]),
        "{}",
        pulled.markdown
    );
    assert_eq!(pulled.json["unstaged"], json!([SETUP_FILE]));
    let file = read(&root, SETUP_FILE);
    assert!(
        file.contains(
            "<<<<<<< ours\nTwo, as I have it.\n=======\nTwo, as Kuria has it.\n>>>>>>> kuria\n"
        ),
        "{file}"
    );
    assert_eq!(status(&root)["conflicted"], json!([SETUP_FILE]));
    assert_eq!(status(&root)["staged"], json!([]));
    let refused = work::add(&root, &paths(&[SETUP_FILE])).unwrap_err();
    assert_eq!(refused.code, "CONFLICTED");

    // Resolved by hand.
    let start = file.find("<<<<<<< ours").unwrap();
    let end = file.find(">>>>>>> kuria\n").unwrap() + ">>>>>>> kuria\n".len();
    let resolved = format!(
        "{}Two, as we both have it.\n{}",
        &file[..start],
        &file[end..]
    );
    std::fs::write(root.join(SETUP_FILE), resolved).unwrap();
    work::add(&root, &paths(&[SETUP_FILE])).unwrap();
    sync::push(session, &root, "Agree on two").await.unwrap();
    let body = world.body(SETUP).await;
    assert!(
        body.contains("Two, as we both have it.") && !body.contains("as Kuria"),
        "{body}"
    );
}

fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.email=me@kuria.example", "-c", "user.name=Me"])
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// r[verify clone.push]
// r[verify clone.status]
#[tokio::test(flavor = "multi_thread")]
async fn a_branch_keeps_what_isn_t_ready_while_the_rest_is_pushed() {
    let world = World::new().await;
    let session = &world.server.session;
    let root = world.clone().await;
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "Clone"]);
    assert!(
        !git(&root, &["ls-files"]).contains(".kuria"),
        ".kuria/ ignores itself"
    );
    // Ready, and not yet.
    edit(
        &root,
        SETUP_FILE,
        "Run the installer.",
        "Run the new installer.",
    );
    git(&root, &["commit", "-q", "-am", "Ready"]);
    edit(&root, GUIDE_FILE, "Read it.", "Read it all, once released.");
    git(&root, &["commit", "-q", "-am", "Not yet"]);

    // What isn't ready kept on a branch, taken out of the working files.
    git(&root, &["branch", "keep"]);
    git(&root, &["reset", "-q", "--hard", "HEAD~1"]);
    assert_eq!(status(&root)["unstaged"], json!([SETUP_FILE]));
    work::add(&root, &paths(&["."])).unwrap();
    sync::push(session, &root, "The new installer")
        .await
        .unwrap();
    assert!(world.body(SETUP).await.contains("Run the new installer."));
    assert_eq!(world.body(GUIDE).await, "Read it.\n");

    // Released as it is.
    let revision = world.revision(SETUP).await;
    let released = world
        .server
        .ok(
            "mutation($n: String!, $v: Int!) { releaseVersion(input: { number: $n, revision: $v }) { number } }",
            json!({ "n": SETUP, "v": revision }),
        )
        .await;
    assert_eq!(released["releaseVersion"]["number"], 1);

    // Then the rest.
    git(&root, &["merge", "-q", "--no-edit", "keep"]);
    assert!(read(&root, GUIDE_FILE).contains("Read it all, once released."));
    assert_eq!(status(&root)["unstaged"], json!([GUIDE_FILE]));
    work::add(&root, &paths(&["."])).unwrap();
    sync::push(session, &root, "Read it all").await.unwrap();
    assert!(
        world
            .body(GUIDE)
            .await
            .contains("Read it all, once released.")
    );
    assert!(world.body(SETUP).await.contains("Run the new installer."));
    let s = status(&root);
    assert_eq!(
        (s["staged"].clone(), s["unstaged"].clone()),
        (json!([]), json!([])),
        "{s}"
    );
}
