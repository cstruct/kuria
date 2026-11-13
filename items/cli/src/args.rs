//! The commands and the options every command takes.

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
    /// The akt types, with how many akter each has.
    Types,
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
        output: Option<std::path::PathBuf>,
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
