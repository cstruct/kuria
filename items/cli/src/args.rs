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
