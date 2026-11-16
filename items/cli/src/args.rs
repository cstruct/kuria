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
    /// An akt type's fields, states, transitions, what it contains and
    /// links to, its board and presentation.
    Schema {
        /// The akt type's key.
        #[arg(value_name = "TYPE")]
        key: String,
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
    },
    /// Akter whose number starts with TEXT, or whose title holds its words.
    Find {
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
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
    /// `kuria diff <number> <from> [<to>]`. In a clone, the working files
    /// against what is staged, or with `--staged` what is staged against
    /// the base: `kuria diff [--staged] [<path>…]`.
    Diff {
        /// The akt, the version to compare from, and the version to
        /// compare to, the living version if left out; in a clone, the
        /// files to compare, every one if none.
        #[arg(value_name = "NUMBER FROM [TO] | PATH")]
        args: Vec<String>,
        /// In a clone: what is staged against the base.
        #[arg(long)]
        staged: bool,
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
    /// Add members to a container: `kuria add <container> <member>…`. In a
    /// clone, stage files to push: `kuria add <path>…`, or with `-p` the
    /// hunks chosen.
    Add {
        /// The container, then its new members; in a clone, the files to
        /// stage, a directory for its changed files.
        #[arg(value_name = "CONTAINER MEMBER | PATH")]
        args: Vec<String>,
        /// In a clone: offer each changed hunk in turn, and stage those
        /// chosen.
        #[arg(short = 'p', long = "patch")]
        patch: bool,
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
    Move {
        number: String,
        #[arg(long, value_name = "NUMBER")]
        before: Option<String>,
        #[arg(long, value_name = "NUMBER")]
        after: Option<String>,
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
    /// Clone a container and its members as files, recording the base
    /// they were cloned from in `<dir>/.kuria/`.
    Clone {
        number: String,
        /// Where to write it; a new directory named as its file is, if
        /// left out. (Not `dir`, which would be the global `--dir`.)
        #[arg(value_name = "DIR")]
        into: Option<PathBuf>,
    },
    /// In a clone: the files staged, changed, removed, and holding
    /// conflict markers.
    Status,
    /// In a clone: Kuria's changes merged into the files, conflicts
    /// marked.
    Pull,
    /// In a clone: what is staged, sent to Kuria as one push.
    Push {
        /// What the push changes, on one line.
        #[arg(short, long)]
        message: String,
    },
    /// In a clone: its files as Kuria writes them, each where its
    /// container's sections put it, with the pictures of the images they
    /// embed.
    Fmt,
    /// Release an akt's current revision as its next version, once none
    /// of its references is broken.
    Release {
        number: String,
        /// A label beside the version's number, such as `2.0`.
        #[arg(long)]
        label: Option<String>,
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

#[derive(Debug, Subcommand)]
pub enum TypesCommand {
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
    fn add_and_diff_take_a_clone_s_paths_too() {
        let parsed = |args: &[&str]| Cli::try_parse_from(args).unwrap().command;
        assert!(matches!(
            parsed(&["kuria", "add", "-p"]),
            Command::Add { patch: true, args, .. } if args.is_empty()
        ));
        assert!(matches!(
            parsed(&["kuria", "add", "D-1", "D-2", "--reason", "Why"]),
            Command::Add { patch: false, args, reason: Some(_) } if args == ["D-1", "D-2"]
        ));
        assert!(matches!(
            parsed(&["kuria", "diff", "--staged", "a.md"]),
            Command::Diff { staged: true, args } if args == ["a.md"]
        ));
        assert!(matches!(
            parsed(&["kuria", "push", "-m", "Why"]),
            Command::Push { message } if message == "Why"
        ));
        assert!(Cli::try_parse_from(["kuria", "push"]).is_err());
    }

    #[test]
    fn a_clone_s_directory_is_not_the_global_dir() {
        let cli = Cli::try_parse_from(["kuria", "clone", "DOCS-1", "handbook"]).unwrap();
        assert!(cli.dir.is_none());
        assert!(matches!(
            cli.command,
            Command::Clone { number, into: Some(into) } if number == "DOCS-1" && into == std::path::Path::new("handbook")
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
