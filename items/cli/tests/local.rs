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
/// A real one-pixel PNG, that pandoc can size and embed.
const PIXEL: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0xc9, 0xfe, 0x92, 0xef, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

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
    let schema = dir
        .ok(&["types", "schema", "task", "--format", "json"])
        .json;
    assert_eq!(
        schema["type"]["fields"][3],
        json!({
            "key": "assignee", "name": "Assignee", "kind": "user", "required": false, "multiline": false,
            "description": null, "options": [], "format": null, "draft": null, "references": null,
        })
    );
    assert_eq!(schema["type"]["presentation"]["icon"], "list_checks");
    assert_eq!(schema["type"]["links"][0]["key"], "blocks");
    let exported = dir.ok(&["types", "export", "gap", "--format", "json"]).json;
    assert_eq!(exported["types"][0]["key"], "gap");
    assert_eq!(
        dir.refused(&["types", "schema", "nope"]).kind.exit_code(),
        2
    );
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
    dir.ok(&[
        "members",
        "place",
        "PLAN-001-T02",
        "--before",
        "PLAN-001-T01",
    ]);
    assert_eq!(
        numbers(&dir.ok(&["ls", "task", "--in", "PLAN-001", "--format", "json"])),
        ["PLAN-001-T02", "PLAN-001-T01"]
    );
    assert_eq!(
        dir.refused(&[
            "members",
            "place",
            "PLAN-001-T02",
            "--after",
            "PLAN-001-T02"
        ])
        .code,
        "BAD_REQUEST"
    );
    // Membership: a transfer is one change of all three.
    let before = revision_of(&dir, "PLAN-002");
    dir.ok(&[
        "members",
        "transfer",
        "PLAN-001-T02",
        "PLAN-001",
        "PLAN-002",
    ]);
    assert_eq!(
        dir.ok(&["show", "PLAN-001-T02", "--format", "json"]).json["akt"]["in"],
        json!(["PLAN-002"])
    );
    assert_eq!(revision_of(&dir, "PLAN-002"), before + 1);
    assert_eq!(
        dir.refused(&["members", "add", "PLAN-002", "PLAN-001-T02"])
            .code,
        "ALREADY_MEMBER"
    );
    assert_eq!(
        dir.refused(&["members", "add", "PLAN-001", "PLAN-002"])
            .code,
        "NOT_CONTAINABLE"
    );
    assert_eq!(
        dir.refused(&["members", "remove", "PLAN-001", "PLAN-001-T02"])
            .code,
        "NOT_MEMBER"
    );
    dir.ok(&["members", "remove", "PLAN-002", "PLAN-001-T02"]);
    dir.ok(&["members", "add", "PLAN-001", "PLAN-001-T02"]);
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
        dir.ok(&["types", "schema", "task", "--format", "json"])
            .json["type"]["name"],
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

/// Defines `plan` again as a type `key` numbered by `pattern`.
fn define_numbered(dir: &Dir, key: &str, pattern: &str, sequence: Option<&str>) {
    let example =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/planning/plan.json");
    let mut ty: Value = serde_json::from_str(&std::fs::read_to_string(example).unwrap()).unwrap();
    ty["key"] = json!(key);
    ty["name"] = json!(key);
    ty["number"] = json!(pattern);
    if let Some(sequence) = sequence {
        ty["sequence"] = json!(sequence);
    }
    let file = dir.root.join(format!("../{key}.json"));
    std::fs::write(&file, ty.to_string()).unwrap();
    dir.ok(&["types", "define", file.to_str().unwrap()]);
}

// r[verify cli.local.numbers]
// r[verify akt.number.random]
#[test]
fn draws_random_numbers_free_among_the_directory_s() {
    let dir = Dir::new();
    define_numbered(&dir, "ticket", "T-{rand:1}", None);
    let mut numbers: Vec<String> = (0..10)
        .map(|i| {
            let title = format!("title=Ticket {i}");
            dir.ok(&["new", "ticket", "--set", &title, "--format", "json"])
                .json["number"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    numbers.sort();
    let every: Vec<String> = (0..10).map(|n| format!("T-{n}")).collect();
    assert_eq!(numbers, every);
    let refused = dir.refused(&["new", "ticket", "--set", "title=One more"]);
    assert_eq!(refused.code, "NUMBERS_EXHAUSTED", "{}", refused.message);
}

// r[verify akt.number.shared]
#[test]
fn akt_types_naming_one_sequence_share_its_numbers() {
    let dir = Dir::new();
    define_numbered(&dir, "policy", "QMS-{seq:4}", Some("qms"));
    define_numbered(&dir, "procedure", "QMS-{seq:4}", Some("qms"));
    let new = |key: &str| {
        dir.ok(&["new", key, "--set", "title=T", "--format", "json"])
            .json["number"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(new("policy"), "QMS-0001");
    assert_eq!(new("procedure"), "QMS-0002");
    // `plan` is numbered `PLAN-{seq:3}`, unnamed: another type can't be.
    let example =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/planning/plan.json");
    let mut ty: Value = serde_json::from_str(&std::fs::read_to_string(example).unwrap()).unwrap();
    ty["key"] = json!("project");
    let file = dir.root.join("../project.json");
    std::fs::write(&file, ty.to_string()).unwrap();
    let refused = dir.refused(&["types", "define", file.to_str().unwrap()]);
    assert!(refused.message.contains("`plan`"), "{}", refused.message);
}

// r[verify cli.local.versions]
// r[verify version.release-file]
#[test]
fn releases_an_external_document_with_its_pdf() {
    let dir = Dir::new();
    let ty = json!({
        "key": "external_document", "name": "External document", "number": "EXT-{seq:3}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "pdf", "name": "PDF", "kind": "attachment", "media_types": ["application/pdf"] }
        ],
        "states": [{ "key": "draft", "name": "Draft", "initial": true }],
        "versions": { "file": "pdf" }
    });
    let document = dir.root.join("../external.json");
    std::fs::write(&document, ty.to_string()).unwrap();
    dir.ok(&["types", "define", document.to_str().unwrap()]);
    dir.ok(&["new", "external_document", "--set", "title=Supplier policy"]);
    let pdf = dir.root.join("../policy.pdf");
    std::fs::write(&pdf, b"%PDF-1.4 first").unwrap();
    // Only a release sets it.
    let at = format!("pdf=@{}", pdf.display());
    assert_eq!(dir.refused(&["set", "EXT-001", &at]).code, "REFUSED");
    dir.commit("Supplier policy");
    assert_eq!(dir.refused(&["release", "EXT-001"]).code, "BAD_REQUEST");
    let released = dir
        .ok(&[
            "release",
            "EXT-001",
            "--file",
            pdf.to_str().unwrap(),
            "--description",
            "As the supplier published it.",
            "--format",
            "json",
        ])
        .json;
    assert_eq!(released["version"], 1);
    assert_eq!(released["revision"], 2);
    // r[verify version.number]: its description, listed.
    let listed = dir.ok(&["versions", "EXT-001", "--format", "json"]).json;
    assert_eq!(
        listed["versions"][0]["description"],
        "As the supplier published it."
    );
    assert_eq!(
        std::fs::read(dir.root.join("attachments/EXT-001/policy.pdf")).unwrap(),
        b"%PDF-1.4 first"
    );
    assert!(file(&dir, "EXT-001").contains("attachments/EXT-001/policy.pdf"));
    dir.commit("Version 1");
    // Again, with no other change.
    std::fs::write(&pdf, b"%PDF-1.4 second").unwrap();
    let released = dir
        .ok(&[
            "release",
            "EXT-001",
            "--file",
            pdf.to_str().unwrap(),
            "--format",
            "json",
        ])
        .json;
    assert_eq!(
        (released["version"].as_i64(), released["revision"].as_i64()),
        (Some(2), Some(3))
    );
}

// r[verify cli.docs-import]
// r[verify version.release-file]
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one folder imported from start to end"
)]
fn imports_a_document_from_a_folder_its_old_versions_as_pdfs() {
    let has_pandoc = Command::new("pandoc").arg("--version").output().is_ok();
    if !has_pandoc {
        eprintln!("skipped: pandoc isn't on the PATH");
        return;
    }
    let dir = Dir::new();
    let documents = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/documents");
    for key in ["image", "document", "document_index"] {
        let file = documents.join(format!("{key}.json"));
        dir.ok(&["types", "define", file.to_str().unwrap()]);
    }
    dir.ok(&["new", "document_index", "--set", "title=Manual"]);
    dir.commit("A manual");

    // The folder: a docx draft with a picture, made here by pandoc, and
    // two PDFs.
    let folder = dir.root.join("../my-doc-folder");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("draft.md"),
        "# My doc\n\nHello **world**, from Word.\n\n![The flow](flow.png){width=50%}\n",
    )
    .unwrap();
    std::fs::write(folder.join("flow.png"), PIXEL).unwrap();
    let made = Command::new("pandoc")
        .current_dir(&folder)
        .args(["draft.md", "-o", "draft.docx"])
        .output()
        .unwrap();
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    std::fs::remove_file(folder.join("draft.md")).unwrap();
    std::fs::remove_file(folder.join("flow.png")).unwrap();
    std::fs::write(folder.join("version1.pdf"), b"%PDF-1.4 one").unwrap();
    std::fs::write(folder.join("version2.pdf"), b"%PDF-1.4 two").unwrap();
    let metadata = json!({
        "title": "My doc",
        "owner": "bruce@example.com",
        "draft": "draft.docx",
        "versions": [
            { "label": "version 1", "description": "foo bar", "file": "version1.pdf" },
            { "label": "version 2", "description": "bar baz", "file": "version2.pdf" }
        ]
    });
    std::fs::write(folder.join("metadata.json"), metadata.to_string()).unwrap();
    let folder = folder.to_str().unwrap();

    // Refused while anything is uncommitted, and before anything changes.
    std::fs::write(dir.root.join("notes.txt"), "wip").unwrap();
    let refused = dir.refused(&[
        "docs", "import", folder, "--type", "document", "--in", "DOCS-1",
    ]);
    assert_eq!(refused.code, "UNCOMMITTED", "{}", refused.message);
    std::fs::remove_file(dir.root.join("notes.txt")).unwrap();

    let imported = dir
        .ok(&[
            "docs", "import", folder, "--type", "document", "--in", "DOCS-1", "--format", "json",
        ])
        .json;
    assert_eq!(
        imported,
        json!({ "number": "DOCS-1-D01", "versions": [1, 2], "pictures": ["IMG-1"], "picturesLeftOut": 0 })
    );
    // Its picture an image akt, embedded so.
    let image = file(&dir, "IMG-1");
    assert!(image.contains("title: The flow"), "{image}");
    assert!(image.contains("attachments/IMG-1/"), "{image}");
    let listed = dir.ok(&["versions", "DOCS-1-D01", "--format", "json"]).json;
    let said: Vec<(Value, Value)> = listed["versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["label"].clone(), v["description"].clone()))
        .collect();
    assert_eq!(
        said,
        [
            (json!("version 2"), json!("bar baz")),
            (json!("version 1"), json!("foo bar"))
        ]
    );
    // Version 1 is its PDF; the living version is the draft, with no PDF.
    let first = dir.ok(&["cat", "DOCS-1-D01", "--version", "1"]).markdown;
    assert!(
        first.contains("attachments/DOCS-1-D01/version1.pdf"),
        "{first}"
    );
    let living = file(&dir, "DOCS-1-D01");
    assert!(living.contains("Hello **world**, from Word."), "{living}");
    assert!(living.contains("![The flow](kuria:IMG-1)"), "{living}");
    assert!(living.contains("owner: bruce@example.com"), "{living}");
    assert!(living.contains("by: me@kuria.example"), "{living}");
    // Each step committed, and nothing left over.
    assert!(
        dir.ok(&["history", "DOCS-1-D01"])
            .markdown
            .contains("version 2")
    );
    let status = Command::new("git")
        .arg("-C")
        .arg(&dir.root)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(
        status.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&status.stdout)
    );
}

// r[verify cli.docs-import]
// r[verify akt.number.given]
#[test]
#[expect(clippy::too_many_lines, reason = "three folders imported in turn")]
fn imported_documents_keep_their_numbers_and_refer_to_each_other() {
    let dir = Dir::new();
    let documents = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/documents");
    for key in ["image", "document"] {
        let file = documents.join(format!("{key}.json"));
        let mut ty: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        if key == "document" {
            // Numbered as the system they come from numbers them.
            ty["key"] = json!("procedure");
            ty["number"] = json!("FOOID-{seq:4}");
            ty["created_in"] = json!([]);
            ty["links"][0]["to"] = json!(["procedure", "image"]);
            ty["fields"][2]["references"] = json!("refers_to");
        }
        let written = dir.root.join(format!("../{key}.json"));
        std::fs::write(&written, ty.to_string()).unwrap();
        dir.ok(&["types", "define", written.to_str().unwrap()]);
    }
    dir.commit("Types");
    let folder = |name: &str, metadata: Value, draft: &str| {
        let folder = dir.root.join(format!("../{name}"));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("draft.md"), draft).unwrap();
        std::fs::write(folder.join("metadata.json"), metadata.to_string()).unwrap();
        folder.to_str().unwrap().to_owned()
    };
    let manual = folder(
        "manual",
        json!({ "number": "FOOID-0001", "title": "Quality manual", "draft": "draft.md" }),
        "# Quality manual\n\nSuppliers follow FOOID-0002 Supplier Policy.\n",
    );
    let policy = folder(
        "policy",
        json!({ "number": "FOOID-0002", "title": "Supplier policy", "draft": "draft.md" }),
        "# Supplier policy\n\nPart of FOOID-0001 Quality Manual, not `FOOID-0001` in code.\n",
    );
    let pattern = r"(?<id>FOOID-\d{4})(?<label>(?:\s?\p{Lu}\w+)*)";
    let import = |folder: &str| {
        dir.ok(&[
            "docs",
            "import",
            folder,
            "--type",
            "procedure",
            "--references",
            pattern,
            "--format",
            "json",
        ])
        .json["number"]
            .clone()
    };
    // The manual first: its reference to the policy is broken until then.
    assert_eq!(import(&manual), "FOOID-0001");
    assert_eq!(import(&policy), "FOOID-0002");
    let manual = file(&dir, "FOOID-0001");
    assert!(
        manual.contains("Suppliers follow [Supplier Policy](kuria:FOOID-0002)."),
        "{manual}"
    );
    let policy = file(&dir, "FOOID-0002");
    assert!(
        policy.contains("Part of [Quality Manual](kuria:FOOID-0001), not `FOOID-0001` in code."),
        "{policy}"
    );
    // Taken, and not a number the pattern gives.
    let taken = dir.refused(&[
        "new",
        "procedure",
        "--set",
        "title=T",
        "--number",
        "FOOID-0002",
    ]);
    assert_eq!(taken.code, "NUMBER_TAKEN", "{}", taken.message);
    let wrong = dir.refused(&["new", "procedure", "--set", "title=T", "--number", "BAR-1"]);
    assert_eq!(wrong.code, "BAD_REQUEST", "{}", wrong.message);
    // The next goes on past them.
    let next = dir
        .ok(&["new", "procedure", "--set", "title=T", "--format", "json"])
        .json;
    assert_eq!(next["number"], "FOOID-0003");
    dir.commit("Another procedure");

    // Numbers written short, padded as the pattern writes them.
    let short = folder(
        "short",
        json!({ "number": "FOOID-12", "title": "Short", "draft": "draft.md" }),
        "# Short\n\nSee FOOID-1 Quality Manual.\n",
    );
    let padded = dir
        .ok(&[
            "docs",
            "import",
            &short,
            "--type",
            "procedure",
            "--references",
            r"(?<id>FOOID-\d{1,4})(?<label>(?:\s?\p{Lu}\w+)*)",
            "--pad-numbers",
            "--format",
            "json",
        ])
        .json;
    assert_eq!(padded["number"], "FOOID-0012");
    let short = file(&dir, "FOOID-0012");
    assert!(
        short.contains("See [Quality Manual](kuria:FOOID-0001)."),
        "{short}"
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
    let unpacked =
        kuria_cli::local::convert::bundle_of(&dir.root, Some("me@kuria.example".to_owned()))
            .unwrap();
    let bundle = &unpacked.bundle;

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
    kuria_cli::local::convert::to_dir(&unpacked, &again).unwrap();
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
    dir.ok(&[
        "members",
        "place",
        "PLAN-001-G01",
        "--before",
        "PLAN-001-T01",
    ]);
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
    dir.ok(&["members", "remove", "PLAN-001", "PLAN-001-T02"]);
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
    dir.ok(&[
        "members",
        "place",
        "PLAN-001-G01",
        "--before",
        "PLAN-001-T02",
    ]);
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
        "key": "protocol", "name": "Protocol", "number": "PROT-{seq:3}",
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "done", "name": "Done", "final": true }
        ],
        "transitions": [{ "key": "finish", "name": "Finish", "from": ["open"], "to": "done",
                          "outcomes": ["pass", "fail", "skip"] }],
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

// r[verify akt.transition.outcomes]
#[test]
fn a_protocol_finishes_only_with_an_outcome_it_allows() {
    let dir = Dir::new();
    released(&dir);
    let schema = dir.ok(&["types", "schema", "protocol"]).markdown;
    assert!(
        schema
            .contains("- `finish` Finish: open → done, only if the outcome is pass, fail, skip\n"),
        "{schema}"
    );
    assert_eq!(
        dir.ok(&["types", "schema", "protocol", "--format", "json"])
            .json["type"]["transitions"][0]["outcomes"],
        json!(["pass", "fail", "skip"])
    );
    let pending = dir.refused(&["do", "PROT-001", "finish"]);
    assert_eq!(pending.code, "REFUSED");
    assert_eq!(
        pending.message,
        "`finish` requires the outcome to be pass, fail or skip, and it is pending"
    );
    for test in ["PROT-001-T2", "PROT-001-T3"] {
        dir.ok(&["verdict", test, "fail", "--description", "It broke"]);
    }
    // Bo hasn't answered T1, but a failure is an outcome.
    let done = dir.ok(&["do", "PROT-001", "finish"]);
    assert_eq!(done.json["state"], "done", "{}", done.markdown);
    // r[verify cli.local.rules]
    // Its file applied unchanged changes nothing, final as it is.
    let file = dir.root.parent().unwrap().join("finished.md");
    std::fs::write(&file, dir.ok(&["cat", "PROT-001"]).markdown).unwrap();
    let applied = dir.ok(&["apply", file.to_str().unwrap()]);
    assert!(
        applied.markdown.contains("PROT-001: nothing changed."),
        "{}",
        applied.markdown
    );
}

// r[verify akt.filter.outcome]
#[test]
fn tests_and_protocols_filter_by_outcome() {
    let dir = Dir::new();
    released(&dir);
    let suite = json!({
        "key": "suite", "name": "Suite", "number": "S-{seq}",
        "states": [{ "key": "open", "name": "Open", "initial": true }],
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "contains": [{ "type": "protocol" }]
    });
    std::fs::write(dir.root.join("types/suite.json"), suite.to_string()).unwrap();
    dir.ok(&["new", "suite", "--set", "title=Nightly"]);
    dir.ok(&["new", "protocol", "--in", "S-1", "--set", "title=Smoke"]);
    dir.ok(&["members", "add", "S-1", "PROT-001"]);
    dir.ok(&[
        "verdict",
        "PROT-001-T2",
        "fail",
        "--description",
        "It broke",
    ]);
    let ls = |args: &[&str]| {
        let mut all = vec!["ls"];
        all.extend_from_slice(args);
        all.extend(["--format", "json"]);
        let mut found = numbers(&dir.ok(&all));
        found.sort();
        found
    };
    assert_eq!(
        ls(&["test", "--outcome", "pending"]),
        ["PROT-001-T1", "PROT-001-T3"]
    );
    assert_eq!(
        ls(&["test", "--outcome", "FAIL", "--limit", "1"]),
        ["PROT-001-T2"]
    );
    assert_eq!(
        ls(&["test", "--in", "PROT-001", "--outcome", "fail"]),
        ["PROT-001-T2"]
    );
    // A protocol's outcome comes from its tests: only within a container.
    assert_eq!(
        ls(&["protocol", "--in", "S-1", "--outcome", "fail"]),
        ["PROT-001"]
    );
    assert_eq!(
        ls(&["protocol", "--in", "S-1", "--outcome", "pending"]),
        ["PROT-002"]
    );
    for args in [
        &["ls", "protocol", "--outcome", "fail"][..],
        &["ls", "suite", "--outcome", "fail"][..],
        &["ls", "--outcome", "fail"][..],
    ] {
        assert_eq!(dir.refused(args).code, "BAD_REQUEST", "{args:?}");
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
#[expect(clippy::too_many_lines, reason = "one test taking verdicts in turn")]
fn tests_in_sections_take_verdicts_and_add_up() {
    let dir = Dir::new();
    released(&dir);
    // r[verify akt.filter]
    // Testers filter by any one of them.
    let where_ = |who: &str| {
        numbers(&dir.ok(&[
            "ls",
            "test",
            "--where",
            &format!("testers={who}"),
            "--format",
            "json",
        ]))
    };
    assert_eq!(where_("bo@kuria.example"), ["PROT-001-T1"]);
    let mut mine = where_("@me");
    mine.sort();
    assert_eq!(mine, ["PROT-001-T1", "PROT-001-T2", "PROT-001-T3"]);
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
        "--description",
        "Bo is away",
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
    // A fail or a skip says why (`akt.field.verdict.description`).
    assert_eq!(
        dir.refused(&["verdict", "PROT-001-T1", "fail"]).code,
        "REFUSED"
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
    dir.ok(&[
        "verdict",
        "PROT-001-T2",
        "fail",
        "--description",
        "It broke",
    ]);
    assert_eq!(outcome(&dir, "PROT-001"), json!("fail"));
    dir.ok(&["verdict", "PROT-001-T2", "clear"]);
    dir.ok(&[
        "verdict",
        "PROT-001-T2",
        "skip",
        "--description",
        "No device",
    ]);
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
        "--description",
        "Bo is away",
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
    dir.ok(&["members", "remove", "PROT-001", "PROT-001-T3"]);
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

/// A document type with a `markdown` body whose references are kept as
/// `refers_to` links.
fn documents(dir: &Dir) {
    let document = json!({
        "key": "document", "name": "Document", "number": "DOC-{seq}",
        "states": [{ "key": "draft", "name": "Draft", "initial": true }],
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "body", "name": "Body", "kind": "markdown", "references": "refers_to" }
        ],
        "links": [{ "key": "refers_to", "name": "Refers to", "inverse": "Referred to by", "to": ["document"] }],
        "versions": {}
    });
    std::fs::write(
        dir.root.join("types/document.json"),
        serde_json::to_string_pretty(&document).unwrap(),
    )
    .unwrap();
}

// r[verify akt.field.markdown]
// r[verify akt.field.markdown.references]
// r[verify cli.local.references]
// r[verify cli.file.kinds]
#[test]
fn markdown_fields_get_heading_ids_and_keep_reference_links() {
    let dir = Dir::new();
    documents(&dir);
    let body = |text: &str| {
        let file = dir.root.parent().unwrap().join("body.md");
        std::fs::write(&file, text).unwrap();
        format!("body=@{}", file.display())
    };
    dir.ok(&[
        "new",
        "document",
        "--set",
        "title=Setup",
        "--set",
        &body("# Install\n\nRun it.\n"),
    ]);
    dir.ok(&[
        "new",
        "document",
        "--set",
        "title=Guide",
        "--set",
        &body("See [install](kuria:DOC-1#install) and [gone](kuria:DOC-1#gone).\n\n# Install\n"),
    ]);
    let setup = dir.ok(&["cat", "DOC-1"]).markdown;
    assert!(
        setup.contains("<!-- field: body -->\n# Install {#install}\n\nRun it.\n"),
        "{setup}"
    );
    // The text says its links: neither file lists them.
    let guide = dir.ok(&["cat", "DOC-2"]).markdown;
    assert!(!guide.contains("links:"), "{guide}");
    assert!(!setup.contains("links:"), "{setup}");
    // But they are read, from both sides, and the broken reference says why.
    let shown = dir.ok(&["show", "DOC-1", "--format", "json"]).json;
    assert_eq!(
        shown["akt"]["links"][0]["name"], "Referred to by",
        "{shown}"
    );
    assert_eq!(shown["akt"]["links"][0]["akt"]["number"], "DOC-2");
    let read = dir.ok(&["show", "DOC-2", "--format", "json"]).json;
    assert_eq!(
        read["akt"]["references"],
        json!([
            { "field": "body", "destination": "kuria:DOC-1#install", "broken": null },
            { "field": "body", "destination": "kuria:DOC-1#gone", "broken": "`DOC-1` has no heading `gone`" }
        ])
    );
    let said = dir.ok(&["show", "DOC-2"]).markdown;
    assert!(
        said.contains(
            "## Broken references\n\n- Body: `kuria:DOC-1#gone`: `DOC-1` has no heading `gone`\n"
        ),
        "{said}"
    );
    // Its file applied unchanged changes nothing; the text owns the links.
    let file = dir.root.parent().unwrap().join("guide.md");
    std::fs::write(&file, &guide).unwrap();
    assert!(
        dir.ok(&["apply", file.to_str().unwrap()])
            .markdown
            .contains("nothing changed")
    );
    assert_eq!(
        dir.refused(&["link", "DOC-2", "refers_to", "DOC-1"]).code,
        "INVALID_LINK"
    );
    assert_eq!(
        dir.refused(&["set", "DOC-1", &body("# A {#same}\n\n# B {#same}\n")])
            .code,
        "REFUSED"
    );
    let schema = dir.ok(&["types", "schema", "document"]).markdown;
    assert!(
        schema.contains("references kept as `refers_to` links"),
        "{schema}"
    );
    assert_eq!(dir.ok(&["check"]).json["problems"], json!([]));
}

// r[verify cli.local.versions]
// r[verify version.together]
// r[verify version.references]
#[test]
fn documents_referring_to_each_other_are_released_together() {
    let dir = Dir::new();
    documents(&dir);
    let file = dir.root.parent().unwrap().join("body.md");
    let body = |text: &str| {
        std::fs::write(&file, text).unwrap();
        format!("body=@{}", file.display())
    };
    dir.ok(&[
        "new",
        "document",
        "--set",
        "title=Setup",
        "--set",
        &body("# Install\n\n[use it](kuria:DOC-2#use)\n"),
    ]);
    dir.ok(&[
        "new",
        "document",
        "--set",
        "title=Guide",
        "--set",
        &body("# Use\n\n[install](kuria:DOC-1#install)\n"),
    ]);
    dir.commit("Two documents");
    // Neither has a version: alone, each is refused, naming the other.
    let alone = dir.refused(&["release", "DOC-2"]);
    assert_eq!(alone.code, "BROKEN_REFERENCES");
    assert!(
        alone.message.contains("`DOC-1` has no version yet")
            && alone.message.contains("released together with DOC-1"),
        "{}",
        alone.message
    );
    assert_eq!(
        dir.refused(&["release", "DOC-2", "DOC-2"]).code,
        "BAD_REQUEST"
    );
    // Together: each pinned to the other's version 1.
    let released = dir.ok(&["release", "DOC-2", "DOC-1", "--label", "1.0"]);
    let all = released.json["released"].as_array().unwrap();
    assert_eq!(all.len(), 2, "{}", released.markdown);
    assert_eq!(all[0]["number"], "DOC-2");
    assert_eq!(all[1]["version"], 1);
    dir.commit("Release both");
    for (number, other) in [("DOC-1", "DOC-2"), ("DOC-2", "DOC-1")] {
        let versions = dir.ok(&["versions", number]).json;
        assert_eq!(versions["versions"][0]["label"], "1.0", "{versions}");
        let pins = dir.ok(&["show", number, "--version", "1"]).json;
        assert!(pins.to_string().contains(other), "{pins}");
    }
}

// r[verify cli.local.versions]
// r[verify version.release]
// r[verify version.references]
// r[verify version.read]
// r[verify version.diff]
// r[verify version.history]
#[test]
fn documents_are_released_and_read_at_their_versions() {
    let dir = Dir::new();
    documents(&dir);
    let file = dir.root.parent().unwrap().join("body.md");
    let body = |text: &str| {
        std::fs::write(&file, text).unwrap();
        format!("body=@{}", file.display())
    };
    dir.ok(&[
        "new",
        "document",
        "--set",
        "title=Setup",
        "--set",
        &body("# Install\n\nRun it.\n"),
    ]);
    // Not committed yet: refused.
    assert_eq!(dir.refused(&["release", "DOC-1"]).code, "UNCOMMITTED");
    dir.commit("Setup");
    let released = dir.ok(&["release", "DOC-1", "--label", "1.0"]);
    assert_eq!(released.json["version"], 1, "{}", released.markdown);
    dir.commit("Release DOC-1 1.0");
    dir.ok(&[
        "new",
        "document",
        "--set",
        "title=Guide",
        "--set",
        &body("[install](kuria:DOC-1#install) [gone](kuria:DOC-1@1#gone)\n"),
    ]);
    dir.commit("Guide");
    let broken = dir.refused(&["release", "DOC-2"]);
    assert_eq!(broken.code, "BROKEN_REFERENCES");
    assert!(
        broken
            .message
            .contains("`DOC-1` has no heading `gone` in version 1"),
        "{}",
        broken.message
    );
    dir.ok(&["set", "DOC-2", &body("[install](kuria:DOC-1#install)\n")]);
    dir.commit("Fix the guide");
    dir.ok(&["release", "DOC-2"]);
    dir.commit("Release DOC-2");
    // The living version changes on.
    dir.ok(&["set", "DOC-1", &body("# Running {#run}\n")]);
    dir.commit("Rewrite the setup");

    // A file never lists its versions; read at one, it says which.
    let living = dir.ok(&["cat", "DOC-1"]).markdown;
    assert!(!living.contains("versions"), "{living}");
    let at = dir.ok(&["cat", "DOC-1", "--version", "1"]).markdown;
    assert!(
        at.contains("revision: 1\nversion: 1\nstate: draft\n"),
        "{at}"
    );
    assert!(at.contains("# Install {#install}"), "{at}");
    let path = dir.root.parent().unwrap().join("at.md");
    std::fs::write(&path, &at).unwrap();
    assert_eq!(
        dir.refused(&["apply", path.to_str().unwrap()]).code,
        "READ_ONLY"
    );
    // DOC-2's version 1 follows its pin to DOC-1's version 1, where the
    // heading still is; its living version doesn't.
    let pinned = dir
        .ok(&["show", "DOC-2", "--version", "1", "--format", "json"])
        .json;
    assert_eq!(
        pinned["akt"]["references"][0]["broken"],
        json!(null),
        "{pinned}"
    );
    let now = dir.ok(&["show", "DOC-2", "--format", "json"]).json;
    assert_eq!(
        now["akt"]["references"][0]["broken"],
        "`DOC-1` has no heading `install`"
    );

    let listed = dir.ok(&["versions", "DOC-1", "--format", "json"]).json;
    assert_eq!(listed["versions"][0]["number"], 1);
    assert_eq!(listed["versions"][0]["label"], "1.0");
    let diff = dir.ok(&["diff", "DOC-1", "1"]).markdown;
    assert!(
        diff.contains("-# Install {#install}") && diff.contains("+# Running {#run}"),
        "{diff}"
    );
    let history = dir.ok(&["history", "DOC-1"]).markdown;
    assert!(history.contains("released as version 1 (1.0)"), "{history}");
    assert!(
        dir.ok(&["show", "DOC-1"])
            .markdown
            .contains("## Versions\n\n- 1 (1.0) · revision 1")
    );
    assert_eq!(
        dir.refused(&["cat", "DOC-1", "--version", "9"]).code,
        "NOT_FOUND"
    );
    assert_eq!(dir.ok(&["check"]).json["problems"], json!([]));
}

/// A bundle's `revisions` replayed as the server's import replays them,
/// checking each hash is that of what the changes so far result in;
/// answers the fields they end with.
fn replayed(ty: &kuria_config::AktType, revisions: &[Value]) -> serde_json::Map<String, Value> {
    let mut raw = serde_json::Map::new();
    for r in revisions {
        for (k, v) in r["changes"].as_object().unwrap() {
            if v.is_null() {
                raw.remove(k);
            } else {
                raw.insert(k.clone(), v.clone());
            }
        }
        let fields = kuria_engine::fields_from_json(ty, &raw).unwrap();
        let hash = kuria_engine::state_hash(r["state"].as_str().unwrap(), &fields)
            .iter()
            .fold(String::new(), |mut s, b| {
                write!(s, "{b:02x}").unwrap();
                s
            });
        assert_eq!(r["hash"], hash.as_str(), "revision {}", r["revision"]);
    }
    raw
}

// r[verify cli.local.convert]
// r[verify cli.local.versions]
#[test]
fn versions_go_to_a_bundle_and_back() {
    let dir = Dir::new();
    documents(&dir);
    let file = dir.root.parent().unwrap().join("body.md");
    let body = |text: &str| {
        std::fs::write(&file, text).unwrap();
        format!("body=@{}", file.display())
    };
    dir.ok(&[
        "new",
        "document",
        "--set",
        "title=Setup",
        "--set",
        &body("# Install\n"),
    ]);
    dir.commit("Setup");
    dir.ok(&["release", "DOC-1", "--label", "1.0"]);
    dir.commit("Release DOC-1");
    dir.ok(&[
        "new",
        "document",
        "--set",
        "title=Guide",
        "--set",
        &body("[install](kuria:DOC-1#install)\n"),
    ]);
    dir.commit("Guide");
    dir.ok(&["release", "DOC-2"]);
    dir.commit("Release DOC-2");
    dir.ok(&["set", "DOC-1", &body("# Running {#run}\n")]);
    dir.commit("Rewrite the setup");

    let unpacked =
        kuria_cli::local::convert::bundle_of(&dir.root, Some("me@kuria.example".to_owned()))
            .unwrap();
    let bundle = &unpacked.bundle;
    assert_eq!(bundle["format"], 4);
    let akt = |number: &str| {
        bundle["akter"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["number"] == number)
            .unwrap()
            .clone()
    };
    // Each revision is what git holds at it, so a version reads on the
    // server as it was released.
    let setup = akt("DOC-1");
    let revisions = setup["revisions"].as_array().unwrap();
    assert_eq!(revisions.len(), 2, "{setup}");
    assert_eq!(revisions[0]["changes"]["body"], "# Install {#install}");
    assert_eq!(
        revisions[1]["changes"],
        json!({ "body": "# Running {#run}" })
    );
    // As the server's import replays them: each hash is that of what the
    // changes so far result in.
    let doc: kuria_config::AktTypeDocument = serde_json::from_value(
        bundle["aktTypes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["key"] == "document")
            .unwrap()["versions"][0]["definition"]
            .clone(),
    )
    .unwrap();
    let ty = kuria_config::validate(doc).unwrap();
    let raw = replayed(&ty, revisions);
    assert_eq!(json!(raw), setup["fields"]);
    assert_eq!(
        setup["versions"],
        json!([{
            "number": 1, "revision": 1, "label": "1.0",
            "releasedBy": setup["versions"][0]["releasedBy"],
            "releasedAt": setup["versions"][0]["releasedAt"],
            "pins": []
        }])
    );
    let guide = akt("DOC-2");
    assert_eq!(guide["versions"][0]["pins"][0]["version"], 1, "{guide}");

    // And back: the versions are listed, and a pin still finds the heading
    // the version it names had.
    let again = dir.root.parent().unwrap().join("again");
    kuria_cli::local::convert::to_dir(&unpacked, &again).unwrap();
    let back = Dir {
        _temp: tempfile::tempdir().unwrap(),
        root: again.clone(),
    };
    for number in ["DOC-1", "DOC-2"] {
        assert_eq!(
            back.ok(&["versions", number, "--format", "json"]).json,
            dir.ok(&["versions", number, "--format", "json"]).json,
            "{number}"
        );
    }
    let pinned = back.ok(&["show", "DOC-2", "--format", "json"]).json;
    assert_eq!(
        pinned["akt"]["references"][0]["broken"],
        "`DOC-1` has no heading `install`"
    );
    assert_eq!(back.ok(&["check"]).json["problems"], json!([]));
}

/// Images whose pictures are PNG or SVG files, builds holding an SBOM, and
/// documents that embed images, released as versions.
fn pictured(dir: &Dir) {
    let image =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/documents/image.json");
    std::fs::copy(image, dir.root.join("types/image.json")).unwrap();
    let types = [
        json!({
            "key": "build", "name": "Build", "number": "BLD-{seq}",
            "states": [{ "key": "built", "name": "Built", "initial": true }],
            "fields": [
                { "key": "title", "name": "Title", "kind": "text", "required": true },
                { "key": "sbom", "name": "SBOM", "kind": "attachment", "format": "sbom" }
            ]
        }),
        json!({
            "key": "document", "name": "Document", "number": "DOC-{seq}",
            "states": [{ "key": "draft", "name": "Draft", "initial": true }],
            "fields": [
                { "key": "title", "name": "Title", "kind": "text", "required": true },
                { "key": "body", "name": "Body", "kind": "markdown", "references": "refers_to" }
            ],
            "links": [{ "key": "refers_to", "name": "Refers to", "inverse": "Referred to by", "to": ["document", "image"] }],
            "versions": {}
        }),
    ];
    for t in types {
        std::fs::write(
            dir.root
                .join(format!("types/{}.json", t["key"].as_str().unwrap())),
            serde_json::to_string_pretty(&t).unwrap(),
        )
        .unwrap();
    }
}

/// A file beside the directory, as `@<path>` gives it.
fn outside(dir: &Dir, name: &str, bytes: &[u8]) -> String {
    let path = dir.root.parent().unwrap().join(name);
    std::fs::write(&path, bytes).unwrap();
    format!("@{}", path.display())
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n one picture";
const SPDX: &str =
    r#"{ "spdxVersion": "SPDX-2.3", "name": "backend", "packages": [{ "name": "serde" }] }"#;

// r[verify cli.local.attachments]
// r[verify cli.local.unsupported]
// r[verify attachment.media-types]
#[test]
fn attachments_are_files_of_the_directory() {
    let dir = Dir::new();
    pictured(&dir);
    let logo = outside(&dir, "logo.png", PNG);
    dir.ok(&[
        "new",
        "image",
        "--set",
        "title=Logo",
        "--set",
        &format!("file={logo}"),
    ]);
    assert_eq!(
        std::fs::read(dir.root.join("attachments/IMG-1/logo.png")).unwrap(),
        PNG
    );
    assert!(
        dir.ok(&["cat", "IMG-1"])
            .markdown
            .contains("file: attachments/IMG-1/logo.png\n"),
        "named by its path"
    );
    // The same name with other bytes is a file of its own.
    let other = outside(&dir, "logo.png", b"\x89PNG another");
    dir.ok(&["set", "IMG-1", &format!("file={other}")]);
    assert!(file(&dir, "IMG-1").contains("file: attachments/IMG-1/logo-2.png\n"));
    assert_eq!(
        std::fs::read(dir.root.join("attachments/IMG-1/logo.png")).unwrap(),
        PNG,
        "an attachment never changes"
    );
    // A file of the directory, named by its path.
    dir.ok(&["set", "IMG-1", "file=attachments/IMG-1/logo.png"]);

    let sbom = outside(&dir, "backend.spdx.json", SPDX.as_bytes());
    dir.ok(&[
        "new",
        "build",
        "--set",
        "title=4.2",
        "--set",
        &format!("sbom={sbom}"),
    ]);
    assert!(
        dir.root
            .join("attachments/BLD-1/backend.spdx.json")
            .is_file()
    );

    // What the field doesn't take, a file the directory lacks, and an
    // upload's identifier are refused, changing nothing.
    let text = format!("file={}", outside(&dir, "notes.txt", b"not a picture"));
    let not_sbom = format!("sbom={}", outside(&dir, "other.json", b"{}"));
    for (args, code) in [
        (["set", "IMG-1", text.as_str()], "REFUSED"),
        (["set", "BLD-1", not_sbom.as_str()], "REFUSED"),
        (
            ["set", "IMG-1", "file=attachments/IMG-1/gone.png"],
            "REFUSED",
        ),
        (["set", "IMG-1", "file=attachments/logo.png"], "REFUSED"),
        (["set", "IMG-1", "file=../logo.png"], "REFUSED"),
        (
            ["set", "IMG-1", "file=0190f5a4-8f0e-7c2a-9e1b-3a4b5c6d7e8f"],
            "LOCAL_UNSUPPORTED",
        ),
    ] {
        let refused = dir.refused(&args);
        assert_eq!(refused.code, code, "{args:?}: {}", refused.message);
    }
    assert!(!dir.root.join("attachments/IMG-1/notes.txt").exists());
    assert_eq!(dir.ok(&["check"]).json["problems"], json!([]));

    // `check` names a file that went, and one the field doesn't take.
    std::fs::write(dir.root.join("attachments/BLD-1/notes.txt"), "x").unwrap();
    edit_akt(&dir, "BLD-1", "backend.spdx.json", "notes.txt");
    std::fs::remove_file(dir.root.join("attachments/IMG-1/logo.png")).unwrap();
    let refused = dir.refused(&["check"]);
    assert_eq!(refused.code, "INCONSISTENT");
    for problem in [
        "akter/IMG-1-logo.md: `file`: the directory has no file `attachments/IMG-1/logo.png`",
        "akter/BLD-1-4-2.md: `sbom`: `attachments/BLD-1/notes.txt` isn't an SBOM",
    ] {
        assert!(refused.message.contains(problem), "{}", refused.message);
    }
}

// r[verify cli.local.attachments]
// r[verify akt.field.markdown.images]
// r[verify version.references]
#[test]
fn a_document_embedding_a_picture_in_the_directory_releases() {
    let dir = Dir::new();
    pictured(&dir);
    let logo = outside(&dir, "logo.png", PNG);
    dir.ok(&[
        "new",
        "image",
        "--set",
        "title=Logo",
        "--set",
        &format!("file={logo}"),
    ]);
    dir.ok(&["new", "image", "--set", "title=Not yet"]);
    dir.ok(&[
        "new",
        "document",
        "--set",
        "title=Setup",
        "--set",
        "body=![Logo](kuria:IMG-1) ![Later](kuria:IMG-2)",
    ]);
    dir.commit("Setup");
    let refused = dir.refused(&["release", "DOC-1"]);
    assert_eq!(refused.code, "BROKEN_REFERENCES");
    assert!(
        refused.message.contains("`IMG-2` has no picture") && !refused.message.contains("IMG-1"),
        "{}",
        refused.message
    );
    dir.ok(&["set", "DOC-1", "body=![Logo](kuria:IMG-1)"]);
    dir.commit("Only the logo");
    let released = dir.ok(&["release", "DOC-1"]);
    assert_eq!(released.json["version"], 1, "{}", released.markdown);
}

// r[verify cli.local.attachments]
// r[verify cli.local.convert]
#[test]
fn attachments_go_to_a_bundle_and_back() {
    let dir = Dir::new();
    pictured(&dir);
    let logo = outside(&dir, "logo.png", PNG);
    dir.ok(&[
        "new",
        "image",
        "--set",
        "title=Logo",
        "--set",
        &format!("file={logo}"),
    ]);
    let sbom = outside(&dir, "backend.spdx.json", SPDX.as_bytes());
    dir.ok(&[
        "new",
        "build",
        "--set",
        "title=4.2",
        "--set",
        &format!("sbom={sbom}"),
    ]);
    dir.commit("Logo and build");
    // A picture replaced, its old file removed: git still has it.
    let svg = outside(
        &dir,
        "logo.svg",
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    );
    dir.ok(&["set", "IMG-1", &format!("file={svg}")]);
    std::fs::remove_file(dir.root.join("attachments/IMG-1/logo.png")).unwrap();
    dir.commit("An SVG logo");

    let unpacked =
        kuria_cli::local::convert::bundle_of(&dir.root, Some("me@kuria.example".to_owned()))
            .unwrap();
    let bundle = &unpacked.bundle;
    let attachments = bundle["attachments"].as_array().unwrap();
    let named = |name: &str| {
        attachments
            .iter()
            .find(|a| a["name"] == name)
            .unwrap_or_else(|| panic!("no {name} in {attachments:?}"))
            .clone()
    };
    let (png, svg, spdx) = (
        named("logo.png"),
        named("logo.svg"),
        named("backend.spdx.json"),
    );
    assert_eq!(attachments.len(), 3);
    assert_eq!(
        (png["mediaType"].as_str(), svg["mediaType"].as_str()),
        (Some("image/png"), Some("image/svg+xml"))
    );
    assert_eq!(spdx["sbom"]["format"], "spdx");
    assert!(png["sbom"].is_null());
    // As the server's import checks them: each one's bytes by its hash.
    for a in attachments {
        let bytes = &unpacked.attachments[a["sha256"].as_str().unwrap()];
        assert_eq!(a["size"], bytes.len());
    }
    assert_eq!(unpacked.attachments[png["sha256"].as_str().unwrap()], PNG);
    let akt = |number: &str| {
        bundle["akter"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["number"] == number)
            .unwrap()
            .clone()
    };
    let image = akt("IMG-1");
    assert_eq!(
        image["attachments"],
        json!([{ "field": "file", "attachment": svg["id"] }])
    );
    assert_eq!(image["fields"]["file"], svg["id"]);
    let revisions = image["revisions"].as_array().unwrap();
    assert_eq!(
        revisions[0]["changes"]["file"], png["id"],
        "the file it held first"
    );
    let doc: kuria_config::AktTypeDocument =
        serde_json::from_str(&std::fs::read_to_string(dir.root.join("types/image.json")).unwrap())
            .unwrap();
    replayed(&kuria_config::validate(doc).unwrap(), revisions);
    // Another client names other attachments, so a directory imports twice.
    let again = kuria_cli::local::convert::bundle_of(&dir.root, None).unwrap();
    assert_ne!(again.bundle["attachments"][0]["id"], attachments[0]["id"]);
    written_back(&dir, &unpacked);
}

/// `unpacked`, the bundle of `dir`, back as a directory: each file as it
/// was, where it was.
fn written_back(dir: &Dir, unpacked: &kuria_cli::local::convert::Unpacked) {
    let back_root = dir.root.parent().unwrap().join("again");
    let wrote = kuria_cli::local::convert::to_dir(unpacked, &back_root).unwrap();
    assert_eq!(wrote["attachments"], 2, "the files akter hold now");
    let back = Dir {
        _temp: tempfile::tempdir().unwrap(),
        root: back_root.clone(),
    };
    for path in [
        "attachments/IMG-1/logo.svg",
        "attachments/BLD-1/backend.spdx.json",
    ] {
        assert_eq!(
            std::fs::read(back_root.join(path)).unwrap(),
            std::fs::read(dir.root.join(path)).unwrap(),
            "{path}"
        );
    }
    for number in ["IMG-1", "BLD-1"] {
        assert_eq!(
            back.ok(&["cat", number]).markdown,
            dir.ok(&["cat", number]).markdown,
            "{number}"
        );
    }
    assert_eq!(back.ok(&["check"]).json["problems"], json!([]));
}

/// Gives the planning example's type `key` the archive rules `rules`.
fn with_rules(dir: &Dir, key: &str, rules: &Value) {
    let path = dir.root.join(format!("types/{key}.json"));
    let mut document: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    document["archive"] = json!({ "rules": rules });
    std::fs::write(&path, serde_json::to_string_pretty(&document).unwrap()).unwrap();
}

/// `akter/archived.md`: each archived akt, and who or which rule archived
/// it.
fn archived_list(dir: &Dir) -> Vec<(String, String)> {
    kuria_cli::local::archive::Archive::read(&dir.root)
        .unwrap()
        .archived
        .into_iter()
        .map(|e| {
            let by = match (e.rule, e.container) {
                (Some(rule), Some(container)) => format!("{rule} {container}"),
                (Some(rule), None) => rule,
                (None, _) => e.by.unwrap_or_default(),
            };
            (e.number, by)
        })
        .collect()
}

fn sorted(mut numbers: Vec<String>) -> Vec<String> {
    numbers.sort();
    numbers
}

impl Dir {
    /// Commits everything, as if at `at`, by git's dates.
    fn commit_at(&self, message: &str, at: &str) {
        self.git(&["add", "-A"]);
        let ran = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(["commit", "-q", "-m", message])
            .env("GIT_AUTHOR_DATE", at)
            .env("GIT_COMMITTER_DATE", at)
            .output()
            .unwrap();
        assert!(
            ran.status.success(),
            "{}",
            String::from_utf8_lossy(&ran.stderr)
        );
    }
}

// r[verify cli.local.archive.kept]
// r[verify cli.local.archive]
#[allow(
    clippy::too_many_lines,
    reason = "one akt archived and back, told in one place"
)]
// r[verify cli.archive]
#[test]
fn archiving_takes_an_akt_out_of_its_file_and_unarchiving_puts_it_back() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["new", "plan", "--set", "title=Other"]);
    dir.commit("Planned");
    let plan = revision_of(&dir, "PLAN-001");
    let task = revision_of(&dir, "PLAN-001-T01");
    let before = dir.ok(&["cat", "PLAN-001-T01"]).markdown;

    let archived = dir
        .ok(&[
            "archive",
            "PLAN-001",
            "--reason",
            "Done with",
            "--format",
            "json",
        ])
        .json;
    assert_eq!(
        archived,
        json!({ "archived": [{ "number": "PLAN-001", "revision": plan + 1 }], "reason": "Done with" })
    );
    // Out of its file, with what is nested in it; listed, by whom and why.
    assert_eq!(akt_files(&dir), ["PLAN-002-other.md", "archived.md"]);
    assert_eq!(
        archived_list(&dir),
        [
            ("PLAN-001".to_owned(), "me@kuria.example".to_owned()),
            (
                "PLAN-001-T01".to_owned(),
                "with_container PLAN-001".to_owned()
            ),
            (
                "PLAN-001-T02".to_owned(),
                "with_container PLAN-001".to_owned()
            ),
        ]
    );
    let entries = kuria_cli::local::archive::Archive::read(&dir.root).unwrap();
    assert_eq!(entries.archived[0].reason.as_deref(), Some("Done with"));
    assert_eq!(i64::from(entries.archived[0].revision), plan);

    // Lists leave it out unless asked; read by number, it says so.
    assert_eq!(numbers(&dir.ok(&["ls", "--format", "json"])), ["PLAN-002"]);
    assert_eq!(
        sorted(numbers(&dir.ok(&["ls", "--archived", "--format", "json"]))),
        ["PLAN-001", "PLAN-001-T01", "PLAN-001-T02"]
    );
    assert_eq!(
        numbers(&dir.ok(&["ls", "--with-archived", "--format", "json"])).len(),
        4
    );
    assert!(
        dir.ok(&["ls", "plan", "--archived"])
            .markdown
            .contains("Active (archived)")
    );
    assert_eq!(
        numbers(&dir.ok(&["find", "Local", "--format", "json"])),
        Vec::<String>::new()
    );
    assert_eq!(
        numbers(&dir.ok(&["find", "Local", "--archived", "--format", "json"])),
        ["PLAN-001"]
    );
    assert_eq!(
        numbers(&dir.ok(&["ls", "--in", "PLAN-001", "--format", "json"])),
        Vec::<String>::new()
    );
    assert_eq!(
        numbers(&dir.ok(&["ls", "--in", "PLAN-001", "--archived", "--format", "json"])),
        ["PLAN-001-T01", "PLAN-001-T02"]
    );
    let cat = dir.ok(&["cat", "PLAN-001-T01", "--format", "json"]).json;
    assert_eq!(cat["akter"][0]["revision"], task + 1);
    assert_eq!(cat["akter"][0]["archived"]["rule"], "with_container");
    let file = dir.ok(&["cat", "PLAN-001"]).markdown;
    assert!(file.contains("archived: {at: "), "{file}");
    assert!(
        file.contains("by: me@kuria.example, reason: Done with}"),
        "{file}"
    );
    assert_eq!(
        dir.ok(&["cat", "PLAN-001", "--all-members", "--format", "json"])
            .json["akter"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        dir.ok(&[
            "cat",
            "PLAN-001",
            "--all-members",
            "--archived",
            "--format",
            "json"
        ])
        .json["akter"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let shown = dir.ok(&["show", "PLAN-001"]).markdown;
    assert!(shown.contains(" by me@kuria.example: Done with"), "{shown}");
    assert!(
        shown.contains("PLAN-001-T01 · First (Task, To do, archived)"),
        "{shown}"
    );

    // Read-only, and its numbers stay taken.
    let refused = dir.refused(&["set", "PLAN-001-T01", "title=Again"]);
    assert_eq!(refused.code, "ARCHIVED");
    assert!(refused.message.contains("kuria unarchive PLAN-001-T01"));
    assert_eq!(
        dir.refused(&["new", "task", "--in", "PLAN-001", "--set", "title=Late"])
            .code,
        "ARCHIVED"
    );
    assert_eq!(dir.refused(&["archive", "PLAN-001"]).code, "BAD_REQUEST");
    assert_eq!(dir.refused(&["unarchive", "PLAN-002"]).code, "BAD_REQUEST");
    assert_eq!(dir.refused(&["unarchive", "PLAN-009"]).code, "NOT_FOUND");
    assert!(
        dir.ok(&["new", "plan", "--set", "title=Third"])
            .markdown
            .contains("PLAN-003")
    );
    dir.ok(&["check"]);

    // Put back as git holds it, once everything is committed.
    assert_eq!(dir.refused(&["unarchive", "PLAN-001"]).code, "UNCOMMITTED");
    dir.commit("Archived");
    let back = dir.ok(&["unarchive", "PLAN-001", "--format", "json"]).json;
    assert_eq!(back["unarchived"][0]["revision"], plan + 2);
    assert_eq!(
        akt_files(&dir),
        [
            "PLAN-001-local-writes.md",
            "PLAN-002-other.md",
            "PLAN-003-third.md",
            "archived.md"
        ]
    );
    assert_eq!(archived_list(&dir), Vec::new());
    assert_eq!(
        dir.ok(&["cat", "PLAN-001-T01"]).markdown,
        before.replace(
            &format!("revision: {task}\n"),
            &format!("revision: {}\n", task + 2)
        )
    );
    dir.ok(&["set", "PLAN-001-T01", "title=Again"]);
    dir.ok(&["check"]);
}

// r[verify cli.local.archive]
#[test]
fn rules_archive_on_the_cli_s_changes_and_by_check_fix() {
    let dir = Dir::new();
    with_rules(
        &dir,
        "plan",
        &json!([{ "newer": 1, "in": ["done"] }, { "days_in": 30, "in": ["dropped"] }]),
    );
    with_rules(&dir, "task", &json!([{ "with_container": "plan" }]));
    for title in ["One", "Two", "Three", "Four"] {
        dir.ok(&[
            "new",
            "plan",
            "--set",
            &format!("title={title}"),
            "--set",
            "goal=G",
        ]);
    }
    for plan in ["PLAN-001", "PLAN-002", "PLAN-003", "PLAN-004"] {
        dir.ok(&["do", plan, "activate"]);
    }
    dir.ok(&["new", "task", "--in", "PLAN-003", "--set", "title=Shared"]);
    dir.ok(&["members", "add", "PLAN-004", "PLAN-003-T01"]);
    dir.commit_at("Planned", "2026-01-01T00:00:00Z");
    dir.ok(&["do", "PLAN-001", "finish"]);
    dir.commit_at("One done", "2026-01-02T00:00:00Z");

    // A newer plan done overtakes the first.
    dir.ok(&["do", "PLAN-002", "finish"]);
    assert_eq!(
        archived_list(&dir),
        [("PLAN-001".to_owned(), "newer".to_owned())]
    );
    dir.commit("Two done");
    // Unarchived by hand, it is kept from rules.
    dir.ok(&["unarchive", "PLAN-001"]);
    let archive = kuria_cli::local::archive::Archive::read(&dir.root).unwrap();
    assert_eq!(archive.kept, ["PLAN-001"]);
    dir.commit("Kept");

    // A task goes with any plan holding it, out of another's file, and
    // comes back with it.
    dir.ok(&["archive", "PLAN-004"]);
    assert_eq!(
        archived_list(&dir),
        [
            ("PLAN-004".to_owned(), "me@kuria.example".to_owned()),
            (
                "PLAN-003-T01".to_owned(),
                "with_container PLAN-004".to_owned()
            ),
        ]
    );
    assert_eq!(
        numbers(&dir.ok(&["ls", "--in", "PLAN-003", "--format", "json"])),
        Vec::<String>::new()
    );
    dir.ok(&["check"]);
    dir.commit("Four archived");
    dir.ok(&["unarchive", "PLAN-004"]);
    assert_eq!(
        numbers(&dir.ok(&["ls", "--in", "PLAN-003", "--format", "json"])),
        ["PLAN-003-T01"]
    );
    assert_eq!(archived_list(&dir), Vec::new());
    dir.ok(&["check"]);
    dir.commit("Four back");

    // Dropped long ago, by git's commit, `check --fix` archives it, with
    // what is nested in it.
    dir.ok(&["do", "PLAN-003", "drop"]);
    dir.commit_at("Dropped", "2026-01-03T00:00:00Z");
    let fixed = dir.ok(&["check", "--fix", "--format", "json"]).json;
    assert_eq!(fixed["archivedByRules"], 1, "{fixed}");
    assert_eq!(
        archived_list(&dir),
        [
            ("PLAN-003".to_owned(), "days_in".to_owned()),
            (
                "PLAN-003-T01".to_owned(),
                "with_container PLAN-003".to_owned()
            ),
        ]
    );
    // Kept from rules, the first stays though the second is newer.
    assert!(dir.ok(&["cat", "PLAN-001"]).json["akter"][0]["archived"].is_null());
}

// r[verify cli.local.archive.kept]
// r[verify cli.local.archive]
// r[verify cli.local.convert]
#[test]
fn archived_akter_go_to_a_bundle_and_back() {
    let dir = Dir::new();
    worked(&dir);
    dir.ok(&["new", "plan", "--set", "title=Other"]);
    dir.commit("Planned");
    let plan = revision_of(&dir, "PLAN-001");
    dir.ok(&["archive", "PLAN-001", "--reason", "Done with"]);
    dir.commit("Archived");
    let unpacked =
        kuria_cli::local::convert::bundle_of(&dir.root, Some("me@kuria.example".to_owned()))
            .unwrap();
    let akter = unpacked.bundle["akter"].as_array().unwrap();
    assert_eq!(akter.len(), 4);
    let of = |number: &str| akter.iter().find(|a| a["number"] == number).unwrap();
    let archived = of("PLAN-001");
    assert_eq!(archived["archived"]["reason"], "Done with");
    assert!(archived["archived"]["by"]["subject"].is_string());
    let revisions = archived["revisions"].as_array().unwrap();
    assert_eq!(revisions.len(), usize::try_from(plan + 1).unwrap());
    assert_eq!(revisions.last().unwrap()["transition"], "archive");
    assert_eq!(
        of("PLAN-001-T01")["archived"]["by"],
        json!({ "rule": "with_container", "container": archived["id"] })
    );
    assert!(of("PLAN-002")["archived"].is_null());

    // Back as a directory no git history holds: archived akter are kept in
    // `akter/archived/`, read and unarchived from there.
    let again = dir.root.parent().unwrap().join("again");
    let wrote = kuria_cli::local::convert::to_dir(&unpacked, &again).unwrap();
    assert_eq!(
        (wrote["akter"].as_i64(), wrote["archived"].as_i64()),
        (Some(1), Some(3))
    );
    let back = Dir {
        _temp: tempfile::tempdir().unwrap(),
        root: again.clone(),
    };
    assert_eq!(
        akt_files(&back),
        ["PLAN-002-other.md", "archived", "archived.md"]
    );
    assert_eq!(archived_list(&back), archived_list(&dir));
    for number in ["PLAN-001", "PLAN-001-T01", "PLAN-002"] {
        assert_eq!(
            back.ok(&["cat", number]).markdown,
            dir.ok(&["cat", number]).markdown,
            "{number}"
        );
    }
    back.ok(&["check"]);
    back.ok(&["unarchive", "PLAN-001"]);
    assert_eq!(
        akt_files(&back),
        [
            "PLAN-001-local-writes.md",
            "PLAN-002-other.md",
            "archived",
            "archived.md"
        ]
    );
    assert!(
        std::fs::read_dir(again.join("akter/archived"))
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(revision_of(&back, "PLAN-001"), plan + 2);
    back.ok(&["check"]);
}

// r[verify akt.transition.members]
// r[verify akt.transition.links]
// r[verify akt.transition.waits]
// r[verify cli.local.rules]
#[test]
fn a_case_closes_once_what_it_waits_for_is_final() {
    let dir = Dir::new();
    let simple = |key: &str, number: &str| {
        json!({
            "key": key, "name": key, "number": number,
            "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
            "states": [
                { "key": "open", "name": "Open", "initial": true },
                { "key": "done", "name": "Done", "final": true }
            ],
            "transitions": [{ "key": "finish", "name": "Finish", "from": ["open"], "to": "done" }]
        })
    };
    let case = json!({
        "key": "case", "name": "Case", "number": "CASE-{seq}",
        "fields": [{ "key": "title", "name": "Title", "kind": "text", "required": true }],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "closed", "name": "Closed", "final": true }
        ],
        "contains": [{ "type": "action" }],
        "links": [
            { "key": "implemented_by", "name": "Implemented by", "inverse": "Implements", "to": ["job"] },
            { "key": "verified_by", "name": "Verified by", "inverse": "Verifies", "to": ["job"] }
        ],
        "transitions": [
            { "key": "close", "name": "Close", "from": ["open"], "to": "closed",
              "waits_for": { "members": ["action"], "links": ["implemented_by"] },
              "requires_links": ["verified_by"] }
        ]
    });
    for (key, document) in [
        ("case", case),
        ("action", simple("action", "ACT-{seq}")),
        ("job", simple("job", "JOB-{seq}")),
    ] {
        std::fs::write(
            dir.root.join(format!("types/{key}.json")),
            document.to_string(),
        )
        .unwrap();
    }
    let schema = dir.ok(&["types", "schema", "case"]).markdown;
    assert!(
        schema.contains(
            "- `close` Close: open → closed, waits for its `action` members to be final, waits for its `implemented_by` links' akter to be final, needs a `verified_by` link\n"
        ),
        "{schema}"
    );
    dir.ok(&["new", "case", "--set", "title=Late reviews"]);
    dir.ok(&["new", "action", "--in", "CASE-1", "--set", "title=Assign"]);
    dir.ok(&["new", "job", "--set", "title=Rota"]);
    dir.ok(&["new", "job", "--set", "title=Check"]);
    dir.ok(&["link", "CASE-1", "implemented_by", "JOB-1"]);
    let shown = dir.ok(&["show", "CASE-1"]).markdown;
    assert!(
        shown.contains(
            "- `close` Close → Closed, waits for ACT-1 and JOB-1, and a Verified by link"
        ),
        "{shown}"
    );
    let refused = dir.refused(&["do", "CASE-1", "close"]);
    assert_eq!(refused.code, "REFUSED");
    assert_eq!(
        refused.message,
        "`close` waits for ACT-1 and JOB-1, which are not final yet, and a `verified_by` link"
    );
    dir.ok(&["do", "ACT-1", "finish"]);
    dir.ok(&["do", "JOB-1", "finish"]);
    dir.ok(&["link", "CASE-1", "verified_by", "JOB-2"]);
    assert_eq!(
        dir.ok(&["show", "CASE-1", "--format", "json"]).json["akt"]["transitions"][0]["waits"],
        Value::Null
    );
    assert_eq!(
        dir.ok(&["do", "CASE-1", "close", "--format", "json"]).json["state"],
        "closed"
    );
}

// r[verify akt.transition.requires-when]
// r[verify cli.local.rules]
#[test]
fn a_corrective_case_needs_its_root_cause() {
    let dir = Dir::new();
    let case = json!({
        "key": "case", "name": "Case", "number": "CASE-{seq}",
        "fields": [
            { "key": "title", "name": "Title", "kind": "text", "required": true },
            { "key": "kind", "name": "Kind", "kind": "choices", "options": [
                { "key": "corrective", "name": "Corrective" },
                { "key": "preventive", "name": "Preventive" } ] },
            { "key": "root_cause", "name": "Root cause", "kind": "text" }
        ],
        "states": [
            { "key": "open", "name": "Open", "initial": true },
            { "key": "planned", "name": "Planned" }
        ],
        "transitions": [{ "key": "plan", "name": "Plan", "from": ["open"], "to": "planned",
            "requires_when": [{ "field": "root_cause", "when": { "kind": ["corrective"] } }] }]
    });
    std::fs::write(dir.root.join("types/case.json"), case.to_string()).unwrap();
    let schema = dir.ok(&["types", "schema", "case"]).markdown;
    assert!(
        schema.contains(
            "- `plan` Plan: open → planned, requires root_cause when kind is corrective\n"
        ),
        "{schema}"
    );
    dir.ok(&[
        "new",
        "case",
        "--set",
        "title=Bug",
        "--set",
        "kind=corrective",
    ]);
    let refused = dir.refused(&["do", "CASE-1", "plan"]);
    assert_eq!(refused.message, "`plan` requires root_cause");
    assert_eq!(
        dir.ok(&[
            "do",
            "CASE-1",
            "plan",
            "--set",
            "root_cause=No rota",
            "--format",
            "json"
        ])
        .json["state"],
        "planned"
    );
}
