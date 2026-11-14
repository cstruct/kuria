//! Local mode's reads on temporary akt directories (`cli.local.*`), with
//! the planning example's akt types, and git where history is asked.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser as _;
use kuria_cli::args::Cli;
use kuria_cli::error::CliError;
use kuria_cli::output::Answer;
use serde_json::{Value, json};

const TYPES: [&str; 4] = ["plan", "task", "gap", "decision"];

/// An akt directory with the planning example's types, in a git
/// repository whose `user.email` is `me@kuria.example`.
struct Dir {
    _temp: tempfile::TempDir,
    root: PathBuf,
}

impl Dir {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("plans");
        let dir = Self { _temp: temp, root };
        dir.ok(&["init"]);
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/planning");
        for key in TYPES {
            std::fs::copy(
                example.join(format!("{key}.json")),
                dir.root.join(format!("types/{key}.json")),
            )
            .unwrap();
        }
        dir.git(&["init", "-q"]);
        dir.git(&["config", "user.email", "me@kuria.example"]);
        dir.git(&["config", "user.name", "Me"]);
        dir
    }

    fn git(&self, args: &[&str]) {
        let ran = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            ran.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    /// Writes the akt file of `number`: its front matter lines, then its
    /// sections.
    fn akt(&self, number: &str, front: &str, sections: &[(&str, &str)]) {
        let mut text = format!("---\nnumber: {number}\n{front}---\n");
        for (key, body) in sections {
            write!(text, "\n<!-- field: {key} -->\n{body}\n").unwrap();
        }
        std::fs::write(self.root.join(format!("akter/{number}.md")), text).unwrap();
    }

    fn run(&self, args: &[&str]) -> Result<Answer, CliError> {
        let root = self.root.to_str().unwrap();
        let mut all = vec!["kuria", "--dir", root];
        all.extend_from_slice(args);
        let cli = Cli::try_parse_from(all).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(kuria_cli::answer(cli))
    }

    fn ok(&self, args: &[&str]) -> Answer {
        self.run(args).unwrap_or_else(|e| panic!("{args:?}: {e}"))
    }

    fn refused(&self, args: &[&str]) -> CliError {
        match self.run(args) {
            Ok(answer) => panic!("{args:?} answered: {}", answer.markdown),
            Err(error) => error,
        }
    }

    /// A plan with two tasks, a gap and a decision, linked.
    fn planned(&self) {
        self.akt(
            "PLAN-001",
            "type: plan\nrevision: 3\nrank: \"1\"\nstate: active\ntitle: Local mode\nowner: me@kuria.example\n",
            &[("goal", "Plans in the repository.")],
        );
        self.akt(
            "PLAN-001-T01",
            "type: task\nrevision: 2\nrank: \"2\"\nstate: doing\nin: [PLAN-001]\nlinks: [Blocks PLAN-001-T02]\ntitle: Spec it\nassignee: me@kuria.example\n",
            &[("description", "Rules first.\n\n```markdown\n<!-- field: acceptance -->\n```")],
        );
        self.akt(
            "PLAN-001-T02",
            "type: task\nrevision: 1\nrank: \"3\"\nstate: todo\nin: [PLAN-001]\nlinks: [Blocked by PLAN-001-T01]\ntitle: Build it\nassignee: someone@else.example\n",
            &[],
        );
        self.akt(
            "PLAN-001-G01",
            "type: gap\nrevision: 1\nrank: \"4\"\nstate: open\nin: [PLAN-001]\ntitle: Where reasons go\n",
            &[],
        );
        self.akt(
            "PLAN-001-D01",
            "type: decision\nrevision: 2\nrank: \"5\"\nstate: decided\nin: [PLAN-001]\ntitle: Git is the history\n",
            &[("context", "C"), ("decision", "D"), ("why", "W")],
        );
    }
}

fn numbers(answer: &Answer) -> Vec<String> {
    answer.json["akter"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["number"].as_str().unwrap().to_owned())
        .collect()
}

// r[verify cli.local.dir]
#[test]
fn an_akt_directory_is_made_once_and_named_by_its_manifest() {
    let dir = Dir::new();
    for part in ["kuria.toml", "types", "akter"] {
        assert!(dir.root.join(part).exists(), "{part}");
    }
    assert_eq!(dir.refused(&["init"]).code, "ALREADY_AN_AKT_DIRECTORY");
    let elsewhere = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "kuria",
        "--dir",
        elsewhere.path().to_str().unwrap(),
        "types",
    ])
    .unwrap();
    let refused = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(kuria_cli::answer(cli))
        .unwrap_err();
    assert_eq!(
        (refused.code.as_str(), refused.kind.exit_code()),
        ("NOT_AN_AKT_DIRECTORY", 1)
    );
    let whoami = dir.ok(&["whoami", "--format", "json"]);
    assert_eq!(whoami.json["me"], "me@kuria.example");
}

// r[verify cli.local.layout]
// r[verify cli.local.types]
#[test]
fn reads_types_and_schemas_from_the_directory() {
    let dir = Dir::new();
    dir.planned();
    let types = dir.ok(&["types", "--format", "json"]).json;
    let task = types["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == "task")
        .unwrap()
        .clone();
    assert_eq!(
        task,
        json!({ "key": "task", "name": "Task", "version": 1, "numberPattern": "{parent}-T{seq:2}", "akterCount": 2 })
    );
    let schema = dir.ok(&["schema", "task", "--format", "json"]).json;
    assert_eq!(
        schema["type"]["fields"][3],
        json!({
            "key": "assignee", "name": "Assignee", "kind": "user", "required": false, "multiline": false,
            "description": null, "options": [], "format": null, "draft": null,
        })
    );
    assert_eq!(schema["type"]["presentation"]["icon"], "list_checks");
    assert_eq!(schema["type"]["links"][0]["key"], "blocks");
    let exported = dir.ok(&["types", "export", "gap", "--format", "json"]).json;
    assert_eq!(exported["types"][0]["key"], "gap");
    assert_eq!(dir.refused(&["schema", "nope"]).kind.exit_code(), 2);
}

// r[verify cli.local.layout]
// r[verify cli.local.people]
#[test]
fn lists_akter_filtered_ordered_and_a_page_at_a_time() {
    let dir = Dir::new();
    dir.planned();
    assert_eq!(
        numbers(&dir.ok(&["ls", "--in", "PLAN-001", "--format", "json"])),
        [
            "PLAN-001-T01",
            "PLAN-001-T02",
            "PLAN-001-G01",
            "PLAN-001-D01"
        ]
    );
    assert_eq!(
        numbers(&dir.ok(&["ls", "task", "--state", "To do", "--format", "json"])),
        ["PLAN-001-T02"]
    );
    assert_eq!(
        numbers(&dir.ok(&["ls", "task", "--where", "assignee=@me", "--format", "json"])),
        ["PLAN-001-T01"]
    );
    assert_eq!(
        numbers(&dir.ok(&["find", "build", "--format", "json"])),
        ["PLAN-001-T02"]
    );
    assert_eq!(
        numbers(&dir.ok(&[
            "ls",
            "--search",
            "PLAN-001-T",
            "--in",
            "PLAN-001",
            "--format",
            "json"
        ])),
        ["PLAN-001-T01", "PLAN-001-T02"]
    );
    let table = dir.ok(&["ls", "task", "--in", "PLAN-001"]).markdown;
    assert!(
        table.contains("| PLAN-001-T01 | Spec it | Doing | me@kuria.example |"),
        "{table}"
    );
    let first = dir
        .ok(&["ls", "--in", "PLAN-001", "--limit", "1", "--format", "json"])
        .json;
    assert_eq!(first["hasNextPage"], true);
    let cursor = first["endCursor"].as_str().unwrap().to_owned();
    assert_eq!(
        numbers(&dir.ok(&[
            "ls", "--in", "PLAN-001", "--limit", "1", "--after", &cursor, "--format", "json"
        ])),
        ["PLAN-001-T02"]
    );
    assert_eq!(
        dir.refused(&["ls", "--after", "nonsense"]).code,
        "BAD_REQUEST"
    );
    assert_eq!(
        dir.refused(&["ls", "task", "--where", "assignee"]).code,
        "BAD_REQUEST"
    );
    assert_eq!(dir.refused(&["ls", "--in", "PLAN-404"]).kind.exit_code(), 2);
}

// r[verify cli.local.layout]
// r[verify cli.file]
#[test]
fn cat_answers_the_file_as_the_server_writes_it() {
    let dir = Dir::new();
    dir.planned();
    let file = std::fs::read_to_string(dir.root.join("akter/PLAN-001-T01.md")).unwrap();
    // The same, without the order only the directory keeps.
    let expected = file.replace("rank: \"2\"\n", "");
    assert_eq!(
        dir.ok(&["cat", "PLAN-001-T01"]).markdown.trim_end(),
        expected.trim_end()
    );
    let json = dir.ok(&["cat", "PLAN-001-T01", "--format", "json"]).json;
    assert_eq!(
        json["akter"][0]["links"][0],
        json!({ "kind": "blocks", "name": "Blocks", "outgoing": true, "number": "PLAN-001-T02", "pinnedRevision": null })
    );
}

// r[verify cli.local.layout]
#[test]
fn shows_an_akt_with_its_board_links_and_transitions() {
    let dir = Dir::new();
    dir.planned();
    let plan = dir.ok(&["show", "PLAN-001", "--format", "json"]).json;
    let doing = plan["akt"]["members"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["key"] == "doing")
        .unwrap()
        .clone();
    assert_eq!(doing["akter"][0]["number"], "PLAN-001-T01");
    assert_eq!(
        plan["akt"]["members"]["akter"].as_array().unwrap().len(),
        2,
        "gap and decision, listed by type"
    );
    let shown = dir.ok(&["show", "PLAN-001"]).markdown;
    assert!(
        shown.contains("### Gap\n\n- PLAN-001-G01 · Where reasons go (Gap, Open)"),
        "{shown}"
    );
    let task = dir.ok(&["show", "PLAN-001-T02"]).markdown;
    assert!(
        task.contains("- Blocked by PLAN-001-T01 · Spec it (Task, Doing)"),
        "{task}"
    );
    assert!(task.contains("`start` Start → Doing"), "{task}");
}

// r[verify cli.local.history]
#[test]
fn history_is_what_git_says() {
    let dir = Dir::new();
    dir.planned();
    let untracked = dir
        .ok(&["history", "PLAN-001-T01", "--format", "json"])
        .json;
    assert_eq!(
        (
            untracked["revisions"].clone(),
            untracked["uncommitted"].clone()
        ),
        (json!([]), json!(true))
    );
    dir.commit("Plan local mode");
    let path = dir.root.join("akter/PLAN-001-T01.md");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("revision: 2", "revision: 3");
    std::fs::write(&path, text).unwrap();
    dir.commit("Start PLAN-001-T01");
    let history = dir
        .ok(&["history", "PLAN-001-T01", "--format", "json"])
        .json;
    let said: Vec<(String, String, String)> = history["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["by"].as_str().unwrap().to_owned(),
                r["what"].as_str().unwrap().to_owned(),
                r["reason"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        said,
        [
            ("Me".into(), "created it".into(), "Plan local mode".into()),
            (
                "Me".into(),
                "changed it".into(),
                "Start PLAN-001-T01".into()
            )
        ]
    );
    assert_eq!(history["uncommitted"], false);
    let shown = dir.ok(&["show", "PLAN-001-T01", "--history"]).markdown;
    assert!(
        shown.contains("Me changed it") && shown.contains("Start PLAN-001-T01"),
        "{shown}"
    );
}

// r[verify cli.local.unsupported]
#[test]
fn refuses_what_local_mode_doesn_t_do() {
    let dir = Dir::new();
    dir.planned();
    for args in [
        &["users"][..],
        &["contacts"],
        &["login"],
        &["logout"],
        &["draft", "PLAN-001", "goal"],
        &["export", "--whole-tenant", "-o", "x"],
    ] {
        let refused = dir.refused(args);
        assert_eq!(
            (refused.code.as_str(), refused.kind.exit_code()),
            ("LOCAL_UNSUPPORTED", 1),
            "{args:?}"
        );
    }
}

// r[verify cli.local.layout]
#[test]
fn refuses_a_file_that_doesn_t_fit_naming_it() {
    for (front, why) in [
        (
            "number: PLAN-001\ntype: plan\nrevision: 1\nrank: \"1\"\nstate: active\ntitle: X\n---\n\n<!-- akt: PLAN-001-T01 -->\n---\nnumber: PLAN-001-T01\ntype: task\nrevision: 1\nrank: \"2\"\nstate: todo\ntitle: Y\n",
            "a nested akt naming its number twice",
        ),
        (
            "number: PLAN-001\ntype: plan\nrevision: 1\nrank: \"1\"\nstate: shipped\ntitle: X\n",
            "no such state",
        ),
        (
            "number: PLAN-001\ntype: plan\nrevision: 1\nrank: \"1\"\nstate: active\ncolour: red\n",
            "no such field",
        ),
        (
            "number: PLAN-001\ntype: nope\nrevision: 1\nrank: \"1\"\nstate: active\n",
            "no such type",
        ),
        (
            "number: PLAN-001\ntype: plan\nrank: \"1\"\nstate: active\n",
            "no revision",
        ),
    ] {
        let dir = Dir::new();
        std::fs::write(
            dir.root.join("akter/PLAN-001.md"),
            format!("---\n{front}---\n"),
        )
        .unwrap();
        let refused = dir.refused(&["ls"]);
        assert_eq!(refused.code, "BAD_FILE", "{why}");
        assert!(
            refused.message.contains("PLAN-001.md"),
            "{why}: {}",
            refused.message
        );
    }
    let dir = Dir::new();
    dir.akt(
        "PLAN-001",
        "type: plan\nrevision: 1\nrank: \"1\"\nstate: active\nowner: \n",
        &[],
    );
    assert!(dir.run(&["ls"]).is_ok(), "an empty value is no value");
    let value: Value = dir.ok(&["cat", "PLAN-001", "--format", "json"]).json;
    assert_eq!(value["akter"][0]["fields"], json!({}));
}

/// The file holding `number`, and where its part of it is.
fn holding(dir: &Dir, number: &str) -> (PathBuf, String, std::ops::Range<usize>) {
    for entry in std::fs::read_dir(dir.root.join("akter")).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let marker = format!("<!-- akt: {number} -->\n");
        let start = if text.starts_with(&format!("---\nnumber: {number}\n")) {
            Some(0)
        } else {
            text.find(&marker).map(|i| i + marker.len())
        };
        if let Some(start) = start {
            let end = text[start..]
                .find("\n<!-- akt: ")
                .map_or(text.len(), |i| start + i + 1);
            return (path, text, start..end);
        }
    }
    panic!("no file holds {number}")
}

/// `number`'s part of the file that holds it.
fn file(dir: &Dir, number: &str) -> String {
    let (_, text, range) = holding(dir, number);
    text[range].to_owned()
}

/// Replaces `from` with `to` once in `number`'s part of its file.
fn edit_akt(dir: &Dir, number: &str, from: &str, to: &str) {
    let (path, mut text, range) = holding(dir, number);
    let part = &text[range.clone()];
    assert!(part.contains(from), "{number} has no `{from}`:\n{part}");
    let changed = part.replacen(from, to, 1);
    text.replace_range(range, &changed);
    std::fs::write(path, text).unwrap();
}

fn revision_of(dir: &Dir, number: &str) -> i64 {
    dir.ok(&["cat", number, "--format", "json"]).json["akter"][0]["revision"]
        .as_i64()
        .unwrap()
}

/// A plan, activated, with two tasks in it.
fn worked(dir: &Dir) {
    dir.ok(&["new", "plan", "--set", "title=Local writes"]);
    dir.ok(&["set", "PLAN-001", "goal=Writes in files"]);
    dir.ok(&["do", "PLAN-001", "activate"]);
    dir.ok(&[
        "new",
        "task",
        "--in",
        "PLAN-001",
        "--set",
        "title=First",
        "--set",
        "assignee=@me",
    ]);
    dir.ok(&["new", "task", "--in", "PLAN-001", "--set", "title=Second"]);
}

// r[verify cli.local.rules]
// r[verify cli.local.numbers]
// r[verify cli.local.revision]
// r[verify cli.local.reason]
#[test]
fn writes_akter_by_the_server_s_rules() {
    let dir = Dir::new();
    let created = dir
        .ok(&[
            "new",
            "plan",
            "--set",
            "title=Local writes",
            "--reason",
            "Planning",
            "--format",
            "json",
        ])
        .json;
    assert_eq!(
        created,
        json!({ "number": "PLAN-001", "revision": 1, "reason": "Planning" })
    );
    assert!(
        dir.ok(&["new", "plan", "--set", "title=Other", "--reason", "Why"])
            .markdown
            .ends_with("Commit with: Why")
    );
    // Activating needs the goal.
    let refused = dir.refused(&["do", "PLAN-001", "activate"]);
    assert_eq!(
        (refused.code.as_str(), refused.kind.exit_code()),
        ("REFUSED", 1)
    );
    dir.ok(&["set", "PLAN-001", "goal=Writes in files"]);
    assert_eq!(
        dir.ok(&["do", "PLAN-001", "Activate", "--format", "json"])
            .json["state"],
        "active"
    );
    let task = dir
        .ok(&[
            "new",
            "task",
            "--in",
            "PLAN-001",
            "--set",
            "title=First",
            "--set",
            "assignee=@me",
            "--format",
            "json",
        ])
        .json;
    assert_eq!(task["number"], "PLAN-001-T01");
    assert_eq!(
        dir.ok(&[
            "new",
            "task",
            "--in",
            "PLAN-002",
            "--set",
            "title=Elsewhere",
            "--format",
            "json"
        ])
        .json["number"],
        "PLAN-002-T01"
    );
    assert_eq!(
        dir.ok(&[
            "new",
            "task",
            "--in",
            "PLAN-001",
            "--set",
            "title=Second",
            "--format",
            "json"
        ])
        .json["number"],
        "PLAN-001-T02"
    );
    // Each task created in it is a change to the plan, as on a server.
    assert_eq!(revision_of(&dir, "PLAN-001"), 5);
    let written = file(&dir, "PLAN-001-T01");
    assert!(
        written.contains("in: [PLAN-001]") && written.contains("assignee: me@kuria.example"),
        "{written}"
    );
    let without_rank = written
        .lines()
        .filter(|l| !l.starts_with("rank:"))
        .collect::<Vec<_>>()
        .join("\n");
    // Nested in the plan's file, whose line gives its number.
    let catted =
        dir.ok(&["cat", "PLAN-001-T01"])
            .markdown
            .replacen("number: PLAN-001-T01\n", "", 1);
    assert_eq!(catted.trim_end(), without_rank.trim_end());
}

// r[verify cli.local.revision]
// r[verify cli.apply]
#[test]
fn applies_files_and_refuses_stale_ones() {
    let dir = Dir::new();
    worked(&dir);
    let out = tempfile::tempdir().unwrap();
    dir.ok(&["cat", "PLAN-001-T01", "-o", out.path().to_str().unwrap()]);
    let path = out.path().join("PLAN-001-T01.md");
    let p = path.to_str().unwrap();
    assert_eq!(
        dir.ok(&["apply", p, "--format", "json"]).json["applied"][0]["changed"],
        json!([])
    );
    let read = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!(
            "{}\n<!-- field: description -->\nWhat it takes.\n",
            read.replace("assignee: me@kuria.example\n", "")
        ),
    )
    .unwrap();
    let dry = dir.ok(&["apply", p, "--dry-run", "--format", "json"]).json;
    assert_eq!(dry["applied"][0]["dryRun"], true);
    assert!(
        !file(&dir, "PLAN-001-T01").contains("What it takes"),
        "a dry run writes nothing"
    );
    let applied = dir
        .ok(&["apply", p, "--reason", "Described", "--format", "json"])
        .json;
    assert_eq!(
        applied["applied"][0]["changed"],
        json!(["assignee", "description"])
    );
    assert_eq!(applied["reason"], "Described");
    let now = file(&dir, "PLAN-001-T01");
    assert!(
        now.contains("<!-- field: description -->\nWhat it takes.") && !now.contains("assignee:"),
        "{now}"
    );
    // The file is now stale.
    let stale = dir.refused(&["apply", p]);
    assert_eq!((stale.code.as_str(), stale.kind.exit_code()), ("STALE", 3));
    dir.ok(&["cat", "PLAN-001-T01", "-o", out.path().to_str().unwrap()]);
    let fresh = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, fresh.replace("state: todo", "state: done")).unwrap();
    assert_eq!(dir.refused(&["apply", p]).field.as_deref(), Some("state"));
    std::fs::write(&path, fresh.replace("---\n\n", "colour: red\n---\n\n")).unwrap();
    assert_eq!(dir.refused(&["apply", p]).code, "UNKNOWN_FIELD");
}

// r[verify cli.local.rules]
// r[verify cli.local.revision]
#[test]
fn arranges_members_order_and_links() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["new", "plan", "--set", "title=Later"]);
    // Linking writes both files.
    let linked = dir
        .ok(&[
            "link",
            "PLAN-001-T01",
            "Blocks",
            "PLAN-001-T02",
            "--pin",
            "--format",
            "json",
        ])
        .json;
    assert_eq!(linked["pinnedRevision"], 1);
    assert!(file(&dir, "PLAN-001-T01").contains("links: [Blocks PLAN-001-T02@1]"));
    assert!(file(&dir, "PLAN-001-T02").contains("links: [Blocked by PLAN-001-T01@1]"));
    let shown = dir.ok(&["show", "PLAN-001-T02", "--format", "json"]).json;
    assert_eq!(shown["akt"]["links"][0]["name"], "Blocked by");
    assert_eq!(
        dir.refused(&["link", "PLAN-001-T01", "blocks", "PLAN-001-T02"])
            .code,
        "INVALID_LINK"
    );
    assert_eq!(
        dir.refused(&["link", "PLAN-001-T01", "blocks", "PLAN-001-T01"])
            .code,
        "INVALID_LINK"
    );
    dir.ok(&["unlink", "PLAN-001-T01", "blocks", "PLAN-001-T02"]);
    assert!(!file(&dir, "PLAN-001-T02").contains("links:"));
    assert_eq!(
        dir.refused(&["unlink", "PLAN-001-T01", "blocks", "PLAN-001-T02"])
            .kind
            .exit_code(),
        2
    );
    // Order.
    dir.ok(&["move", "PLAN-001-T02", "--before", "PLAN-001-T01"]);
    assert_eq!(
        numbers(&dir.ok(&["ls", "task", "--in", "PLAN-001", "--format", "json"])),
        ["PLAN-001-T02", "PLAN-001-T01"]
    );
    assert_eq!(
        dir.refused(&["move", "PLAN-001-T02", "--after", "PLAN-001-T02"])
            .code,
        "BAD_REQUEST"
    );
    // Membership: a transfer is one change of all three.
    let before = revision_of(&dir, "PLAN-002");
    dir.ok(&["transfer", "PLAN-001-T02", "PLAN-001", "PLAN-002"]);
    assert_eq!(
        dir.ok(&["show", "PLAN-001-T02", "--format", "json"]).json["akt"]["in"],
        json!(["PLAN-002"])
    );
    assert_eq!(revision_of(&dir, "PLAN-002"), before + 1);
    assert_eq!(
        dir.refused(&["add", "PLAN-002", "PLAN-001-T02"]).code,
        "ALREADY_MEMBER"
    );
    assert_eq!(
        dir.refused(&["add", "PLAN-001", "PLAN-002"]).code,
        "NOT_CONTAINABLE"
    );
    assert_eq!(
        dir.refused(&["remove", "PLAN-001", "PLAN-001-T02"]).code,
        "NOT_MEMBER"
    );
    dir.ok(&["remove", "PLAN-002", "PLAN-001-T02"]);
    dir.ok(&["add", "PLAN-001", "PLAN-001-T02"]);
    assert_eq!(
        dir.ok(&["show", "PLAN-001-T02", "--format", "json"]).json["akt"]["in"],
        json!(["PLAN-001"])
    );
}

// r[verify cli.local.types]
#[test]
fn redefines_types_only_where_their_akter_fit() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["do", "PLAN-001-T01", "start"]);
    let example =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/planning/task.json");
    let mut task: Value = serde_json::from_str(&std::fs::read_to_string(example).unwrap()).unwrap();
    task["name"] = json!("Job");
    let renamed = dir.root.join("../job.json");
    std::fs::write(&renamed, task.to_string()).unwrap();
    assert_eq!(
        dir.ok(&[
            "types",
            "define",
            renamed.to_str().unwrap(),
            "--format",
            "json"
        ])
        .json["defined"][0]["key"],
        "task"
    );
    assert_eq!(
        dir.ok(&["schema", "task", "--format", "json"]).json["type"]["name"],
        "Job"
    );
    // Without `assignee`, PLAN-001-T01, which has one, wouldn't fit.
    let fields: Vec<Value> = task["fields"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["key"] != "assignee")
        .cloned()
        .collect();
    task["fields"] = json!(fields);
    task["presentation"]["card"] = json!([]);
    task["presentation"]["list"] = json!([]);
    std::fs::write(&renamed, task.to_string()).unwrap();
    let refused = dir.refused(&["types", "define", renamed.to_str().unwrap()]);
    assert!(
        refused.message.contains("PLAN-001-T01"),
        "{}",
        refused.message
    );
}

// r[verify cli.local.lock]
#[test]
fn writes_at_once_lose_nothing() {
    let dir = Dir::new();
    dir.ok(&["new", "plan", "--set", "title=Busy"]);
    let root = dir.root.to_str().unwrap().to_owned();
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let root = root.clone();
            std::thread::spawn(move || {
                let title = format!("title=Task {i}");
                let cli = Cli::try_parse_from([
                    "kuria", "--dir", &root, "new", "task", "--in", "PLAN-001", "--set", &title,
                ])
                .unwrap();
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(kuria_cli::answer(cli))
                    .map(|a| a.json["number"].as_str().unwrap().to_owned())
            })
        })
        .collect();
    let mut created: Vec<String> = threads
        .into_iter()
        .map(|t| t.join().unwrap().unwrap())
        .collect();
    created.sort();
    let expected: Vec<String> = (1..=8).map(|n| format!("PLAN-001-T{n:02}")).collect();
    assert_eq!(created, expected);
    assert_eq!(
        revision_of(&dir, "PLAN-001"),
        9,
        "every member's creation counted"
    );
    assert!(!dir.root.join(".kuria.lock").exists());
}

// r[verify cli.local.rules]
#[test]
fn refuses_writes_as_the_server_does() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["new", "plan", "--set", "title=Other"]);
    // A task must be created in a plan, and refused as the server would.
    assert_eq!(
        dir.refused(&["new", "task", "--set", "title=Loose"]).code,
        "CREATED_IN"
    );
    assert_eq!(
        dir.refused(&["new", "task", "--in", "PLAN-001"]).code,
        "REFUSED"
    );
    assert_eq!(
        dir.refused(&["do", "PLAN-001-T01", "nope"]).code,
        "UNKNOWN_TRANSITION"
    );
    assert_eq!(
        dir.refused(&["set", "PLAN-001-T01", "colour=red"]).code,
        "UNKNOWN_FIELD"
    );
    let ls = dir.ok(&["ls", "task", "--where", "assignee=@me", "--format", "json"]);
    assert_eq!(numbers(&ls), ["PLAN-001-T01"]);
    // A finished plan takes no new members.
    dir.ok(&["do", "PLAN-002", "drop"]);
    assert_eq!(
        dir.refused(&["new", "task", "--in", "PLAN-002", "--set", "title=Late"])
            .code,
        "FROZEN"
    );
}

// r[verify cli.local.check]
#[test]
fn check_passes_a_consistent_directory() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["link", "PLAN-001-T01", "blocks", "PLAN-001-T02", "--pin"]);
    let checked = dir.ok(&["check", "--format", "json"]).json;
    assert_eq!(
        checked,
        json!({ "consistent": true, "types": 4, "akter": 3, "problems": [] })
    );
}

// r[verify cli.local.check]
#[test]
fn check_names_every_problem_with_its_file() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["new", "gap", "--in", "PLAN-001", "--set", "title=Open"]);
    dir.ok(&["link", "PLAN-001-T01", "blocks", "PLAN-001-T02"]);
    let edit = |number: &str, from: &str, to: &str| edit_akt(&dir, number, from, to);
    // A link only one side lists, and a kind no type declares.
    edit("PLAN-001-T02", "links: [Blocked by PLAN-001-T01]\n", "");
    edit(
        "PLAN-001-T01",
        "links: [Blocks PLAN-001-T02]",
        "links: [Blocks PLAN-001-T02, Relates PLAN-001-T02]",
    );
    // A container the directory lacks, and one that can't contain it.
    edit(
        "PLAN-001-T02",
        "in: [PLAN-001]",
        "in: [PLAN-404, PLAN-001-G01]",
    );
    // Two akter in one place in the order.
    let rank = file(&dir, "PLAN-001")
        .lines()
        .find(|l| l.starts_with("rank:"))
        .unwrap()
        .to_owned();
    let gap_rank = file(&dir, "PLAN-001-G01")
        .lines()
        .find(|l| l.starts_with("rank:"))
        .unwrap()
        .to_owned();
    edit("PLAN-001-G01", &gap_rank, &rank);
    // A number its type's pattern doesn't give.
    std::fs::write(
        dir.root.join("akter/TASK-9.md"),
        "---\nnumber: TASK-9\ntype: task\nrevision: 1\nrank: \"9\"\nstate: todo\ntitle: Loose\n---\n",
    )
    .unwrap();
    // A merge left unresolved, and a state the type lacks.
    std::fs::write(
        dir.root.join("akter/PLAN-002.md"),
        "<<<<<<< HEAD\n---\nnumber: PLAN-002\n=======\n>>>>>>> other\n",
    )
    .unwrap();
    std::fs::write(
        dir.root.join("akter/PLAN-003.md"),
        "---\nnumber: PLAN-003\ntype: plan\nrevision: 1\nrank: \"8\"\nstate: shipped\ntitle: X\n---\n",
    )
    .unwrap();

    let refused = dir.refused(&["check"]);
    assert_eq!(
        (refused.code.as_str(), refused.kind.exit_code()),
        ("INCONSISTENT", 1)
    );
    // The tasks and the gap are nested in the plan's file.
    let plan = "akter/PLAN-001-local-writes.md";
    for (file, says) in [
        (
            plan,
            "PLAN-001-T01: lists `Blocks PLAN-001-T02`, but PLAN-001-T02 doesn't list `Blocked by PLAN-001-T01`",
        ),
        (
            plan,
            "PLAN-001-T01: the link `Relates PLAN-001-T02` is of no kind",
        ),
        (
            plan,
            "PLAN-001-T02: is in `PLAN-404`, which the directory lacks",
        ),
        (plan, "a `gap` can't contain a `task`"),
        (plan, "PLAN-001-G01: has the rank of PLAN-001"),
        ("akter/TASK-9.md", "isn't a number `task` gives"),
        ("akter/PLAN-002.md", "starts with `---`"),
        ("akter/PLAN-003.md", "no state `shipped`"),
    ] {
        assert!(
            refused
                .message
                .lines()
                .any(|l| l.starts_with(&format!("- {file}: ")) && l.contains(says)),
            "no `{says}` for {file} in:\n{}",
            refused.message
        );
    }
    assert_eq!(
        dir.refused(&["check", "--format", "json"]).code,
        "INCONSISTENT"
    );
}

// r[verify cli.local.convert]
#[test]
fn a_directory_goes_to_a_bundle_and_back_unchanged() {
    let dir = Dir::new();
    dir.planned();
    dir.ok(&[
        "link",
        "PLAN-001-G01",
        "resolved_by",
        "PLAN-001-T02",
        "--pin",
    ]);
    dir.ok(&["set", "PLAN-001-D01", "decided_by=Dev@Shop.example"]);
    // Written by hand in files of their own, then moved as the layout says.
    dir.ok(&["check", "--fix"]);
    dir.commit("Planned");
    let bundle =
        kuria_cli::local::convert::bundle_of(&dir.root, Some("me@kuria.example".to_owned()))
            .unwrap();

    // As the server's import checks it: revisions from 1 to the file's,
    // each hash its state's and values'.
    let types: std::collections::HashMap<String, kuria_config::AktType> = bundle["aktTypes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            let v = &t["versions"][0];
            let doc = serde_json::from_value(v["definition"].clone()).unwrap();
            (
                v["id"].as_str().unwrap().to_owned(),
                kuria_config::validate(doc).unwrap(),
            )
        })
        .collect();
    for akt in bundle["akter"].as_array().unwrap() {
        let ty = &types[akt["typeVersion"].as_str().unwrap()];
        let fields =
            kuria_engine::fields_from_json(ty, akt["fields"].as_object().unwrap()).unwrap();
        let hash: String = kuria_engine::state_hash(akt["state"].as_str().unwrap(), &fields)
            .iter()
            .fold(String::new(), |mut s, b| {
                write!(s, "{b:02x}").unwrap();
                s
            });
        let revisions = akt["revisions"].as_array().unwrap();
        let file = revision_of(&dir, akt["number"].as_str().unwrap());
        assert_eq!(revisions.len(), usize::try_from(file).unwrap());
        for (i, r) in revisions.iter().enumerate() {
            assert_eq!(r["revision"], i + 1);
            assert_eq!(r["hash"], hash.as_str());
        }
    }
    assert_eq!(
        bundle["links"].as_array().unwrap().len(),
        2,
        "each link once"
    );
    assert!(
        bundle["people"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["email"] == "Dev@Shop.example")
    );

    // And back: every akt as it was, nested in its plan's file.
    let again = dir.root.parent().unwrap().join("again");
    kuria_cli::local::convert::to_dir(&bundle, &again).unwrap();
    let back = Dir {
        _temp: tempfile::tempdir().unwrap(),
        root: again.clone(),
    };
    for number in numbers(&dir.ok(&["ls", "--format", "json"])) {
        assert_eq!(
            back.ok(&["cat", &number]).markdown,
            dir.ok(&["cat", &number]).markdown,
            "{number}"
        );
    }
    let files: Vec<_> = std::fs::read_dir(again.join("akter"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(files, ["PLAN-001-local-mode.md"]);
    for key in TYPES {
        let doc = |root: &Path| {
            serde_json::from_str::<Value>(
                &std::fs::read_to_string(root.join(format!("types/{key}.json"))).unwrap(),
            )
            .unwrap()
        };
        assert_eq!(doc(&again), doc(&dir.root), "{key}");
    }
    let refused = kuria_cli::local::convert::bundle_of(&again, None);
    assert!(refused.is_ok(), "a written directory converts again");
}

// r[verify cli.local.layout]
// r[verify cli.local.nesting]
#[test]
fn reads_nested_akter_from_their_parent_s_file() {
    let dir = Dir::new();
    let text = r#"---
number: PLAN-001
type: plan
revision: 2
rank: "1"
state: active
title: Local mode
---

<!-- field: goal -->
Plans in the repository. An example:

```markdown
<!-- akt: PLAN-001-T09 -->
```

<!-- akt: PLAN-001-T01 -->
---
type: task
revision: 1
rank: "2"
state: doing
in: [PLAN-001]
links: [Blocks PLAN-001-T02]
title: Spec it
---

<!-- field: description -->
Rules first.

<!-- akt: PLAN-001-T02 -->
---
type: task
revision: 1
rank: "3"
state: todo
in: [PLAN-001]
links: [Blocked by PLAN-001-T01]
title: Build it
---
"#;
    std::fs::write(dir.root.join("akter/PLAN-001-local-mode.md"), text).unwrap();

    let mut listed = numbers(&dir.ok(&["ls", "task", "--format", "json"]));
    listed.sort();
    assert_eq!(listed, ["PLAN-001-T01", "PLAN-001-T02"]);
    let plan = dir.ok(&["cat", "PLAN-001"]).markdown;
    assert!(
        plan.contains("<!-- akt: PLAN-001-T09 -->"),
        "a line in a fence is text"
    );
    let task = dir.ok(&["cat", "PLAN-001-T01"]).markdown;
    assert!(task.starts_with("---\nnumber: PLAN-001-T01\n"), "{task}");
    assert!(task.contains("Rules first."), "{task}");
    assert!(!task.contains("Build it"), "only its own text: {task}");
    let shown = dir.ok(&["show", "PLAN-001"]).markdown;
    assert!(shown.contains("PLAN-001-T02 · Build it"), "{shown}");
    // A write to a nested akt rewrites its file, the rest as it was.
    let path = dir.root.join("akter/PLAN-001-local-mode.md");
    dir.ok(&["set", "PLAN-001-T01", "title=Spec it well"]);
    let changed = text
        .replacen("revision: 1\nrank: \"2\"", "revision: 2\nrank: \"2\"", 1)
        .replace("title: Spec it\n", "title: Spec it well\n");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), changed);
    let text = changed;

    // A problem in a nested akt names its file and its number.
    std::fs::write(
        dir.root.join("akter/PLAN-001-local-mode.md"),
        text.replace("state: todo", "state: shipped"),
    )
    .unwrap();
    let refused = dir.refused(&["ls"]);
    assert!(
        refused
            .message
            .contains("PLAN-001-local-mode.md: PLAN-001-T02:"),
        "{}",
        refused.message
    );
}

/// The names of the files in `akter/`, sorted.
fn akt_files(dir: &Dir) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(dir.root.join("akter"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

// r[verify cli.local.nesting]
// r[verify cli.local.names]
#[test]
fn writes_akter_into_their_parent_s_file_named_by_title() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["new", "gap", "--in", "PLAN-001", "--set", "title=Open"]);
    // One file for the plan and everything numbered after it.
    assert_eq!(akt_files(&dir), ["PLAN-001-local-writes.md"]);
    let text = std::fs::read_to_string(dir.root.join("akter/PLAN-001-local-writes.md")).unwrap();
    let at = |marker: &str| text.find(marker).unwrap_or_else(|| panic!("no {marker}"));
    assert!(text.starts_with("---\nnumber: PLAN-001\n"));
    assert!(at("<!-- akt: PLAN-001-T01 -->") < at("<!-- akt: PLAN-001-T02 -->"));
    assert!(at("<!-- akt: PLAN-001-T02 -->") < at("<!-- akt: PLAN-001-G01 -->"));

    // Its order is the directory's.
    dir.ok(&["move", "PLAN-001-G01", "--before", "PLAN-001-T01"]);
    let text = std::fs::read_to_string(dir.root.join("akter/PLAN-001-local-writes.md")).unwrap();
    assert!(text.find("PLAN-001-G01 -->") < text.find("PLAN-001-T01 -->"));

    // A new title renames the file, with its nested akter.
    dir.ok(&["set", "PLAN-001", "title=Writes, nested"]);
    assert_eq!(akt_files(&dir), ["PLAN-001-writes-nested.md"]);
    assert_eq!(
        numbers(&dir.ok(&["ls", "task", "--in", "PLAN-001", "--format", "json"])).len(),
        2
    );
    // A nested akt's title names no file.
    dir.ok(&["set", "PLAN-001-T01", "title=Renamed task"]);
    assert_eq!(akt_files(&dir), ["PLAN-001-writes-nested.md"]);
    // Removed from its plan, a task stays in its file, in nothing.
    dir.ok(&["remove", "PLAN-001", "PLAN-001-T02"]);
    assert_eq!(akt_files(&dir), ["PLAN-001-writes-nested.md"]);
    assert!(!file(&dir, "PLAN-001-T02").contains("in:"));
    dir.ok(&["check"]);
}

// r[verify cli.local.nesting]
#[test]
fn a_write_moves_an_akt_of_its_own_file_into_its_parent_s() {
    let dir = Dir::new();
    dir.planned();
    assert_eq!(akt_files(&dir).len(), 5);
    dir.ok(&["set", "PLAN-001-T02", "title=Build it now"]);
    assert_eq!(
        akt_files(&dir),
        [
            "PLAN-001-D01.md",
            "PLAN-001-G01.md",
            "PLAN-001-T01.md",
            "PLAN-001.md"
        ]
    );
    let plan = std::fs::read_to_string(dir.root.join("akter/PLAN-001.md")).unwrap();
    assert!(
        plan.contains("<!-- akt: PLAN-001-T02 -->\n---\ntype: task\n"),
        "{plan}"
    );
    assert!(
        dir.ok(&["cat", "PLAN-001-T02"])
            .markdown
            .contains("title: Build it now")
    );
}

// r[verify cli.local.history]
#[test]
fn history_follows_each_akt_whatever_file_holds_it() {
    let dir = Dir::new();
    dir.planned();
    dir.commit("Plan local mode");
    // T02 moves into the plan's file; T01 is left as it was.
    dir.ok(&["set", "PLAN-001-T02", "title=Build it now"]);
    dir.commit("Retitle the build");
    dir.ok(&["set", "PLAN-001-G01", "title=Where reasons go, really"]);
    let said = |number: &str| {
        let history = dir.ok(&["history", number, "--format", "json"]).json;
        let revisions: Vec<(i64, String)> = history["revisions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["revision"].as_i64().unwrap(),
                    r["reason"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        (revisions, history["uncommitted"].as_bool().unwrap())
    };
    assert_eq!(
        said("PLAN-001-T02"),
        (
            vec![
                (1, "Plan local mode".to_owned()),
                (2, "Retitle the build".to_owned())
            ],
            false
        )
    );
    assert_eq!(
        said("PLAN-001-T01"),
        (vec![(2, "Plan local mode".to_owned())], false)
    );
    // The plan's file changed, but not the plan.
    assert_eq!(
        said("PLAN-001"),
        (vec![(3, "Plan local mode".to_owned())], false)
    );
    assert!(said("PLAN-001-G01").1, "changed, not committed");
}

// r[verify cli.local.check]
// r[verify cli.local.nesting]
// r[verify cli.local.names]
#[test]
fn check_names_akter_out_of_place_and_files_misnamed_and_fixes_them() {
    let dir = Dir::new();
    dir.planned();
    let before: Vec<String> = [
        "PLAN-001",
        "PLAN-001-T01",
        "PLAN-001-T02",
        "PLAN-001-G01",
        "PLAN-001-D01",
    ]
    .iter()
    .map(|n| dir.ok(&["cat", n]).markdown)
    .collect();
    let refused = dir.refused(&["check"]);
    for says in [
        "- akter/PLAN-001-T01.md: is numbered after PLAN-001, so it belongs in akter/PLAN-001.md, after it",
        "- akter/PLAN-001.md: is named for PLAN-001, its first akt, as `PLAN-001-local-mode.md`",
    ] {
        assert!(
            refused.message.contains(says),
            "no `{says}` in:\n{}",
            refused.message
        );
    }

    // Fixed: one file, named by its plan, every akt as it was.
    let fixed = dir.ok(&["check", "--fix", "--format", "json"]).json;
    assert_eq!(fixed["consistent"], true);
    assert_eq!(fixed["files"], json!({ "before": 5, "after": 1 }));
    assert_eq!(akt_files(&dir), ["PLAN-001-local-mode.md"]);
    let after: Vec<String> = [
        "PLAN-001",
        "PLAN-001-T01",
        "PLAN-001-T02",
        "PLAN-001-G01",
        "PLAN-001-D01",
    ]
    .iter()
    .map(|n| dir.ok(&["cat", n]).markdown)
    .collect();
    assert_eq!(after, before);

    // An akt stored twice, one nested where it isn't numbered after
    // anything, and a file left named by an old title.
    let path = dir.root.join("akter/PLAN-001-local-mode.md");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        dir.root.join("akter/PLAN-001-T01.md"),
        file(&dir, "PLAN-001-T01").replace("---\ntype:", "---\nnumber: PLAN-001-T01\ntype:"),
    )
    .unwrap();
    std::fs::write(
        &path,
        text.replace("title: Local mode", "title: Local mode, renamed"),
    )
    .unwrap();
    std::fs::write(
        dir.root.join("akter/PLAN-002.md"),
        "---\nnumber: PLAN-002\ntype: plan\nrevision: 1\nrank: \"9\"\nstate: draft\ntitle: Other\n---\n\n<!-- akt: PLAN-003 -->\n---\ntype: plan\nrevision: 1\nrank: \"10\"\nstate: draft\ntitle: Third\n---\n",
    )
    .unwrap();
    let refused = dir.refused(&["check"]);
    for says in [
        "- akter/PLAN-001-local-mode.md: PLAN-001-T01: is stored twice: also in akter/PLAN-001-T01.md",
        "- akter/PLAN-001-local-mode.md: is named for PLAN-001, its first akt, as `PLAN-001-local-mode-renamed.md`",
        "- akter/PLAN-002.md: PLAN-003: isn't numbered after an akt of its file, so it begins a file of its own",
        "- akter/PLAN-002.md: is named for PLAN-002, its first akt, as `PLAN-002-other.md`",
    ] {
        assert!(
            refused.message.contains(says),
            "no `{says}` in:\n{}",
            refused.message
        );
    }
}

// r[verify cli.members]
// r[verify cli.file.multi]
#[test]
fn cat_gives_an_akt_with_its_members_as_one_file() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["new", "gap", "--in", "PLAN-001", "--set", "title=Open"]);
    dir.ok(&["move", "PLAN-001-G01", "--before", "PLAN-001-T02"]);
    let json = |args: &[&str]| {
        let mut all = args.to_vec();
        all.extend(["--format", "json"]);
        dir.ok(&all).json["akter"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["number"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    // In the container's order, whatever their types.
    assert_eq!(
        json(&["cat", "PLAN-001", "--all-members"]),
        ["PLAN-001", "PLAN-001-T01", "PLAN-001-G01", "PLAN-001-T02"]
    );
    assert_eq!(
        json(&["cat", "PLAN-001", "--members", "task"]),
        ["PLAN-001", "PLAN-001-T01", "PLAN-001-T02"]
    );
    assert_eq!(
        json(&["cat", "PLAN-001-T01", "--all-members"]),
        ["PLAN-001-T01"]
    );
    assert_eq!(
        dir.refused(&["cat", "PLAN-001", "--members", "nope"]).code,
        "UNKNOWN_AKT_TYPE"
    );

    // One file: its own first, each member after its line, without
    // `number`; the plan's file, but for `rank`.
    let text = dir.ok(&["cat", "PLAN-001", "--all-members"]).markdown;
    assert!(text.starts_with("---\nnumber: PLAN-001\n"), "{text}");
    assert!(
        text.contains("\n<!-- akt: PLAN-001-G01 -->\n---\ntype: gap\n"),
        "{text}"
    );
    let stored = std::fs::read_to_string(dir.root.join("akter/PLAN-001-local-writes.md")).unwrap();
    let unranked = stored
        .lines()
        .filter(|l| !l.starts_with("rank: "))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(text.trim_end(), unranked.trim_end());
    // One akt is its file, as before.
    assert!(
        !dir.ok(&["cat", "PLAN-001-T01"])
            .markdown
            .contains("<!-- akt:")
    );

    // Written out, each to its own file.
    let out = dir.root.parent().unwrap().join("out");
    dir.ok(&[
        "cat",
        "PLAN-001",
        "--members",
        "gap",
        "-o",
        out.to_str().unwrap(),
    ]);
    let mut names: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["PLAN-001-G01.md", "PLAN-001.md"]);
}

// r[verify cli.apply]
// r[verify cli.file.multi]
#[test]
fn applies_a_multi_akt_file_checking_every_akt_first() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["new", "gap", "--in", "PLAN-001", "--set", "title=Open"]);
    let plan = dir.root.parent().unwrap().join("plan.md");
    let write = |text: &str| std::fs::write(&plan, text).unwrap();
    let apply = |args: &[&str]| {
        let mut all = vec!["apply", plan.to_str().unwrap()];
        all.extend_from_slice(args);
        dir.run(&all)
    };
    let read = dir.ok(&["cat", "PLAN-001", "--all-members"]).markdown;

    // Unchanged, nothing changes.
    write(&read);
    let said = apply(&["--format", "json"]).unwrap().json;
    assert_eq!(said["applied"].as_array().unwrap().len(), 4);
    assert!(
        said["applied"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["changed"] == json!([]))
    );

    // Two tasks edited in place: exactly those change.
    let edited = read
        .replace("title: First\n", "title: First edited\n")
        .replace("title: Second\n", "title: Second edited\n");
    write(&edited);
    let said = apply(&["--reason", "Edited the tasks", "--format", "json"])
        .unwrap()
        .json;
    let changed: Vec<(String, Value)> = said["applied"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["number"].as_str().unwrap().to_owned(),
                a["changed"].clone(),
            )
        })
        .collect();
    assert_eq!(
        changed,
        [
            ("PLAN-001".to_owned(), json!([])),
            ("PLAN-001-T01".to_owned(), json!(["title"])),
            ("PLAN-001-T02".to_owned(), json!(["title"])),
            ("PLAN-001-G01".to_owned(), json!([])),
        ]
    );
    assert_eq!(said["reason"], "Edited the tasks");
    assert!(file(&dir, "PLAN-001-T02").contains("title: Second edited"));

    // Stale or read-only anywhere, nothing changes.
    let stored = std::fs::read_to_string(dir.root.join("akter/PLAN-001-local-writes.md")).unwrap();
    let again = edited.replace("title: Open\n", "title: Open edited\n");
    write(&again);
    assert_eq!(apply(&[]).unwrap_err().code, "STALE");
    let fresh = dir
        .ok(&["cat", "PLAN-001", "--all-members"])
        .markdown
        .replace("title: First edited\n", "title: First again\n");
    write(&fresh.replace("type: gap\n", "type: task\n"));
    assert_eq!(apply(&[]).unwrap_err().code, "READ_ONLY");
    assert_eq!(
        std::fs::read_to_string(dir.root.join("akter/PLAN-001-local-writes.md")).unwrap(),
        stored,
        "nothing changed"
    );

    // An akt given twice, and a line and a `number` that disagree.
    write(&format!(
        "{fresh}\n<!-- akt: PLAN-001-T01 -->\n{}",
        file(&dir, "PLAN-001-T01")
    ));
    assert!(apply(&[]).unwrap_err().message.contains("given twice"));
    write(&fresh.replacen(
        "<!-- akt: PLAN-001-T01 -->\n---\n",
        "<!-- akt: PLAN-001-T01 -->\n---\nnumber: PLAN-001-T02\n",
        1,
    ));
    assert!(
        apply(&[])
            .unwrap_err()
            .message
            .contains("its `number` is PLAN-001-T02")
    );
}

/// A protocol type, laying out its tests in sections, and a test type,
/// with testers and their verdicts, in `dir`.
fn protocols(dir: &Dir) {
    let states = json!([{ "key": "open", "name": "Open", "initial": true }]);
    let protocol = json!({
        "key": "protocol", "name": "Protocol", "number": "PROT-{seq:3}", "states": states,
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "layout", "name": "Layout", "kind": "sections", "of": ["test"] }
        ],
        "contains": [{ "type": "test" }]
    });
    let test = json!({
        "key": "test", "name": "Test", "number": "{parent}-T{seq}", "states": states,
        "created_in": ["protocol"],
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "testers", "name": "Testers", "kind": "user", "multiple": true },
            { "key": "verdict", "name": "Verdict", "kind": "verdict", "assignees": "testers" },
            { "key": "reviewed", "name": "Reviewed", "kind": "checkbox" }
        ]
    });
    for (key, document) in [("protocol", protocol), ("test", test)] {
        std::fs::write(
            dir.root.join(format!("types/{key}.json")),
            serde_json::to_string_pretty(&document).unwrap(),
        )
        .unwrap();
    }
}

/// The outcome `show --format json` gives `number`.
fn outcome(dir: &Dir, number: &str) -> Value {
    dir.ok(&["show", number, "--format", "json"]).json["akt"]["outcome"].clone()
}

/// A release protocol of three tests, the first two in a section, the
/// first with two testers.
fn released(dir: &Dir) {
    protocols(dir);
    dir.ok(&["new", "protocol", "--set", "title=Release"]);
    for (title, testers) in [
        ("Sign in", "@me,bo@kuria.example"),
        ("Sign out", "@me"),
        ("Reset", "@me"),
    ] {
        dir.ok(&[
            "new",
            "test",
            "--in",
            "PROT-001",
            "--set",
            &format!("title={title}"),
            "--set",
            &format!("testers={testers}"),
        ]);
    }
    assert_eq!(outcome(dir, "PROT-001"), json!("pending"));

    let layout = dir.root.parent().unwrap().join("layout.yaml");
    std::fs::write(
        &layout,
        "- title: Session\n  collapsible: true\n  text: Each way in and out.\n  items:\n  - PROT-001-T1\n  - PROT-001-T2\n- PROT-001-T3\n",
    )
    .unwrap();
    dir.ok(&["set", "PROT-001", &format!("layout=@{}", layout.display())]);
}

// r[verify cli.file.kinds]
// r[verify cli.local.rules]
// r[verify akt.outcome]
// r[verify akt.outcome.section]
// r[verify akt.field.sections]
// r[verify akt.field.verdict]
#[test]
fn tests_in_sections_take_verdicts_and_add_up() {
    let dir = Dir::new();
    released(&dir);
    // Sections place only members, each once.
    let layout = dir.root.parent().unwrap().join("bad.yaml");
    std::fs::write(&layout, "- PROT-001\n").unwrap();
    let at = format!("layout=@{}", layout.display());
    assert_eq!(dir.refused(&["set", "PROT-001", &at]).code, "REFUSED");
    std::fs::write(&layout, "- PROT-001-T1\n- PROT-001-T1\n").unwrap();
    assert_eq!(dir.refused(&["set", "PROT-001", &at]).code, "REFUSED");
    let text = dir.ok(&["cat", "PROT-001"]).markdown;
    assert!(
        text.contains("state: open\noutcome: pending\n"),
        "outcome after state: {text}"
    );
    assert!(
        text.contains("<!-- field: layout -->\n- title: Session\n  text: Each way in and out.\n  collapsible: true\n  items:\n  - PROT-001-T1\n  - PROT-001-T2\n- PROT-001-T3\n"),
        "{text}"
    );
    let test = dir.ok(&["cat", "PROT-001-T1"]).markdown;
    assert!(
        test.contains("testers: [me@kuria.example, bo@kuria.example]\n"),
        "{test}"
    );

    // Verdicts, one's own and one given for another assignee.
    let said = dir.ok(&["verdict", "PROT-001-T1", "pass"]);
    assert!(
        said.markdown.starts_with("PROT-001-T1: pass (revision "),
        "{}",
        said.markdown
    );
    assert_eq!(said.json["outcome"], json!("pending"), "bo hasn't answered");
    dir.ok(&[
        "verdict",
        "PROT-001-T1",
        "skip",
        "--for",
        "bo@kuria.example",
    ]);
    assert_eq!(outcome(&dir, "PROT-001-T1"), json!("pass"));
    assert_eq!(
        dir.refused(&[
            "verdict",
            "PROT-001-T1",
            "pass",
            "--for",
            "eve@kuria.example"
        ])
        .code,
        "REFUSED",
        "not an assignee"
    );
    assert_eq!(
        dir.refused(&["verdict", "PROT-001-T1", "maybe"]).code,
        "BAD_REQUEST"
    );
    assert_eq!(
        dir.refused(&["verdict", "PROT-001", "pass"]).code,
        "BAD_REQUEST"
    );
    assert_eq!(
        dir.refused(&["set", "PROT-001-T1", "verdict=pass"]).code,
        "READ_ONLY"
    );
    dir.ok(&["verdict", "PROT-001-T2", "fail"]);
    assert_eq!(outcome(&dir, "PROT-001"), json!("fail"));
    dir.ok(&["verdict", "PROT-001-T2", "clear"]);
    dir.ok(&["verdict", "PROT-001-T2", "skip"]);
    assert_eq!(outcome(&dir, "PROT-001"), json!("pending"), "T3 is open");
    dir.ok(&["verdict", "PROT-001-T3", "pass"]);
    assert_eq!(outcome(&dir, "PROT-001"), json!("pass"));

    let shown = dir.ok(&["show", "PROT-001"]);
    assert!(
        shown.markdown.contains("Protocol · Open · pass · revision"),
        "{}",
        shown.markdown
    );
    assert!(
        shown.markdown.contains("## Sections\n\n- **Session** — pass\n  Each way in and out.\n  - PROT-001-T1 · Sign in (Test, Open) — pass\n  - PROT-001-T2 · Sign out (Test, Open) — skip\n- PROT-001-T3 · Reset (Test, Open) — pass\n"),
        "{}",
        shown.markdown
    );
    assert_eq!(
        shown.json["akt"]["members"]["sections"][0]["outcome"],
        json!("pass")
    );
    let listed = dir.ok(&["ls", "test", "--in", "PROT-001"]).markdown;
    assert!(listed.contains("| Outcome |"), "{listed}");
}

// r[verify cli.file.kinds]
// r[verify akt.field.sections]
#[test]
fn protocol_files_apply_and_a_test_leaves_its_section() {
    let dir = Dir::new();
    released(&dir);
    for test in ["PROT-001-T1", "PROT-001-T2", "PROT-001-T3"] {
        dir.ok(&["verdict", test, "pass"]);
    }
    dir.ok(&[
        "verdict",
        "PROT-001-T1",
        "skip",
        "--for",
        "bo@kuria.example",
    ]);
    // A file read and applied unchanged changes nothing; its verdicts and
    // outcome are read-only.
    let out = dir.root.parent().unwrap().join("out");
    dir.ok(&[
        "cat",
        "PROT-001",
        "PROT-001-T1",
        "-o",
        out.to_str().unwrap(),
    ]);
    let applied = dir.ok(&[
        "apply",
        out.join("PROT-001.md").to_str().unwrap(),
        out.join("PROT-001-T1.md").to_str().unwrap(),
    ]);
    assert_eq!(
        applied.markdown,
        "PROT-001: nothing changed.\nPROT-001-T1: nothing changed."
    );
    let file = out.join("PROT-001-T1.md");
    let read = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, read.replace(": skip", ": fail")).unwrap();
    assert_eq!(
        dir.refused(&["apply", file.to_str().unwrap()]).code,
        "READ_ONLY"
    );
    std::fs::write(&file, read.replace("outcome: pass", "outcome: fail")).unwrap();
    assert_eq!(
        dir.refused(&["apply", file.to_str().unwrap()]).code,
        "READ_ONLY"
    );

    // A test that leaves takes its place in the sections with it.
    dir.ok(&["remove", "PROT-001", "PROT-001-T3"]);
    assert!(
        !dir.ok(&["cat", "PROT-001"])
            .markdown
            .contains("- PROT-001-T3")
    );
    dir.ok(&["check"]);
}

// r[verify akt.copy]
// r[verify akt.copy.only]
// r[verify akt.copy.links]
#[test]
fn copies_a_protocol_with_the_tests_chosen() {
    let dir = Dir::new();
    protocols(&dir);
    dir.ok(&["new", "protocol", "--set", "title=Template"]);
    for title in ["One", "Two"] {
        dir.ok(&[
            "new",
            "test",
            "--in",
            "PROT-001",
            "--set",
            &format!("title={title}"),
            "--set",
            "testers=@me",
        ]);
    }
    let layout = dir.root.parent().unwrap().join("layout.yaml");
    std::fs::write(
        &layout,
        "- title: First\n  items: [PROT-001-T1]\n- title: Second\n  items: [PROT-001-T2]\n",
    )
    .unwrap();
    dir.ok(&["set", "PROT-001", &format!("layout=@{}", layout.display())]);
    dir.ok(&["verdict", "PROT-001-T1", "pass"]);
    dir.ok(&["set", "PROT-001-T1", "reviewed=true"]);
    assert!(
        dir.ok(&["cat", "PROT-001-T1"])
            .markdown
            .contains("reviewed: true\n")
    );

    let copied = dir.ok(&[
        "copy",
        "PROT-001",
        "--only",
        "PROT-001-T1",
        "--reason",
        "Run it",
    ]);
    assert_eq!(
        copied.markdown,
        "Copied PROT-001 to PROT-002, with 1 member:\n\n- PROT-001-T1 → PROT-002-T1\n\nCommit with: Run it"
    );
    assert_eq!(
        copied.json["members"],
        json!([{ "from": "PROT-001-T1", "number": "PROT-002-T1" }])
    );
    let copy = dir.ok(&["cat", "PROT-002"]).markdown;
    assert!(
        copy.contains("<!-- field: layout -->\n- title: First\n  items:\n  - PROT-002-T1\n"),
        "sections pointing at the copies, an emptied one gone: {copy}"
    );
    assert!(!copy.contains("Second"), "{copy}");
    let test = dir.ok(&["cat", "PROT-002-T1"]).markdown;
    assert!(test.contains("testers: [me@kuria.example]\n"), "{test}");
    assert!(!test.contains("verdict:"), "a copy has no verdicts: {test}");
    assert!(!test.contains("reviewed:"), "nor checked boxes: {test}");
    assert!(test.contains("outcome: pending\n"), "{test}");
    assert_eq!(outcome(&dir, "PROT-001"), json!("pending"));

    assert_eq!(
        dir.refused(&["copy", "PROT-001", "--only", "PROT-009"])
            .code,
        "BAD_REQUEST"
    );
    dir.ok(&["copy", "PROT-001"]);
    assert_eq!(
        numbers(&dir.ok(&["ls", "test", "--in", "PROT-003", "--format", "json"])),
        ["PROT-003-T1", "PROT-003-T2"]
    );
    dir.ok(&["check"]);
}
