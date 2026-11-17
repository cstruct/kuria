//! The commands and the options every command takes.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use uuid::Uuid;

use crate::output::Format;

/// Kuria's command-line client, for people and agents.
///
/// Signs in with `kuria login` (a link authorized in a browser), or as
/// software with a Konklav secret key in `KURIA_KEY`. Answers in Markdown,
/// or in JSON with `--format json`.
#[derive(Debug, Parser)]
#[command(name = "kuria", version)]
pub struct Cli {
    /// How to answer.
    #[arg(long, global = true, value_enum, default_value_t = Format::Md)]
    pub format: Format,
    /// Kuria's URL; else `KURIA_URL`, else the profile's.
    #[arg(long, global = true)]
    pub url: Option<String>,
    /// The tenant; else `KURIA_TENANT`, else the profile's.
    #[arg(long, global = true)]
    pub tenant: Option<Uuid>,
    /// The profile to sign in with; else the default one.
    #[arg(long, global = true)]
    pub profile: Option<String>,
    /// An akt directory to work on instead of a server (local mode); else
    /// `KURIA_DIR`.
    #[arg(long, global = true, value_name = "DIR")]
    pub dir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Sign in by a link, authorized in a browser on any machine.
    Login {
        /// Only print the link; don't open a browser.
        #[arg(long)]
        no_browser: bool,
    },
    /// End the session at Konklav and forget its key and tokens.
    Logout,
    /// Who the client is signed in as, and to which Kuria and tenant.
    Whoami,
    /// The akt types, with how many akter each has; or their documents.
    Types {
        #[command(subcommand)]
        action: Option<TypesCommand>,
    },
    /// Akter as a table, most recently changed first, a page at a time.
    Ls {
        /// Only this akt type's akter, with its list fields as columns.
        #[arg(value_name = "TYPE")]
        akt_type: Option<String>,
        /// Only those in this state, by key or name.
        #[arg(long)]
        state: Option<String>,
        /// Only the members of this container.
        #[arg(long = "in", value_name = "CONTAINER")]
        inside: Option<String>,
        /// Only those whose field holds this value, as `<field>=<value>`;
        /// `@me` for yourself. Needs TYPE. May be given more than once.
        #[arg(long = "where", value_name = "FIELD=VALUE")]
        conditions: Vec<String>,
        /// Only those with this outcome. Needs TYPE, with a verdict field,
        /// or `--in` for a type whose outcome comes from its members.
        #[arg(long, value_parser = ["pass", "fail", "skip", "pending"], ignore_case = true)]
        outcome: Option<String>,
        /// Only those whose number starts with this, or whose title holds
        /// each of its words.
        #[arg(long)]
        search: Option<String>,
        /// How many to a page (at most 200).
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// The page after this cursor, from the previous page.
        #[arg(long)]
        after: Option<String>,
        /// Every page.
        #[arg(long, conflicts_with = "after")]
        all: bool,
        /// Only archived akter.
        #[arg(long, conflicts_with = "with_archived")]
        archived: bool,
        /// Archived akter as well as live ones.
        #[arg(long)]
        with_archived: bool,
    },
    /// Akter whose number starts with TEXT, or whose title holds its words.
    Find {
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Only archived akter.
        #[arg(long, conflicts_with = "with_archived")]
        archived: bool,
        /// Archived akter as well as live ones.
        #[arg(long)]
        with_archived: bool,
    },
    /// Each akt as an akt file: Markdown with YAML front matter.
    Cat {
        #[arg(required = true, value_name = "NUMBER")]
        numbers: Vec<String>,
        /// Write each to `<DIR>/<NUMBER>.md` instead.
        #[arg(long, short, value_name = "DIR")]
        output: Option<PathBuf>,
        /// Each followed by its direct members of these akt types, in the
        /// container's order.
        #[arg(
            long,
            value_delimiter = ',',
            value_name = "TYPE,…",
            conflicts_with = "all_members"
        )]
        members: Vec<String>,
        /// Each followed by all its direct members, in the container's
        /// order.
        #[arg(long)]
        all_members: bool,
        /// The akt as it was at this version of it.
        #[arg(long)]
        version: Option<i64>,
        /// Only its archived members.
        #[arg(long, conflicts_with = "with_archived")]
        archived: bool,
        /// Its archived members as well as its live ones.
        #[arg(long)]
        with_archived: bool,
    },
    /// Everything about one akt: fields, containers, members, links and the
    /// transitions its state allows.
    Show {
        number: String,
        /// Its revisions too.
        #[arg(long)]
        history: bool,
        /// The akt as it was at this version of it.
        #[arg(long)]
        version: Option<i64>,
        /// Only its archived members.
        #[arg(long, conflicts_with = "with_archived")]
        archived: bool,
        /// Its archived members as well as its live ones.
        #[arg(long)]
        with_archived: bool,
    },
    /// An akt's versions, latest first.
    Versions { number: String },
    /// A `markdown` field as a PDF: of the living version, or a version.
    Pdf {
        number: String,
        /// The field, if its type has more than one `markdown` field.
        #[arg(long)]
        field: Option<String>,
        /// The version to render.
        #[arg(long)]
        version: Option<i64>,
        /// Where to write it.
        #[arg(long, short)]
        output: PathBuf,
    },
    /// What changed in an akt between two versions, or since a version:
    /// `kuria diff <number> <from> [<to>]`.
    Diff {
        number: String,
        /// The version to compare from.
        from: i64,
        /// The version to compare to; the living version if left out.
        to: Option<i64>,
    },
    /// An akt's revisions: who changed what, and when.
    History { number: String },
    /// The users values can name.
    Users,
    /// The contacts values can name.
    Contacts,
    /// Create an akt, answering its number.
    New {
        #[arg(value_name = "TYPE")]
        akt_type: String,
        /// A container to create it in, by number; the first is its parent.
        #[arg(long = "in", value_name = "CONTAINER")]
        inside: Vec<String>,
        /// A field's value, as `<field>=<value>`; `@<file>` or `@-` for a
        /// multiline text.
        #[arg(long = "set", value_name = "FIELD=VALUE")]
        sets: Vec<String>,
        /// Its fields from an akt file, as `kuria cat` writes one.
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Edit each akt with the fields its file changed.
    Apply {
        #[arg(required = true, value_name = "FILE")]
        files: Vec<PathBuf>,
        /// Say what would change, and change nothing.
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Edit an akt's fields without a file.
    Set {
        number: String,
        #[arg(required = true, value_name = "FIELD=VALUE")]
        sets: Vec<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Perform a transition, by key or name, with the fields it requires.
    Do {
        number: String,
        transition: String,
        #[arg(long = "set", value_name = "FIELD=VALUE")]
        sets: Vec<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// A container's members: add, remove, transfer between containers,
    /// or place on its boards.
    Members {
        #[command(subcommand)]
        action: MembersCommand,
    },
    /// Link an akt to another by a kind its type declares, such as
    /// `ITEM-1 blocks ITEM-2`.
    Link {
        source: String,
        kind: String,
        target: String,
        /// Pin the target as it is now.
        #[arg(long)]
        pin: bool,
        #[arg(long)]
        reason: Option<String>,
    },
    /// End a link.
    Unlink {
        source: String,
        kind: String,
        target: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Give one's verdict of an akt, or one for another assignee with
    /// `--for`; `clear` takes it back.
    Verdict {
        number: String,
        /// `pass`, `fail`, `skip` or `clear`.
        verdict: String,
        /// The assignee to give it for, by email, name or `@me`.
        #[arg(long = "for", value_name = "USER")]
        for_whom: Option<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// A container's documents as files on this machine: a clone, worked
    /// on with any editor and synced with Kuria.
    Docs {
        #[command(subcommand)]
        action: DocsCommand,
    },
    /// Release an akt's current revision as its next version, once none
    /// of its references is broken; several, together, all or nothing, a
    /// reference from one to another read as what that one is released as.
    Release {
        #[arg(required = true, value_name = "NUMBER")]
        numbers: Vec<String>,
        /// A label beside the version's number, such as `2.0`.
        #[arg(long)]
        label: Option<String>,
        /// What the version is, such as what changed: text of at most
        /// 2,000 characters, `@<file>` or `@-` to read it.
        #[arg(long)]
        description: Option<String>,
        /// The PDF the version is released with, for an akt type whose
        /// versions carry one, such as an external document.
        #[arg(long, value_name = "PDF")]
        file: Option<std::path::PathBuf>,
    },
    /// Copy an akt and its members, or the part `--only` names, as new
    /// akter of the same types.
    Copy {
        number: String,
        /// Keep only these akter of its tree, with their members and those
        /// between them and it.
        #[arg(long, value_name = "NUMBER")]
        only: Vec<String>,
        /// The containers to put the copy in; else those of the akt.
        #[arg(long = "in", value_name = "CONTAINER")]
        inside: Vec<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Archive each akt, in order, stopping at the first refused: kept,
    /// read-only, and out of lists unless asked for.
    Archive {
        #[arg(required = true, value_name = "NUMBER")]
        numbers: Vec<String>,
        /// Why, on one line.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Make each archived akt live again, in order, stopping at the first
    /// refused.
    Unarchive {
        #[arg(required = true, value_name = "NUMBER")]
        numbers: Vec<String>,
        /// Why, on one line.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Fill a field from its recipe.
    Draft {
        number: String,
        field: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// A guide for agents: the commands, the akt file and the exit codes.
    Guide,
    /// Make an akt directory for local mode: `kuria init --dir <path>`.
    Init,
    /// Check a whole akt directory as a server would hold it, naming each
    /// problem's file: `kuria check --dir <path>`.
    Check {
        /// First move every akt into the file it belongs in, and name each
        /// file by its first akt, as writes do.
        #[arg(long)]
        fix: bool,
    },
    /// Export a bundle: selected akt types, with or without their akter,
    /// or the whole tenant, into a new directory. With `--to-dir`, the
    /// selected types and their akter as a new akt directory instead.
    #[command(group(clap::ArgGroup::new("what").required(true).args(["types", "whole_tenant"])))]
    #[command(group(clap::ArgGroup::new("into").required(true).args(["output", "to_dir"])))]
    Export {
        /// The akt types, comma-separated.
        #[arg(long, value_delimiter = ',', value_name = "KEY,…")]
        types: Vec<String>,
        /// With the selected types' akter, their history and what they name.
        #[arg(long, requires = "types")]
        akter: bool,
        /// Everything Kuria holds for the tenant.
        #[arg(long)]
        whole_tenant: bool,
        /// The directory to write the bundle to.
        #[arg(long, short, value_name = "DIR")]
        output: Option<PathBuf>,
        /// The new akt directory to write the selected types and their
        /// akter to, for local mode.
        #[arg(long, value_name = "DIR", conflicts_with = "whole_tenant")]
        to_dir: Option<PathBuf>,
    },
    /// Import a bundle into the tenant, keeping its akter's numbers and
    /// history. With `--from-dir`, an akt directory's types and akter.
    #[command(group(clap::ArgGroup::new("from").required(true).args(["bundle", "from_dir"])))]
    Import {
        /// The bundle's directory, as `export` wrote it. (Not `dir`, which
        /// names the global `--dir` of local mode.)
        #[arg(value_name = "DIR")]
        bundle: Option<PathBuf>,
        /// The akt directory whose types and akter to import, keeping
        /// their numbers.
        #[arg(long, value_name = "DIR")]
        from_dir: Option<PathBuf>,
        /// A JSON object mapping the bundle's subjects to this tenant's.
        #[arg(long, value_name = "FILE")]
        map_subjects: Option<PathBuf>,
    },
}

/// `kuria docs`: a clone's commands (`clone.*`).
#[derive(Debug, Subcommand)]
pub enum DocsCommand {
    /// Clone a container and its members as files, recording the base
    /// they were cloned from in `<dir>/.kuria/`.
    Clone {
        number: String,
        /// Where to write it; a new directory named as its file is, if
        /// left out. (Not `dir`, which would be the global `--dir`.)
        #[arg(value_name = "DIR")]
        into: Option<PathBuf>,
    },
    /// The files staged, changed, removed, and holding conflict markers.
    Status,
    /// The working files against what is staged, or with `--staged` what
    /// is staged against the base.
    Diff {
        /// The files to compare; every one if none.
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
        /// What is staged against the base.
        #[arg(long)]
        staged: bool,
    },
    /// Stage files to push, a directory for its changed files, or with
    /// `-p` the hunks chosen.
    Add {
        /// The files to stage.
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
        /// Offer each changed hunk in turn, and stage those chosen.
        #[arg(short = 'p', long = "patch")]
        patch: bool,
    },
    /// Kuria's changes merged into the files, conflicts marked.
    Pull,
    /// What is staged, sent to Kuria as one push.
    Push {
        /// What the push changes, on one line.
        #[arg(short, long)]
        message: String,
    },
    /// The files as Kuria writes them, each where its container's sections
    /// put it, with the pictures of the images they embed.
    Fmt,
}

#[derive(Debug, Subcommand)]
pub enum MembersCommand {
    /// Add members to a container.
    Add {
        container: String,
        #[arg(required = true, value_name = "MEMBER")]
        members: Vec<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Remove members from a container.
    Remove {
        container: String,
        #[arg(required = true, value_name = "MEMBER")]
        members: Vec<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Move a member from one container to another, as one change.
    Transfer {
        member: String,
        from: String,
        to: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Place an akt directly before or after another, on every board.
    #[command(group(clap::ArgGroup::new("place").required(true).args(["before", "after"])))]
    Place {
        number: String,
        #[arg(long, value_name = "NUMBER")]
        before: Option<String>,
        #[arg(long, value_name = "NUMBER")]
        after: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum TypesCommand {
    /// An akt type's fields, states, transitions, what it contains and
    /// links to, its board and presentation.
    Schema {
        /// The akt type's key.
        #[arg(value_name = "TYPE")]
        key: String,
    },
    /// The documents the akt types were defined by, as JSON.
    Export {
        /// The akt types; all of them if none.
        #[arg(value_name = "KEY")]
        keys: Vec<String>,
        /// Write each to `<DIR>/<KEY>.json` instead.
        #[arg(long, short, value_name = "DIR")]
        output: Option<PathBuf>,
    },
    /// Define each document's akt type, as its next version.
    Define {
        #[arg(required = true, value_name = "FILE")]
        files: Vec<PathBuf>,
        /// Only check the documents, offline; define nothing.
        #[arg(long)]
        check: bool,
    },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory as _;

    use super::*;

    #[test]
    fn the_commands_are_well_formed() {
        Cli::command().debug_assert();
        let cli = Cli::try_parse_from(["kuria", "whoami", "--format", "json"]).unwrap();
        assert_eq!(cli.format, Format::Json);
        assert!(Cli::try_parse_from(["kuria", "whoami", "--tenant", "shop"]).is_err());
    }

    // r[verify cli.guide]
    #[test]
    fn the_guide_names_every_command() {
        for command in Cli::command().get_subcommands() {
            let name = command.get_name();
            assert!(
                crate::GUIDE.contains(&format!("kuria {name}")),
                "the guide doesn't mention `kuria {name}`"
            );
        }
        for code in 0..=5 {
            assert!(
                crate::GUIDE.contains(&format!("| {code} |")),
                "exit code {code}"
            );
        }
    }

    #[test]
    fn a_clone_s_commands_are_under_docs() {
        let docs = |args: &[&str]| match Cli::try_parse_from(args).unwrap().command {
            Command::Docs { action } => action,
            command => panic!("not a docs command: {command:?}"),
        };
        assert!(matches!(
            docs(&["kuria", "docs", "add", "-p"]),
            DocsCommand::Add { patch: true, paths } if paths.is_empty()
        ));
        assert!(matches!(
            docs(&["kuria", "docs", "add", "a.md", "b.md"]),
            DocsCommand::Add { patch: false, paths } if paths == ["a.md", "b.md"]
        ));
        assert!(matches!(
            docs(&["kuria", "docs", "diff", "--staged", "a.md"]),
            DocsCommand::Diff { staged: true, paths } if paths == ["a.md"]
        ));
        assert!(matches!(
            docs(&["kuria", "docs", "push", "-m", "Why"]),
            DocsCommand::Push { message } if message == "Why"
        ));
        assert!(matches!(
            docs(&["kuria", "docs", "status"]),
            DocsCommand::Status
        ));
        assert!(matches!(
            docs(&["kuria", "docs", "pull"]),
            DocsCommand::Pull
        ));
        assert!(matches!(docs(&["kuria", "docs", "fmt"]), DocsCommand::Fmt));
        assert!(Cli::try_parse_from(["kuria", "docs", "push"]).is_err());
        for gone in ["clone", "status", "pull", "push", "fmt"] {
            assert!(Cli::try_parse_from(["kuria", gone]).is_err(), "{gone}");
        }
    }

    #[test]
    fn members_and_schema_are_grouped_and_diff_takes_versions() {
        let parsed = |args: &[&str]| Cli::try_parse_from(args).unwrap().command;
        assert!(matches!(
            parsed(&["kuria", "members", "add", "D-1", "D-2", "--reason", "Why"]),
            Command::Members { action: MembersCommand::Add { container, members, reason: Some(_) } }
                if container == "D-1" && members == ["D-2"]
        ));
        assert!(matches!(
            parsed(&["kuria", "members", "remove", "D-1", "D-2"]),
            Command::Members {
                action: MembersCommand::Remove { .. }
            }
        ));
        assert!(matches!(
            parsed(&["kuria", "members", "transfer", "D-2", "D-1", "D-3"]),
            Command::Members { action: MembersCommand::Transfer { member, .. } } if member == "D-2"
        ));
        assert!(matches!(
            parsed(&["kuria", "members", "place", "D-2", "--before", "D-3"]),
            Command::Members {
                action: MembersCommand::Place {
                    before: Some(_),
                    after: None,
                    ..
                }
            }
        ));
        assert!(Cli::try_parse_from(["kuria", "members", "add", "D-1"]).is_err());
        assert!(Cli::try_parse_from(["kuria", "members", "place", "D-2"]).is_err());
        assert!(matches!(
            parsed(&["kuria", "types", "schema", "task"]),
            Command::Types { action: Some(TypesCommand::Schema { key }) } if key == "task"
        ));
        for gone in ["add", "remove", "transfer", "move", "schema"] {
            assert!(
                Cli::try_parse_from(["kuria", gone, "D-1", "D-2"]).is_err(),
                "{gone}"
            );
        }
        assert!(matches!(
            parsed(&["kuria", "diff", "D-1", "1", "2"]),
            Command::Diff { number, from: 1, to: Some(2) } if number == "D-1"
        ));
        assert!(Cli::try_parse_from(["kuria", "diff", "--staged"]).is_err());
        assert!(Cli::try_parse_from(["kuria", "diff", "D-1", "one"]).is_err());
    }

    #[test]
    fn a_clone_s_directory_is_not_the_global_dir() {
        let cli = Cli::try_parse_from(["kuria", "docs", "clone", "DOCS-1", "handbook"]).unwrap();
        assert!(cli.dir.is_none());
        assert!(matches!(
            cli.command,
            Command::Docs { action: DocsCommand::Clone { number, into: Some(into) } } if number == "DOCS-1" && into == std::path::Path::new("handbook")
        ));
    }

    #[test]
    fn import_s_directory_isn_t_local_mode_s() {
        let cli = Cli::try_parse_from(["kuria", "import", "bundle"]).unwrap();
        assert!(cli.dir.is_none());
        assert!(
            matches!(cli.command, Command::Import { bundle: Some(bundle), .. } if bundle == std::path::Path::new("bundle"))
        );
    }
}
