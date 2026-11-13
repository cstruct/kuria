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
    },
    /// Everything about one akt: fields, containers, members, links and the
    /// transitions its state allows.
    Show {
        number: String,
        /// Its revisions too.
        #[arg(long)]
        history: bool,
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
    /// Fill a field from its recipe.
    Draft {
        number: String,
        field: String,
        #[arg(long)]
        reason: Option<String>,
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
}
