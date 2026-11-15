//! Repositories Kuria hosts, driven with the real `git` binary over HTTP
//! against the server on a local port, with storage in memory
//! (`git.repo.*`).

mod support;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use kuria_test_support::TestDatabase;
use serde_json::{Value, json};

use support::decider::FakeDecider;
use support::{FakeHost, graphql_with, router_with, send};

/// Reads every type, edits none.
const READER: &str = "tenant-reader";

fn index() -> Value {
    json!({
        "key": "index", "name": "Index", "number": "IX-{seq}", "git": true,
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "folders", "name": "Folders", "kind": "sections", "of": ["doc"] }
        ],
        "states": [{ "key": "open", "name": "Open", "initial": true }],
        "contains": [{ "type": "doc" }]
    })
}

fn doc() -> Value {
    json!({
        "key": "doc", "name": "Doc", "number": "DOC-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "body", "name": "Body", "kind": "markdown" }
        ],
        "states": [{ "key": "draft", "name": "Draft", "initial": true }]
    })
}

/// A plain container, which isn't a repository.
fn folder() -> Value {
    json!({
        "key": "folder", "name": "Folder", "number": "F-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [{ "key": "open", "name": "Open", "initial": true }],
        "contains": [{ "type": "doc" }]
    })
}

struct World {
    _db: TestDatabase,
    router: Router,
    /// The server, as git addresses it.
    url: String,
    /// Where the test's clones are.
    dir: tempfile::TempDir,
}

impl World {
    async fn new() -> Self {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
        let db = TestDatabase::new().await;
        let decider: Arc<FakeDecider> = FakeDecider::allowing_tenant();
        decider.allow_everything("tenant-dpop");
        decider.grant(READER, &[kuria_server::authorize::READ]);
        let router = router_with(&FakeHost::new(), db.pool().clone(), decider);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let served = router
            .clone()
            .into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move { axum::serve(listener, served).await.unwrap() });
        let world = Self {
            _db: db,
            router,
            url,
            dir: tempfile::tempdir().unwrap(),
        };
        for document in [doc(), index(), folder()] {
            world
                .ok(
                    "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
                    json!({ "d": document }),
                )
                .await;
        }
        world
    }

    async fn ok(&self, query: &str, variables: Value) -> Value {
        let answer = send(&self.router, graphql_with("tenant", query, &variables))
            .await
            .json();
        assert!(answer.get("errors").is_none(), "{answer}");
        answer["data"].clone()
    }

    async fn create(&self, ty: &str, inside: &[&str], fields: Value) -> String {
        self.ok(
            "mutation($t: String!, $i: [String!]!, $f: JSON, $r: String) { createAkt(input: { type: $t, in: $i, fields: $f, reason: $r }) { number } }",
            json!({ "t": ty, "i": inside, "f": fields, "r": format!("Add {ty} {}", fields["title"]) }),
        )
        .await["createAkt"]["number"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    async fn edit(&self, number: &str, fields: Value) {
        let revision = self.revision(number).await;
        self.ok(
            "mutation($n: String!, $r: Int!, $f: JSON!) { editAkt(input: { number: $n, revision: $r, fields: $f, reason: \"Edit\" }) { number } }",
            json!({ "n": number, "r": revision, "f": fields }),
        )
        .await;
    }

    async fn revision(&self, number: &str) -> i64 {
        self.ok(
            "query($n: String!) { akt(number: $n) { revision } }",
            json!({ "n": number }),
        )
        .await["akt"]["revision"]
            .as_i64()
            .unwrap()
    }

    async fn body(&self, number: &str) -> String {
        let fields = self
            .ok(
                "query($n: String!) { akt(number: $n) { fields { ... on MarkdownValue { field markdown } } } }",
                json!({ "n": number }),
            )
            .await;
        fields["akt"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["field"] == "body")
            .map(|f| f["markdown"].as_str().unwrap().to_owned())
            .unwrap_or_default()
    }

    /// An index with two documents, one edited.
    async fn documents(&self) -> String {
        let ix = self
            .create("index", &[], json!({ "title": "Handbook" }))
            .await;
        let first = self
            .create(
                "doc",
                &[&ix],
                json!({ "title": "Intro", "body": "# Welcome\n\nHello.\n" }),
            )
            .await;
        self.create(
            "doc",
            &[&ix],
            json!({ "title": "Usage", "body": "Use it.\n" }),
        )
        .await;
        self.edit(&first, json!({ "body": "# Welcome\n\nHello, reader.\n" }))
            .await;
        ix
    }

    fn repo(&self, number: &str) -> String {
        format!("{}/git/{number}.git", self.url)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Clones `number` into `name`, signed in with `token`.
    async fn clone(&self, number: &str, name: &str, token: &str) -> std::process::Output {
        let into = self.path(name);
        git(
            self.dir.path(),
            token,
            &["clone", &self.repo(number), &into.to_string_lossy()],
        )
        .await
    }
}

/// Runs `git args…` in `dir`, giving git `token` as `kuria git-credential`
/// would, as Basic credentials, and answers what it did.
async fn git(dir: &Path, token: &str, args: &[&str]) -> std::process::Output {
    let helper = format!("!f() {{ echo username=kuria; echo password={token}; }}; f");
    tokio::process::Command::new("git")
        .arg("-c")
        .arg("credential.helper=")
        .arg("-c")
        .arg(format!("credential.helper={helper}"))
        .args([
            "-c",
            "user.name=Ada Lovelace",
            "-c",
            "user.email=ada@example.com",
        ])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "advice.detachedHead=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .await
        .unwrap()
}

/// `git args…` in `dir`, which must succeed; its output.
async fn ok(dir: &Path, args: &[&str]) -> String {
    let out = git(dir, "tenant", args).await;
    assert!(
        out.status.success(),
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// What git said on standard error.
fn said(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The path of the file of the akt `number` in the clone at `dir`.
fn file_of(dir: &Path, number: &str) -> PathBuf {
    let listed = std::process::Command::new("git")
        .args(["ls-files"])
        .current_dir(dir)
        .output()
        .unwrap();
    let listed = String::from_utf8_lossy(&listed.stdout);
    let path = listed
        .lines()
        .find(|p| {
            Path::new(p)
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{number}-")))
        })
        .unwrap_or_else(|| panic!("no file of {number} in {listed}"));
    dir.join(path)
}

/// Replaces `from` with `to` in a file.
fn change(path: &Path, from: &str, to: &str) {
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains(from), "{text}");
    std::fs::write(path, text.replacen(from, to, 1)).unwrap();
}

/// The commit the history `git-remote-kuria` builds of `number` ends at, as
/// a clone through the helper has it: the helper's own code, over HTTP,
/// signed in by DPoP as the CLI is (`git.identity`).
async fn helper_main(world: &World, number: &str, name: &str) -> String {
    let session = kuria_cli::session::Session::started(
        kuria_cli::http::Http::new(kuria_cli::dpop::DpopKey::generate()),
        world.url.clone(),
        "test".to_owned(),
        &kuria_cli::konklav::Tokens {
            access_token: "tenant-dpop".to_owned(),
            refresh_token: None,
            expires_at: i64::MAX,
        },
    );
    let clone = world.path(name);
    ok(
        world.dir.path(),
        &["init", "--quiet", &clone.to_string_lossy()],
    )
    .await;
    let remote = kuria_cli::remote::Remote {
        session,
        number: number.to_owned(),
        name: "origin".to_owned(),
        git_dir: clone.join(".git"),
    };
    let history = remote.history().await.unwrap();
    let stream = kuria_cli::remote::fast_import(&history, "refs/heads/main");
    let mut import = tokio::process::Command::new("git")
        .args(["fast-import", "--quiet"])
        .current_dir(&clone)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use tokio::io::AsyncWriteExt as _;
        let mut stdin = import.stdin.take().unwrap();
        stdin.write_all(&stream).await.unwrap();
    }
    assert!(import.wait().await.unwrap().success());
    ok(&clone, &["rev-parse", "refs/heads/main"]).await
}

// r[verify git.repo.serve]
// r[verify git.repo.seed]
// r[verify git.repo.storage]
#[tokio::test(flavor = "multi_thread")]
async fn a_repository_starts_from_the_helper_s_history() {
    let world = World::new().await;
    let ix = world.documents().await;
    let cloned = world.clone(&ix, "a", "tenant").await;
    assert!(
        cloned.status.success(),
        "{}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    let a = world.path("a");
    let main = ok(&a, &["rev-parse", "HEAD"]).await;
    // Every commit the helper builds, by id.
    assert_eq!(main, helper_main(&world, &ix, "helper").await);
    let commits: usize = ok(&a, &["rev-list", "--count", "HEAD"])
        .await
        .parse()
        .unwrap();
    assert!(commits > 1, "{commits}");
    assert!(file_of(&a, "DOC-1").exists() && file_of(&a, &ix).exists());
    // A second clone, from the repository as kept.
    let again = world.clone(&ix, "b", "tenant").await;
    assert!(again.status.success());
    assert_eq!(ok(&world.path("b"), &["rev-parse", "HEAD"]).await, main);
}

// r[verify git.repo.serve]
#[tokio::test(flavor = "multi_thread")]
async fn serves_only_repositories_the_caller_may_use() {
    let world = World::new().await;
    let ix = world.documents().await;
    let folder = world
        .create("folder", &[], json!({ "title": "Plain" }))
        .await;
    // Not a repository, and none for a caller without credentials.
    assert!(!world.clone(&folder, "f", "tenant").await.status.success());
    assert!(!world.clone(&ix, "n", "invalid").await.status.success());
    // A reader fetches, and can't push.
    let read = world.clone(&ix, "r", READER).await;
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let r = world.path("r");
    change(&file_of(&r, "DOC-2"), "Use it.", "Use it well.");
    ok(&r, &["commit", "--quiet", "-am", "Better"]).await;
    let pushed = git(&r, READER, &["push", "origin", "main"]).await;
    assert!(said(&pushed).contains("403"), "{}", said(&pushed));
    assert!(!pushed.status.success());
    assert_eq!(world.revision("DOC-2").await, 1);
}

// r[verify git.repo.push]
// r[verify git.push]
// r[verify git.identity]
#[tokio::test(flavor = "multi_thread")]
async fn a_push_to_main_changes_kuria_and_is_fetched_as_pushed() {
    let world = World::new().await;
    let ix = world.documents().await;
    assert!(world.clone(&ix, "a", "tenant").await.status.success());
    assert!(world.clone(&ix, "b", "tenant").await.status.success());
    let a = world.path("a");
    change(&file_of(&a, "DOC-2"), "Use it.", "Use it well.");
    ok(&a, &["commit", "--quiet", "-am", "Say how"]).await;
    change(
        &file_of(&a, "DOC-1"),
        "Hello, reader.",
        "Hello, dear reader.",
    );
    change(&file_of(&a, "DOC-2"), "Use it well.", "Use it.");
    ok(&a, &["commit", "--quiet", "-am", "Greet and take back"]).await;
    ok(&a, &["push", "--quiet", "origin", "main"]).await;
    let pushed = ok(&a, &["rev-parse", "HEAD"]).await;

    // Each commit a batch, with its message as the reason.
    assert_eq!(world.revision("DOC-2").await, 3);
    assert_eq!(world.body("DOC-2").await.trim_end(), "Use it.");
    assert!(world.body("DOC-1").await.contains("Hello, dear reader."));
    let reasons = world
        .ok(
            "query { akt(number: \"DOC-2\") { revisions { reason } } }",
            json!({}),
        )
        .await;
    assert_eq!(reasons["akt"]["revisions"][1]["reason"], "Say how");

    // Fetched as pushed, and built again by the helper as pushed.
    let b = world.path("b");
    ok(&b, &["pull", "--quiet", "--ff-only"]).await;
    assert_eq!(ok(&b, &["rev-parse", "HEAD"]).await, pushed);
    assert_eq!(helper_main(&world, &ix, "helper").await, pushed);
}

// r[verify git.repo.push]
// r[verify git.push.normal]
// r[verify git.push.fast-forward]
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_push_keeps_nothing() {
    let world = World::new().await;
    let ix = world.documents().await;
    assert!(world.clone(&ix, "a", "tenant").await.status.success());
    let a = world.path("a");
    let main = ok(&a, &["rev-parse", "HEAD"]).await;

    // A file Kuria wouldn't write, after a commit it would: neither kept.
    change(&file_of(&a, "DOC-2"), "Use it.", "Use it well.");
    ok(&a, &["commit", "--quiet", "-am", "Fine"]).await;
    std::fs::write(a.join("notes.md"), "Mine.\n").unwrap();
    ok(&a, &["add", "notes.md"]).await;
    ok(&a, &["commit", "--quiet", "-m", "Not an akt"]).await;
    let refused = git(&a, "tenant", &["push", "origin", "main"]).await;
    assert!(!refused.status.success());
    assert!(
        said(&refused).contains("An akt file starts with"),
        "{}",
        said(&refused)
    );
    assert_eq!(world.revision("DOC-2").await, 1);
    let remote = ok(&a, &["ls-remote", "origin", "refs/heads/main"]).await;
    assert!(remote.starts_with(&main), "{remote}");

    // Kuria changed since: not pushed over.
    ok(&a, &["reset", "--quiet", "--hard", &main]).await;
    world
        .edit("DOC-2", json!({ "body": "Use it daily.\n" }))
        .await;
    change(&file_of(&a, "DOC-1"), "Hello, reader.", "Hi.");
    ok(&a, &["commit", "--quiet", "-am", "Shorter"]).await;
    let refused = git(&a, "tenant", &["push", "origin", "main"]).await;
    assert!(!refused.status.success());
    assert!(
        said(&refused).contains("Kuria has changes"),
        "{}",
        said(&refused)
    );
    assert_eq!(world.revision("DOC-1").await, 2);
}

// r[verify git.repo.storage]
// r[verify git.repo.push]
#[tokio::test(flavor = "multi_thread")]
async fn of_two_racing_pushes_to_one_ref_one_is_refused() {
    let world = World::new().await;
    let ix = world.documents().await;
    for name in ["a", "b"] {
        assert!(world.clone(&ix, name, "tenant").await.status.success());
        let dir = world.path(name);
        ok(&dir, &["checkout", "--quiet", "-b", "race"]).await;
        std::fs::write(dir.join("by.txt"), name).unwrap();
        ok(&dir, &["add", "by.txt"]).await;
        ok(&dir, &["commit", "--quiet", "-m", name]).await;
    }
    let (a, b) = (world.path("a"), world.path("b"));
    let (first, second) = tokio::join!(
        git(&a, "tenant", &["push", "origin", "race"]),
        git(&b, "tenant", &["push", "origin", "race"]),
    );
    assert_ne!(first.status.success(), second.status.success());
    let loser = if first.status.success() {
        &second
    } else {
        &first
    };
    assert!(said(loser).contains("stale"), "{}", said(loser));
    let winner = if first.status.success() { &a } else { &b };
    let kept = ok(&a, &["ls-remote", "origin", "refs/heads/race"]).await;
    assert!(kept.starts_with(&ok(winner, &["rev-parse", "HEAD"]).await));

    // A non-fast-forward of main, forced, is refused too.
    ok(&b, &["checkout", "--quiet", "main"]).await;
    ok(&b, &["commit", "--quiet", "--amend", "-m", "Rewritten"]).await;
    let forced = git(&b, "tenant", &["push", "--force", "origin", "main"]).await;
    assert!(
        said(&forced).contains("not a fast-forward"),
        "{}",
        said(&forced)
    );
    assert!(!forced.status.success());
}

// r[verify git.repo.branches]
#[tokio::test(flavor = "multi_thread")]
async fn branches_and_tags_are_kept_as_pushed_and_change_nothing() {
    let world = World::new().await;
    let ix = world.documents().await;
    assert!(world.clone(&ix, "a", "tenant").await.status.success());
    let a = world.path("a");
    ok(&a, &["checkout", "--quiet", "-b", "topic"]).await;
    change(&file_of(&a, "DOC-2"), "Use it.", "Anything at all");
    std::fs::write(a.join("scratch.txt"), "not an akt\n").unwrap();
    ok(&a, &["add", "-A"]).await;
    ok(&a, &["commit", "--quiet", "-m", "Try things"]).await;
    ok(&a, &["checkout", "--quiet", "main"]).await;
    ok(
        &a,
        &["merge", "--quiet", "--no-ff", "-m", "Merge topic", "topic"],
    )
    .await;
    let merge = ok(&a, &["rev-parse", "HEAD"]).await;
    ok(&a, &["branch", "--quiet", "-f", "merged", &merge]).await;
    ok(&a, &["reset", "--quiet", "--hard", "origin/main"]).await;
    ok(&a, &["tag", "plain"]).await;
    ok(&a, &["tag", "-a", "-m", "Annotated", "noted"]).await;
    ok(
        &a,
        &[
            "push", "--quiet", "origin", "topic", "merged", "plain", "noted",
        ],
    )
    .await;
    assert_eq!(world.revision("DOC-2").await, 1);
    assert_eq!(world.body("DOC-2").await.trim_end(), "Use it.");

    assert!(world.clone(&ix, "b", "tenant").await.status.success());
    let b = world.path("b");
    ok(&b, &["fetch", "--quiet", "--tags", "origin"]).await;
    assert_eq!(ok(&b, &["rev-parse", "origin/merged"]).await, merge);
    assert_eq!(
        ok(&b, &["rev-parse", "origin/topic"]).await,
        ok(&a, &["rev-parse", "topic"]).await
    );
    assert_eq!(ok(&b, &["cat-file", "-t", "noted"]).await, "tag");
    // A branch is deleted as it was pushed.
    ok(&a, &["push", "--quiet", "origin", "--delete", "topic"]).await;
    assert_eq!(
        ok(&a, &["ls-remote", "origin", "refs/heads/topic"]).await,
        ""
    );
}
