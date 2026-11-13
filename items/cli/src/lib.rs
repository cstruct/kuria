//! `kuria`, Kuria's command-line client ([spec](../../../docs/spec/cli.md)).
//!
//! - [`args`]: the commands and their options.
//! - [`profile`]: profiles and which one a command uses.
//! - [`secret`]: where a session's key and tokens are kept.
//! - [`dpop`] and [`http`]: DPoP proofs on every request.
//! - [`konklav`]: where Kuria signs in, and Konklav's endpoints.
//! - [`session`] and [`graphql`]: the session a command uses, and Kuria's API.
//! - [`account`]: `login`, `logout` and `whoami`.
//! - [`local`]: local mode, on an akt directory instead of a server.
//! - [`reads`]: `types`, `schema`, `ls`, `find`, `cat`, `show`, `history`,
//!   `users` and `contacts`, with [`model`] (Kuria's answers), [`values`]
//!   (field values as text and in files), [`file`] (the akt file),
//!   [`history`] (revisions as the app says them) and [`md`].
//! - [`writes`]: `new`, `apply`, `set`, `do`, `draft`, `add`, `remove`,
//!   `transfer`, `move`, `link`, `unlink`, `types export` and `define`, and
//!   bundles' `export` and `import`.
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
pub mod local;
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
use reads::Source;
use secret::Store;
use session::{Context, Session};
use std::path::{Path, PathBuf};

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

/// What `cli` answers, or why it was refused, without printing it.
pub async fn answer(cli: Cli) -> Result<Answer, CliError> {
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
    // r[impl cli.local.dir]
    let root = cli
        .dir
        .clone()
        .or_else(|| env("KURIA_DIR").map(PathBuf::from));
    match (cli.command, root) {
        (Command::Guide, _) => Ok(guide()),
        (Command::Init, Some(root)) => {
            local::init(&root)?;
            Ok(Answer {
                markdown: format!("Made the akt directory {}.", root.display()),
                json: serde_json::json!({ "dir": root }),
            })
        }
        (Command::Init, None) => Err(CliError::refused(
            "BAD_REQUEST",
            "Name the directory: `kuria init --dir <path>`",
        )),
        // r[impl cli.local.unsupported]
        (Command::Login { .. } | Command::Logout, Some(_)) => Err(local::unsupported(
            "Signing in",
            "an akt directory has no sign-in; git says who changed what",
        )),
        (Command::Whoami, Some(root)) => whoami_local(&root),
        (
            Command::Types {
                action: Some(TypesCommand::Define { files, check: true }),
            },
            _,
        ) => writes::define(None, &files, true).await,
        (command, Some(root)) => local(&root, command).await,
        (command, None) => server(&context, command).await,
    }
}

async fn server(context: &Context, command: Command) -> Result<Answer, CliError> {
    match command {
        Command::Login { no_browser } => account::login(context, !no_browser).await,
        Command::Logout => account::logout(context).await,
        Command::Whoami => account::whoami(context).await,
        // Checked offline: no session.
        Command::Types {
            action: Some(TypesCommand::Define { files, check: true }),
        } => writes::define(None, &files, true).await,
        command => {
            let session = Session::open(context).await?;
            let answer = dispatch(&session, command).await;
            session.close().await;
            answer
        }
    }
}

/// The guide for agents, which needs no session.
// r[impl cli.guide]
fn guide() -> Answer {
    Answer {
        markdown: GUIDE.to_owned(),
        json: serde_json::json!({ "guide": GUIDE }),
    }
}

/// `kuria guide`'s text.
pub const GUIDE: &str = include_str!("guide.md");

/// A command that reads or writes, through `session`.
/// Whether `command` only reads, so that local mode answers it too.
fn reads_only(command: &Command) -> bool {
    matches!(
        command,
        Command::Types {
            action: None | Some(TypesCommand::Export { .. })
        } | Command::Schema { .. }
            | Command::Ls { .. }
            | Command::Find { .. }
            | Command::Cat { .. }
            | Command::Show { .. }
            | Command::History { .. }
            | Command::Users
            | Command::Contacts
    )
}

/// A command that reads, from a server or an akt directory.
async fn read(source: Source<'_>, command: Command) -> Result<Answer, CliError> {
    match command {
        Command::Types { action: None } => reads::types(source).await,
        Command::Types {
            action: Some(TypesCommand::Export { keys, output }),
        } => writes::export(source, &keys, output.as_deref()).await,
        Command::Schema { key } => reads::schema(source, &key).await,
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
            reads::ls(source, &listing).await
        }
        Command::Find { text, limit } => reads::find(source, &text.join(" "), limit).await,
        Command::Cat { numbers, output } => reads::cat(source, &numbers, output.as_deref()).await,
        Command::Show { number, history } => reads::show(source, &number, history).await,
        Command::History { number } => reads::history(source, &number).await,
        Command::Users => reads::users(source).await,
        Command::Contacts => reads::contacts(source).await,
        _ => unreachable!("only reads come here"),
    }
}

/// A command on the akt directory at `root` (`cli.local.*`).
async fn local(root: &Path, command: Command) -> Result<Answer, CliError> {
    let me = std::env::var("KURIA_ME").ok().filter(|v| !v.is_empty());
    let local = reads::Local::open(root, me)?;
    if reads_only(&command) {
        return read(Source::Local(&local), command).await;
    }
    Err(match command {
        Command::Draft { .. } => {
            local::unsupported("`draft`", "drafting looks through a server's akter")
        }
        Command::Export { .. } | Command::Import { .. } => local::unsupported(
            "Bundles",
            "a directory converts from a server with `export --to-dir` (PLAN-008-T06)",
        ),
        _ => local::unsupported("Writing", "local writes come with PLAN-008-T04"),
    })
}

/// `kuria whoami` in local mode: the directory, and who `@me` is.
fn whoami_local(root: &Path) -> Result<Answer, CliError> {
    let me = std::env::var("KURIA_ME")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| local::git::email(root));
    reads::Local::open(root, me.clone())?;
    Ok(Answer {
        markdown: format!(
            "Working on the akt directory {}, as {}.",
            root.display(),
            me.as_deref()
                .unwrap_or("no one (set KURIA_ME, or git's user.email)")
        ),
        json: serde_json::json!({ "dir": root, "me": me, "signedInWith": "local" }),
    })
}

async fn dispatch(session: &Session, command: Command) -> Result<Answer, CliError> {
    if reads_only(&command) {
        return read(Source::Server(session), command).await;
    }
    match command {
        Command::Types {
            action: Some(TypesCommand::Define { files, check }),
        } => writes::define(Some(session), &files, check).await,
        Command::Export {
            types,
            akter,
            whole_tenant,
            output,
        } => {
            let what = if whole_tenant {
                writes::Exported::Tenant
            } else {
                writes::Exported::Types {
                    keys: &types,
                    akter,
                }
            };
            writes::export_bundle(session, &what, &output).await
        }
        Command::Import {
            bundle,
            map_subjects,
        } => writes::import_bundle(session, &bundle, map_subjects.as_deref()).await,
        command => write(session, command).await,
    }
}

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
