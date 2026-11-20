//! `kuria mcp` (`mcp.*`): an MCP client's requests over a pipe, against a
//! Kuria server on a local port whose documents have live drafts, while the
//! app edits one meanwhile; and the binary itself over standard input and
//! output, on an akt directory.

mod support;

use std::io::{BufReader, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};

use kuria_cli::dpop::DpopKey;
use kuria_cli::http::Http;
use kuria_cli::konklav::Tokens;
use kuria_cli::mcp::Kuria;
use kuria_cli::session::Session;
use rmcp::ServiceExt as _;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _};

use support::Server;

const BODY: &str = "# Setup {#setup}

## Install {#install}

Run the installer.

## Restart {#restart}

Restart the server.
";

/// An MCP client's side of a connection: requests out, answers in.
struct Client<W, R> {
    write: W,
    read: R,
    next: i64,
}

impl<W: tokio::io::AsyncWrite + Unpin, R: tokio::io::AsyncBufRead + Unpin> Client<W, R> {
    async fn send(&mut self, message: &Value) {
        self.write
            .write_all(format!("{message}\n").as_bytes())
            .await
            .unwrap();
        self.write.flush().await.unwrap();
    }

    async fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await;
        loop {
            let mut line = String::new();
            assert!(self.read.read_line(&mut line).await.unwrap() > 0, "closed");
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                assert!(message["error"].is_null(), "{message}");
                return message["result"].clone();
            }
        }
    }

    async fn start(&mut self) -> Value {
        let info = self
            .request(
                "initialize",
                json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                        "clientInfo": { "name": "test", "version": "1" } }),
            )
            .await;
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .await;
        info
    }

    /// A tool call's text, and whether it is a tool error.
    async fn call(&mut self, tool: &str, arguments: Value) -> (String, bool) {
        let result = self
            .request(
                "tools/call",
                json!({ "name": tool, "arguments": arguments }),
            )
            .await;
        (
            result["content"][0]["text"].as_str().unwrap().to_owned(),
            result["isError"] == true,
        )
    }

    async fn ok(&mut self, tool: &str, arguments: Value) -> String {
        let (text, error) = self.call(tool, arguments).await;
        assert!(!error, "{tool}: {text}");
        text
    }
}

/// A document type with live drafts.
fn document() -> Value {
    json!({
        "key": "document", "name": "Document", "number": "DOC-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "body", "name": "Body", "kind": "markdown" }
        ],
        "states": [{ "key": "draft", "name": "Draft", "initial": true }],
        "versions": {},
        "live_drafts": true
    })
}

async fn body(server: &Server, number: &str) -> String {
    let data = server
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
        .unwrap()
}

// r[verify mcp.tools]
// r[verify mcp.search]
// r[verify mcp.read]
// r[verify mcp.edit]
// r[verify mcp.errors]
// r[verify cli.write]
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_searches_reads_and_edits_while_the_app_types() {
    let server = Server::start().await;
    server
        .ok(
            "mutation($d: JSON!) { defineAktType(document: $d) { key } }",
            json!({ "d": document() }),
        )
        .await;
    let number = server
        .ok(
            "mutation($f: JSON) { createAkt(input: { type: \"document\", fields: $f }) { number } }",
            json!({ "f": { "title": "Setup", "body": BODY } }),
        )
        .await["createAkt"]["number"]
        .as_str()
        .unwrap()
        .to_owned();
    let session = Session::started(
        Http::new(DpopKey::generate()),
        server.session.url.clone(),
        "test".to_owned(),
        &Tokens {
            access_token: "tenant".to_owned(),
            refresh_token: None,
            expires_at: i64::MAX,
        },
    );
    tokio::task::LocalSet::new()
        .run_until(async {
            let (ours, theirs) = tokio::io::duplex(1 << 16);
            let (read, write) = tokio::io::split(ours);
            let served = tokio::task::spawn_local(async move {
                let running = Kuria::on(session).serve(theirs).await.unwrap();
                running.waiting().await.unwrap();
            });
            let mut client = Client {
                write,
                read: tokio::io::BufReader::new(read),
                next: 0,
            };

            let info = client.start().await;
            assert_eq!(info["serverInfo"]["name"], "kuria", "{info}");
            let tools = client.request("tools/list", json!({})).await;
            assert_eq!(tools["tools"].as_array().unwrap().len(), 5, "{tools}");

            let found = client
                .ok("search_documents", json!({ "text": "installer" }))
                .await;
            assert!(found.contains(&number) && found.contains("«installer»"), "{found}");

            let section = client
                .ok(
                    "read_document",
                    json!({ "number": number, "field": "body", "heading": "install" }),
                )
                .await;
            assert!(section.contains("has a live draft"), "{section}");
            assert!(section.contains("## Install {#install}\n\nRun the installer."), "{section}");
            assert!(!section.contains("Restart"), "{section}");

            // The app edits another section while the agent thinks.
            let typed = BODY.replace("Restart the server.", "Restart the server, twice.");
            server
                .ok(
                    "mutation($n: String!, $f: JSON!) { editAkt(input: { number: $n, revision: 1, fields: $f }) { revision } }",
                    json!({ "n": number, "f": { "body": typed } }),
                )
                .await;
            let edited = client
                .ok(
                    "edit_document",
                    json!({ "number": number, "reason": "Clearer",
                            "replacements": [{ "field": "body", "span": "Run the installer.", "text": "Run `setup`." }] }),
                )
                .await;
            assert!(edited.contains("Run `setup`."), "{edited}");
            assert_eq!(
                body(&server, &number).await,
                BODY.replace("Run the installer.", "Run `setup`.")
                    .replace("Restart the server.", "Restart the server, twice.")
            );

            // A span someone changed since is a tool error saying so.
            let (refused, error) = client
                .call(
                    "edit_document",
                    json!({ "number": number,
                            "replacements": [{ "field": "body", "span": "Restart the server.", "text": "Done." }] }),
                )
                .await;
            assert!(error, "{refused}");
            assert!(refused.contains("\"code\":\"NO_MATCH\"") && refused.contains("\"field\":\"body\""), "{refused}");
            let (missing, error) = client.call("read_document", json!({})).await;
            assert!(error && missing.contains("BAD_REQUEST"), "{missing}");
            drop(client);
            served.abort();
        })
        .await;
}

/// Sends `message` as a line to `kuria mcp` and reads lines until the
/// answer to the request `id`, if it is one.
fn exchange(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut impl std::io::BufRead,
    message: &Value,
) -> Value {
    writeln!(stdin, "{message}").unwrap();
    stdin.flush().unwrap();
    if message.get("id").is_none() {
        return Value::Null;
    }
    loop {
        let mut line = String::new();
        assert!(stdout.read_line(&mut line).unwrap() > 0, "kuria mcp closed");
        let answer: Value = serde_json::from_str(&line).unwrap();
        if answer["id"] == message["id"] {
            return answer;
        }
    }
}

/// An akt directory with the planning example's types and one plan, in a
/// temporary directory, answering it and its root.
fn planned(kuria: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("plans");
    let run = |args: &[&str]| {
        let ran = Command::new(kuria)
            .arg("--dir")
            .arg(&root)
            .args(args)
            .env("KURIA_ME", "me@kuria.example")
            .output()
            .unwrap();
        assert!(
            ran.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
    };
    run(&["init"]);
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/planning");
    for key in ["plan", "task", "gap", "decision"] {
        std::fs::copy(
            example.join(format!("{key}.json")),
            root.join(format!("types/{key}.json")),
        )
        .unwrap();
    }
    run(&[
        "new",
        "plan",
        "--set",
        "title=Local mode",
        "--set",
        "goal=Plans in the repository, read by agents.",
    ]);

    (temp, root)
}

// r[verify mcp.stdio]
// r[verify mcp.edit]
// r[verify cli.local.rules]
#[test]
fn the_binary_serves_an_akt_directory_over_standard_input_and_output() {
    let kuria = env!("CARGO_BIN_EXE_kuria");
    let (_temp, root) = planned(kuria);

    let mut child = Command::new(kuria)
        .arg("--dir")
        .arg(&root)
        .arg("mcp")
        .env("KURIA_ME", "me@kuria.example")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let info = exchange(
        &mut stdin,
        &mut stdout,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": { "name": "test", "version": "1" } } }),
    );
    assert_eq!(info["result"]["serverInfo"]["name"], "kuria", "{info}");
    exchange(
        &mut stdin,
        &mut stdout,
        &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    );
    let call = |id: i64, tool: &str, arguments: Value| {
        json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
                "params": { "name": tool, "arguments": arguments } })
    };
    let found = exchange(
        &mut stdin,
        &mut stdout,
        &call(
            2,
            "search_documents",
            json!({ "text": "repository agents" }),
        ),
    );
    let text = found["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("PLAN-001") && text.contains("«repository»"),
        "{found}"
    );
    let edited = exchange(
        &mut stdin,
        &mut stdout,
        &call(
            3,
            "edit_document",
            json!({ "number": "PLAN-001",
            "replacements": [{ "field": "goal", "span": "read by agents", "text": "read and edited by agents" }] }),
        ),
    );
    assert_ne!(edited["result"]["isError"], true, "{edited}");
    let read = exchange(
        &mut stdin,
        &mut stdout,
        &call(
            4,
            "read_document",
            json!({ "number": "PLAN-001", "field": "goal" }),
        ),
    );
    let text = read["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("has no live draft"), "{text}");
    assert!(
        text.contains("Plans in the repository, read and edited by agents."),
        "{text}"
    );
    drop(stdin);
    assert!(child.wait().unwrap().success());
}
