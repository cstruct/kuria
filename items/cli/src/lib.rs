//! `kuria`, Kuria's command-line client ([spec](../../../docs/spec/cli.md)).
//!
//! - [`args`]: the commands and their options.
//! - [`profile`]: profiles and which one a command uses.
//! - [`secret`]: where a session's key and tokens are kept.
//! - [`dpop`] and [`http`]: DPoP proofs on every request.
//! - [`konklav`]: where Kuria signs in, and Konklav's endpoints.
//! - [`session`] and [`graphql`]: the session a command uses, and Kuria's API.
//! - [`account`]: `login`, `logout` and `whoami`.
//! - [`output`] and [`error`]: answers, refusals and exit codes.
#![expect(
    clippy::missing_panics_doc,
    reason = "a library for the `kuria` binary and its tests only; its panics are on system randomness, serializing its own values and poisoned locks"
)]

pub mod account;
pub mod args;
pub mod dpop;
pub mod error;
pub mod graphql;
pub mod http;
pub mod konklav;
pub mod output;
pub mod profile;
pub mod secret;
pub mod session;

use std::process::ExitCode;

use args::{Cli, Command};
use error::CliError;
use output::Answer;
use profile::{Asked, Config, config_dir};
use secret::Store;
use session::Context;

/// Runs `cli`, saying its answer or refusal, and answers the exit code.
pub async fn run(cli: Cli) -> ExitCode {
    let format = cli.format;
    match answer(cli).await {
        Ok(answer) => {
            answer.print(format);
            ExitCode::SUCCESS
        }
        Err(error) => {
            output::refusal(&error, format);
            error.exit_code()
        }
    }
}

async fn answer(cli: Cli) -> Result<Answer, CliError> {
    let dir = config_dir()?;
    let config = Config::load(&dir)?;
    let env = |var: &str| std::env::var(var).ok().filter(|v| !v.is_empty());
    let env_tenant = env("KURIA_TENANT")
        .map(|t| {
            t.parse()
                .map_err(|_| CliError::refused("BAD_TENANT", "KURIA_TENANT isn't a tenant id"))
        })
        .transpose()?;
    let target = Asked {
        profile: cli.profile,
        url: cli.url,
        tenant: cli.tenant,
        env_url: env("KURIA_URL"),
        env_tenant,
    }
    .resolve(&config);
    let context = Context {
        target,
        store: Store::new(&dir),
        dir,
        secret_key: env("KURIA_KEY"),
    };
    match cli.command {
        Command::Login { no_browser } => account::login(&context, !no_browser).await,
        Command::Logout => account::logout(&context).await,
        Command::Whoami => account::whoami(&context).await,
    }
}
