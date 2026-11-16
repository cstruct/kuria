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
//! - [`clone`]: clones, `clone`, `status`, `diff`, `add`, `pull`, `push`
//!   and `fmt` on a container's files.
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
pub mod clone;
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
pub mod versions;
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
    let mut context = Context {
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
    if root.is_none() || is_clone_command(&cli.command) {
        let here = std::env::current_dir().map_err(|e| CliError::unavailable(e.to_string()))?;
        if let Some(command) = for_clone(&cli.command, &here) {
            if root.is_some() {
                return Err(local::unsupported(
                    "Clones",
                    "a clone syncs with a server; leave out `--dir`",
                ));
            }
            if context.target.url.is_none() {
                context.target.url = clone::sync::url_of(&here);
            }
            return in_clone(&context, cli.command, command, &here).await;
        }
    }
    match (cli.command, root) {
        (Command::Guide, _) => Ok(guide()),
        (Command::Init, Some(root)) => {
            local::init(&root)?;
            Ok(Answer {
                markdown: format!("Made the akt directory {}.", root.display()),
                json: serde_json::json!({ "dir": root }),
            })
        }
        (Command::Check { fix }, Some(root)) => local::check::run(&root, fix),
        (Command::Check { .. }, None) => Err(CliError::refused(
            "BAD_REQUEST",
            "`check` checks an akt directory: `kuria check --dir <path>`",
        )),
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

/// What a command does in a clone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InClone {
    Clone,
    Status,
    Diff,
    Add,
    Pull,
    Push,
    Fmt,
}

/// Whether `command` is one only clones take.
fn is_clone_command(command: &Command) -> bool {
    matches!(
        command,
        Command::Clone { .. }
            | Command::Status
            | Command::Pull
            | Command::Push { .. }
            | Command::Fmt
    ) || matches!(command, Command::Add { patch: true, .. })
        || matches!(command, Command::Diff { staged: true, .. })
}

/// What `command`, run in `here`, does in a clone, if it is a clone's:
/// `add` and `diff` are when given `-p` or `--staged`, or run in a clone
/// with paths, or nothing, rather than akter and versions.
fn for_clone(command: &Command, here: &Path) -> Option<InClone> {
    let in_one = || clone::sync::in_clone(here);
    let paths = |args: &[String]| args.iter().all(|a| here.join(a).exists());
    match command {
        Command::Clone { .. } => Some(InClone::Clone),
        Command::Status => Some(InClone::Status),
        Command::Pull => Some(InClone::Pull),
        Command::Push { .. } => Some(InClone::Push),
        Command::Fmt => Some(InClone::Fmt),
        Command::Add { args, patch, .. } => {
            (*patch || in_one() && paths(args)).then_some(InClone::Add)
        }
        Command::Diff { args, staged } => {
            let versions = (2..=3).contains(&args.len())
                && args[1..].iter().all(|a| a.parse::<i64>().is_ok())
                && !here.join(&args[0]).exists();
            (*staged || in_one() && !versions).then_some(InClone::Diff)
        }
        _ => None,
    }
}

/// A clone's command, run in `here` (`clone.*`).
async fn in_clone(
    context: &Context,
    command: Command,
    what: InClone,
    here: &Path,
) -> Result<Answer, CliError> {
    match (what, command) {
        (InClone::Status, _) => clone::work::status(here),
        (InClone::Diff, Command::Diff { args, staged }) => clone::work::diff(here, staged, &args),
        (
            InClone::Add,
            Command::Add {
                args, patch: true, ..
            },
        ) => {
            let stdin = std::io::stdin();
            clone::work::add_patch(here, &args, &mut stdin.lock(), &mut std::io::stderr())
        }
        (InClone::Add, Command::Add { args, .. }) => clone::work::add(here, &args),
        (_, command) => {
            let session = Session::open(context).await?;
            let answer = match command {
                Command::Clone { number, dir } => {
                    clone::sync::clone(&session, &number, dir.as_deref(), here).await
                }
                Command::Pull => clone::sync::pull(&session, here).await,
                Command::Push { message } => clone::sync::push(&session, here, &message).await,
                // r[impl clone.normal]
                _ => clone::fmt::fmt(&session, here).await,
            };
            session.close().await;
            answer
        }
    }
}

/// `add`'s arguments for a container: the container, then its members.
fn container_and_members(args: Vec<String>) -> Result<(String, Vec<String>), CliError> {
    let mut args = args.into_iter();
    match (args.next(), args.as_slice()) {
        (Some(container), [_, ..]) => Ok((container, args.collect())),
        _ => Err(CliError::refused(
            "BAD_REQUEST",
            "Name the container and its members: `kuria add <container> <member>…`",
        )),
    }
}

/// `diff`'s arguments for versions: the akt, and the versions from and to.
fn versions_of(args: &[String]) -> Result<(String, i64, Option<i64>), CliError> {
    let refused = || {
        CliError::refused(
            "BAD_REQUEST",
            "Name the akt and versions: `kuria diff <number> <from> [<to>]`, or run `kuria diff` in a clone",
        )
    };
    match args {
        [number, from, rest @ ..] if rest.len() <= 1 => {
            let from = from.parse().map_err(|_| refused())?;
            let to = rest
                .first()
                .map(|t| t.parse())
                .transpose()
                .map_err(|_| refused())?;
            Ok((number.clone(), from, to))
        }
        _ => Err(refused()),
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
            | Command::Versions { .. }
            | Command::Diff { .. }
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
            outcome,
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
                outcome: outcome.map(|o| o.to_ascii_lowercase()),
                search,
                limit,
                after,
                all,
            };
            reads::ls(source, &listing).await
        }
        Command::Find { text, limit } => reads::find(source, &text.join(" "), limit).await,
        Command::Cat {
            numbers,
            output,
            members,
            all_members,
            version,
        } => {
            let members = match (all_members, members.is_empty()) {
                (true, _) => reads::WithMembers::All,
                (false, false) => reads::WithMembers::Types(&members),
                (false, true) => reads::WithMembers::None,
            };
            reads::cat(source, &numbers, output.as_deref(), members, version).await
        }
        Command::Show {
            number,
            history,
            version,
        } => reads::show(source, &number, history, version).await,
        Command::Versions { number } => crate::versions::list(source, &number).await,
        Command::Diff { args, .. } => {
            let (number, from, to) = versions_of(&args)?;
            crate::versions::diff(source, &number, from, to).await
        }
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
    match command {
        Command::Draft { .. } => Err(local::unsupported(
            "`draft`",
            "drafting looks through a server's akter",
        )),
        // r[impl cli.local.unsupported]
        Command::Pdf { .. } => Err(local::unsupported(
            "`pdf`",
            "Kuria renders PDFs; the client doesn't carry the renderer",
        )),
        Command::Export { .. } | Command::Import { .. } => Err(local::unsupported(
            "Bundles",
            "a directory converts to and from a server with `export --to-dir` and `import --from-dir`, signed in",
        )),
        command => {
            let me = local.me.clone();
            drop(local);
            local_write(root, me, command)
        }
    }
}

/// A command that writes, on the akt directory at `root`, under its lock.
// r[impl cli.local.rules]
// r[impl cli.local.lock]
/// `link`, `unlink`, `verdict` and `copy` in local mode.
fn local_more(w: &mut local::write::Writer, command: Command) -> Result<Answer, CliError> {
    use local::arrange;
    match command {
        Command::Link {
            source,
            kind,
            target,
            pin,
            reason,
        } => arrange::link(w, &source, &kind, &target, pin, reason.as_deref()),
        Command::Unlink {
            source,
            kind,
            target,
            reason,
        } => arrange::unlink(w, &source, &kind, &target, reason.as_deref()),
        Command::Verdict {
            number,
            verdict,
            for_whom,
            reason,
        } => local::edits::verdict(w, &number, &verdict, for_whom.as_deref(), reason.as_deref()),
        Command::Copy {
            number,
            only,
            inside,
            reason,
        } => local::copy::copy(w, &number, &only, &inside, reason.as_deref()),
        _ => unreachable!("only links, verdicts and copies"),
    }
}

/// `verdict` and `copy`, on a server.
async fn protocol(session: &Session, command: Command) -> Result<Answer, CliError> {
    match command {
        Command::Release { number, label } => {
            versions::release(session, &number, label.as_deref()).await
        }
        Command::Pdf {
            number,
            field,
            version,
            output,
        } => versions::pdf(session, &number, field.as_deref(), version, &output).await,
        Command::Verdict {
            number,
            verdict,
            for_whom,
            reason,
        } => {
            writes::verdict(
                session,
                &number,
                &verdict,
                for_whom.as_deref(),
                reason.as_deref(),
            )
            .await
        }
        Command::Copy {
            number,
            only,
            inside,
            reason,
        } => writes::copy(session, &number, &only, &inside, reason.as_deref()).await,
        _ => unreachable!("only verdict and copy"),
    }
}

fn local_write(root: &Path, me: Option<String>, command: Command) -> Result<Answer, CliError> {
    use local::{arrange, edits};
    let mut w = local::write::Writer::open(root, me)?;
    let mut save = true;
    let answer = match command {
        Command::New {
            akt_type,
            inside,
            sets,
            file,
            reason,
        } => edits::create(
            &mut w,
            &akt_type,
            &inside,
            &sets,
            file.as_deref(),
            reason.as_deref(),
        ),
        Command::Apply {
            files,
            dry_run,
            reason,
        } => {
            save = !dry_run;
            edits::apply(&mut w, &files, dry_run, reason.as_deref())
        }
        Command::Set {
            number,
            sets,
            reason,
        } => edits::set(&mut w, &number, &sets, reason.as_deref()),
        Command::Do {
            number,
            transition,
            sets,
            reason,
        } => edits::perform(&mut w, &number, &transition, &sets, reason.as_deref()),
        Command::Add { args, reason, .. } => {
            let (container, members) = container_and_members(args)?;
            arrange::membership(
                &mut w,
                writes::Membership::Add,
                &container,
                &members,
                reason.as_deref(),
            )
        }
        Command::Remove {
            container,
            members,
            reason,
        } => arrange::membership(
            &mut w,
            writes::Membership::Remove,
            &container,
            &members,
            reason.as_deref(),
        ),
        Command::Transfer {
            member,
            from,
            to,
            reason,
        } => arrange::transfer(&mut w, &member, &from, &to, reason.as_deref()),
        Command::Move {
            number,
            before,
            after,
        } => arrange::move_akt(&mut w, &number, before.as_deref(), after.as_deref()),
        command @ (Command::Link { .. }
        | Command::Unlink { .. }
        | Command::Verdict { .. }
        | Command::Copy { .. }) => local_more(&mut w, command),
        Command::Release { number, label } => {
            let times = local::git::Times::of(&w.dir.root);
            local::versions::release(&mut w, &times, &number, label.as_deref())
        }
        Command::Types {
            action: Some(TypesCommand::Define { files, .. }),
        } => {
            let mut documents = Vec::new();
            for file in &files {
                documents.extend(writes::documents(file)?);
            }
            edits::define(&mut w, documents)
        }
        _ => unreachable!("reads and refusals are answered before"),
    }?;
    if save {
        w.save()?;
    }
    Ok(answer)
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
            to_dir: Some(to_dir),
            ..
        } => to_dir_of(session, &types, &to_dir).await,
        Command::Import {
            from_dir: Some(from_dir),
            map_subjects,
            ..
        } => {
            let me = std::env::var("KURIA_ME")
                .ok()
                .filter(|v| !v.is_empty())
                .or_else(|| local::git::email(&from_dir));
            let unpacked = local::convert::bundle_of(&from_dir, me)?;
            let bytes = serde_json::to_vec(&unpacked.bundle).expect("serializable");
            writes::bundles::post(session, &bytes, &unpacked.files(), map_subjects.as_deref()).await
        }
        Command::Export {
            types,
            akter,
            whole_tenant,
            output: Some(output),
            ..
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
            bundle: Some(bundle),
            map_subjects,
            ..
        } => writes::import_bundle(session, &bundle, map_subjects.as_deref()).await,
        command => write(session, command).await,
    }
}

/// `kuria export --types <key>,… --to-dir <dir>` (`cli.local.convert`).
// r[impl cli.local.convert]
async fn to_dir_of(session: &Session, types: &[String], dir: &Path) -> Result<Answer, CliError> {
    writes::bundles::check_new(dir)?;
    let query = format!("types={}&akter=true", types.join(","));
    let files = writes::bundles::fetch(session, &query).await?;
    let bundle = writes::bundles::bundle_json(&files)?;
    let wrote = local::convert::to_dir(&local::convert::Unpacked::of(bundle, &files), dir)?;
    Ok(Answer {
        markdown: format!(
            "Wrote {} akt types, {} akter and {} attachments to the akt directory {}; commit it to start its history.",
            wrote["aktTypes"],
            wrote["akter"],
            wrote["attachments"],
            dir.display()
        ),
        json: wrote,
    })
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
        Command::Add { args, reason, .. } => {
            let (container, members) = container_and_members(args)?;
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
        command @ (Command::Verdict { .. }
        | Command::Copy { .. }
        | Command::Release { .. }
        | Command::Pdf { .. }) => protocol(session, command).await,
        Command::Draft {
            number,
            field,
            reason,
        } => writes::draft(session, &number, &field, reason.as_deref()).await,
        _ => unreachable!("dispatched above"),
    }
}
