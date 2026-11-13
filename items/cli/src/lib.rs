//! `kuria`, Kuria's command-line client ([spec](../../../docs/spec/cli.md)).
//!
//! - [`args`]: the commands and their options.
//! - [`profile`]: profiles and which one a command uses.
//! - [`secret`]: where a session's key and tokens are kept.
//! - [`dpop`] and [`http`]: DPoP proofs on every request.
//! - [`konklav`]: where Kuria signs in, and Konklav's endpoints.
//! - [`session`] and [`graphql`]: the session a command uses, and Kuria's API.
//! - [`account`]: `login`, `logout` and `whoami`.
//! - [`reads`]: `types`, `schema`, `ls`, `find`, `cat`, `show`, `history`,
//!   `users` and `contacts`, with [`model`] (Kuria's answers), [`values`]
//!   (field values as text and in files), [`file`] (the akt file),
//!   [`history`] (revisions as the app says them) and [`md`].
//! - [`writes`]: `new`, `apply`, `set`, `do`, `draft`, `add`, `remove`,
//!   `transfer`, `move`, `link`, `unlink`, and `types export` and `define`.
//! - [`output`] and [`error`]: answers, refusals and exit codes.
#![expect(
    clippy::missing_panics_doc,
    reason = "a library for the `kuria` binary and its tests only; its panics are on system randomness, serializing its own values and poisoned locks"
)]
#![allow(
    clippy::format_push_string,
    reason = "answers are text built a piece at a time; `write!` into a `String` reads worse and can't fail either"
)]

pub mod account;
pub mod args;
pub mod dpop;
pub mod error;
pub mod file;
pub mod graphql;
pub mod history;
pub mod http;
pub mod konklav;
pub mod md;
pub mod model;
pub mod output;
pub mod profile;
pub mod reads;
pub mod secret;
pub mod session;
pub mod values;
pub mod writes;

use std::process::ExitCode;

use args::{Cli, Command, TypesCommand};
use error::CliError;
use output::Answer;
use profile::{Asked, Config, config_dir};
use secret::Store;
use session::{Context, Session};

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
        // Checked offline: no session.
        Command::Types {
            action: Some(TypesCommand::Define { files, check: true }),
        } => writes::define(None, &files, true).await,
        command => {
            let session = Session::open(&context).await?;
            let answer = dispatch(&session, command).await;
            session.close().await;
            answer
        }
    }
}

/// A command that reads or writes, through `session`.
async fn dispatch(session: &Session, command: Command) -> Result<Answer, CliError> {
    match command {
        Command::Types { action: None } => reads::types(session).await,
        Command::Types {
            action: Some(TypesCommand::Export { keys, output }),
        } => writes::export(session, &keys, output.as_deref()).await,
        Command::Types {
            action: Some(TypesCommand::Define { files, check }),
        } => writes::define(Some(session), &files, check).await,
        Command::Schema { key } => reads::schema(session, &key).await,
        Command::Ls {
            akt_type,
            state,
            inside,
            conditions,
            search,
            limit,
            after,
            all,
        } => {
            let listing = reads::Listing {
                akt_type,
                state,
                inside,
                conditions,
                search,
                limit,
                after,
                all,
            };
            reads::ls(session, &listing).await
        }
        Command::Find { text, limit } => reads::find(session, &text.join(" "), limit).await,
        Command::Cat { numbers, output } => reads::cat(session, &numbers, output.as_deref()).await,
        Command::Show { number, history } => reads::show(session, &number, history).await,
        Command::History { number } => reads::history(session, &number).await,
        Command::Users => reads::users(session).await,
        Command::Contacts => reads::contacts(session).await,
        command => write(session, command).await,
    }
}

/// A command that writes, through `session`.
async fn write(session: &Session, command: Command) -> Result<Answer, CliError> {
    match command {
        Command::New {
            akt_type,
            inside,
            sets,
            file,
            reason,
        } => {
            let created = writes::Created {
                akt_type: &akt_type,
                inside: &inside,
                sets: &sets,
                file: file.as_deref(),
                reason: reason.as_deref(),
            };
            writes::create(session, &created).await
        }
        Command::Apply {
            files,
            dry_run,
            reason,
        } => writes::apply(session, &files, dry_run, reason.as_deref()).await,
        Command::Set {
            number,
            sets,
            reason,
        } => writes::set(session, &number, &sets, reason.as_deref()).await,
        Command::Do {
            number,
            transition,
            sets,
            reason,
        } => writes::perform(session, &number, &transition, &sets, reason.as_deref()).await,
        Command::Add {
            container,
            members,
            reason,
        } => {
            writes::membership(
                session,
                writes::Membership::Add,
                &container,
                &members,
                reason.as_deref(),
            )
            .await
        }
        Command::Remove {
            container,
            members,
            reason,
        } => {
            writes::membership(
                session,
                writes::Membership::Remove,
                &container,
                &members,
                reason.as_deref(),
            )
            .await
        }
        Command::Transfer {
            member,
            from,
            to,
            reason,
        } => writes::transfer(session, &member, &from, &to, reason.as_deref()).await,
        Command::Move {
            number,
            before,
            after,
        } => writes::move_akt(session, &number, before.as_deref(), after.as_deref()).await,
        Command::Link {
            source,
            kind,
            target,
            pin,
            reason,
        } => writes::link(session, &source, &kind, &target, pin, reason.as_deref()).await,
        Command::Unlink {
            source,
            kind,
            target,
            reason,
        } => writes::unlink(session, &source, &kind, &target, reason.as_deref()).await,
        Command::Draft {
            number,
            field,
            reason,
        } => writes::draft(session, &number, &field, reason.as_deref()).await,
        _ => unreachable!("dispatched above"),
    }
}
