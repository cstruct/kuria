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
        &["new", "task", "--in", "PLAN-001"],
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
            "number: PLAN-009\ntype: plan\nrevision: 1\nrank: \"1\"\nstate: active\ntitle: X\n",
            "its number isn't its name",
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
