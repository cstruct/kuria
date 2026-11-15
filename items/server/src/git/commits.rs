//! Kuria's own changes as commits on `main` (`git.repo.commits`).
//!
//! What `main` holds of each akt is its mark (`git_mark`): the revision,
//! and the live draft's update, its file there is as of. A pass over a
//! repository compares each akt with its mark and, if any change is due,
//! builds `main`'s next tree from Kuria as it is (the helper's own code, its
//! GraphQL answered in-process as Kuria), commits it on `main` by whoever
//! made the changes, and moves `main`, the marks and the new versions' tags
//! in one transaction. A change is due:
//!
//! - an akt's new revision, or the container's members changing, at once;
//! - a live draft's changes once it has been quiet for [`Timing::quiet`];
//! - a release at once, as the version it names, which is then tagged
//!   `<number>/v<n>` (a lightweight tag, as the seeded history's are).
//!
//! A live draft whose changes aren't due yet keeps its file on `main` as it
//! is. Passes run before every request a repository answers, so a fetch or
//! push sees what is due, and every [`Timing::every`] in the background.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use diesel_async::AsyncConnection as _;
use kuria_store::akter::AktRow;
use kuria_store::git::{DraftHead, Mark};
use uuid::Uuid;

use super::cache::{self, git};
use super::{GitState, Opened};
use crate::akter::StoredType;
use crate::authorize::Authorizer;
use crate::graphql::Principal;

/// The branch Kuria commits on.
const MAIN: &str = "refs/heads/main";

/// When Kuria's changes become commits.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// How long a live draft must go without changes before they are
    /// committed: a minute.
    pub quiet: Duration,
    /// How often every repository is looked at in the background, if at
    /// all.
    pub every: Option<Duration>,
}

impl Timing {
    /// As a server runs: a minute's quiet, every repository looked at every
    /// five seconds.
    #[must_use]
    pub fn serving() -> Self {
        Self {
            every: Some(Duration::from_secs(5)),
            ..Self::default()
        }
    }
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            quiet: Duration::from_secs(60),
            every: None,
        }
    }
}

/// The repository `akt`, `number`, of `tenant`, as Kuria opens it to
/// commit to it: reading every akter and contact.
pub(crate) fn as_kuria(state: &GitState, tenant: Uuid, akt: Uuid, number: &str) -> Opened {
    Opened {
        principal: Principal {
            subject: Uuid::nil().into(),
            tenant: tenant.into(),
            expires_at: i64::MAX,
        },
        authorizer: Arc::new(Authorizer::kuria()),
        tenant,
        akt,
        number: number.to_owned(),
        dir: state.caches.dir(tenant, akt),
    }
}

/// Looks at every repository with a `main`, committing what is due.
pub(crate) async fn sweep(state: &GitState) {
    let repositories = async {
        let mut conn = state.pool.get().await.map_err(|e| e.to_string())?;
        kuria_store::git::repositories(&mut conn)
            .await
            .map_err(|e| e.to_string())
    };
    let repositories = match repositories.await {
        Ok(found) => found,
        Err(e) => {
            tracing::warn!("listing repositories: {e}");
            return;
        }
    };
    for (tenant, akt, number) in repositories {
        let opened = as_kuria(state, tenant, akt, &number);
        let _guard = state.caches.lock(&opened.dir).await;
        let done = async {
            cache::fill(state, &opened).await?;
            commit_pending(state, &opened).await
        };
        if let Err(e) = done.await {
            tracing::warn!("committing to {number}: {e}");
        }
    }
}

/// Runs the background sweep every `every`.
pub(crate) fn spawn(state: GitState, every: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(every);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            sweep(&state).await;
        }
    });
}

/// Commits what is due of Kuria's changes to the repository `opened`,
/// whose cache is filled and locked, and writes the cache's refs again if
/// it did (`git.repo.commits`).
// r[impl git.repo.commits]
pub(crate) async fn commit_pending(state: &GitState, opened: &Opened) -> Result<(), String> {
    let mut moved = false;
    // A release is committed alone; the draft's later changes after it.
    for _ in 0..3 {
        match pass(state, opened).await? {
            Passed::Nothing => break,
            Passed::Done => {
                moved = true;
                break;
            }
            Passed::Again => moved = true,
        }
    }
    if moved {
        cache::fill(state, opened).await?;
    }
    Ok(())
}

/// What a pass did.
enum Passed {
    /// Nothing was due.
    Nothing,
    /// It committed, or moved marks or tags.
    Done,
    /// It did, and more is due.
    Again,
}

/// Where an akt's file in the next commit comes from.
#[derive(Clone)]
enum Source {
    /// Kuria now, its live draft's fields if it has one.
    Now,
    /// Its latest revision, a release's fields.
    Released(i32),
    /// `main`'s file, as Kuria keeps it.
    Main(String),
}

/// One akt of the repository, and what the pass makes of it.
struct Held {
    row: AktRow,
    old: Option<Mark>,
    new: Mark,
    due: bool,
    source: Source,
    /// The versions it releases into `main`, by number.
    released: Vec<i32>,
}

/// Each akt the repository holds now: the container, then its members.
async fn akter_of(
    conn: &mut kuria_store::Connection,
    opened: &Opened,
) -> Result<Vec<(AktRow, bool)>, String> {
    let e = |e: diesel::result::Error| e.to_string();
    let container = kuria_store::akter::akt_by_number(conn, opened.tenant, &opened.number)
        .await
        .map_err(e)?
        .ok_or("the repository's akt is gone")?;
    let mut rows = vec![container.clone()];
    rows.extend(
        kuria_store::containers::members_with_types(conn, opened.tenant, container.id)
            .await
            .map_err(e)?
            .into_iter()
            .map(|(row, _)| row),
    );
    let type_ids: Vec<Uuid> = rows.iter().map(|r| r.type_version_id).collect();
    let types: HashMap<Uuid, bool> =
        kuria_store::akter::type_versions_by_id(conn, opened.tenant, &type_ids)
            .await
            .map_err(e)?
            .into_iter()
            .map(|row| {
                let id = row.id;
                StoredType::read(row).map(|t| (id, crate::akter::live::has_live_draft(&t)))
            })
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let live = types.get(&row.type_version_id).copied().unwrap_or(false);
            (row, live)
        })
        .collect())
}

/// What `main` holds of each akt the repository holds, as Kuria is now,
/// had it all been committed: each akt's revision, and its live draft's
/// last update, or, if `seen` gives the revisions a history was built from,
/// that revision and the last update a release took, as that history has
/// them.
pub(crate) async fn now(
    conn: &mut kuria_store::Connection,
    opened: &Opened,
    seen: Option<&BTreeMap<String, i64>>,
) -> Result<Vec<Mark>, String> {
    let akter = akter_of(conn, opened).await?;
    let ids: Vec<Uuid> = akter.iter().map(|(r, _)| r.id).collect();
    let heads: HashMap<Uuid, DraftHead> = kuria_store::git::draft_heads(conn, &ids)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|h| (h.akt_id, h))
        .collect();
    Ok(akter
        .iter()
        .map(|(row, _)| {
            let head = heads.get(&row.id);
            match seen {
                Some(seen) => Mark {
                    akt_id: row.id,
                    revision: seen
                        .get(&row.number)
                        .and_then(|r| i32::try_from(*r).ok())
                        .unwrap_or(0),
                    draft_seq: head.map_or(0, |h| h.released_seq),
                },
                None => Mark {
                    akt_id: row.id,
                    revision: row.revision,
                    draft_seq: head.map_or(0, |h| h.last_seq),
                },
            }
        })
        .collect())
}

/// Moves each mark that `before` has as it is to where `after` has it: what
/// a push changed of akter `main` held as they were is on `main` now, by
/// the push, so it isn't Kuria's to commit.
pub(crate) async fn advance(
    conn: &mut kuria_store::Connection,
    opened: &Opened,
    before: &[Mark],
    after: &[Mark],
) -> Result<(), diesel::result::Error> {
    let marks: HashMap<Uuid, Mark> = kuria_store::git::marks(conn, opened.tenant, opened.akt)
        .await?
        .into_iter()
        .map(|m| (m.akt_id, m))
        .collect();
    let moved: Vec<Mark> = after
        .iter()
        .filter(|a| {
            let was = before.iter().find(|b| b.akt_id == a.akt_id);
            was.is_some() && marks.get(&a.akt_id) == was
        })
        .copied()
        .collect();
    let keep: Vec<Uuid> = marks.keys().copied().collect();
    kuria_store::git::set_marks(conn, opened.tenant, opened.akt, &moved, &keep).await
}

/// Decides what of each akt is due, from its mark.
fn decide(
    akter: Vec<(AktRow, bool)>,
    heads: &HashMap<Uuid, DraftHead>,
    marks: &HashMap<Uuid, Mark>,
    versions: &[kuria_store::versions::VersionRow],
    quiet: Duration,
    now: DateTime<Utc>,
) -> (Vec<Held>, bool) {
    let mut again = false;
    let held = akter
        .into_iter()
        .map(|(row, live)| {
            let old = marks.get(&row.id).copied();
            let revision = row.revision;
            let mark = |draft_seq| Mark {
                akt_id: row.id,
                revision,
                draft_seq,
            };
            let changed = old.is_none_or(|m| m.revision != revision);
            let unchanged = |old: Mark| (old, false, Source::Main(String::new()));
            let (new, due, source) = if live {
                let head = heads.get(&row.id);
                let last = head.map_or(0, |h| h.last_seq);
                let old_revision = old.map_or(0, |m| m.revision);
                let released = versions
                    .iter()
                    .any(|v| v.akt_id == row.id && v.revision > old_revision);
                let quiet =
                    head.is_none_or(|h| (now - h.updated_at).to_std().unwrap_or_default() >= quiet);
                match old {
                    _ if released => {
                        let seq = head
                            .map_or(0, |h| h.released_seq)
                            .max(old.map_or(0, |m| m.draft_seq));
                        again |= last > seq && quiet;
                        (mark(seq), true, Source::Released(revision))
                    }
                    Some(old) if !changed && (last == old.draft_seq || !quiet) => unchanged(old),
                    _ => (mark(last), true, Source::Now),
                }
            } else {
                match old {
                    Some(old) if !changed => unchanged(old),
                    _ => (mark(0), true, Source::Now),
                }
            };
            let old_revision = old.map_or(0, |m| m.revision);
            let mut released: Vec<i32> = versions
                .iter()
                .filter(|v| {
                    v.akt_id == row.id && v.revision > old_revision && v.revision <= new.revision
                })
                .map(|v| v.number)
                .collect();
            if !due {
                released.clear();
            }
            Held {
                row,
                old,
                new,
                due,
                source,
                released,
            }
        })
        .collect();
    (held, again)
}

/// One pass: commits what is due, if anything is.
async fn pass(state: &GitState, opened: &Opened) -> Result<Passed, String> {
    let dir = &opened.dir;
    let e = |e: diesel::result::Error| e.to_string();
    let mut conn = state.pool.get().await.map_err(|e| e.to_string())?;
    let refs: BTreeMap<String, String> =
        kuria_store::git::refs(&mut conn, opened.tenant, opened.akt)
            .await
            .map_err(e)?
            .into_iter()
            .collect();
    let Some(main) = refs.get(MAIN).cloned() else {
        return Ok(Passed::Nothing);
    };
    let akter = akter_of(&mut conn, opened).await?;
    let ids: Vec<Uuid> = akter.iter().map(|(r, _)| r.id).collect();
    let heads: HashMap<Uuid, DraftHead> = kuria_store::git::draft_heads(&mut conn, &ids)
        .await
        .map_err(e)?
        .into_iter()
        .map(|h| (h.akt_id, h))
        .collect();
    let marks: HashMap<Uuid, Mark> = kuria_store::git::marks(&mut conn, opened.tenant, opened.akt)
        .await
        .map_err(e)?
        .into_iter()
        .map(|m| (m.akt_id, m))
        .collect();
    let versions = kuria_store::versions::of_akter(&mut conn, &ids)
        .await
        .map_err(e)?;
    let (mut held, again) = decide(
        akter,
        &heads,
        &marks,
        &versions,
        state.timing.quiet,
        Utc::now(),
    );
    let removed = marks.keys().any(|id| !ids.contains(id));
    if !removed && !held.iter().any(|h| h.due) {
        return Ok(Passed::Nothing);
    }

    // `main`'s files, by number, as Kuria keeps them.
    let mut on_main: HashMap<String, String> = HashMap::new();
    for (path, text) in super::history::files(dir, &[], &main).await? {
        if kuria_cli::remote::is_picture_path(&path) {
            continue;
        }
        if let Some(number) = kuria_cli::remote::number_of_file(&text) {
            let kept = kuria_cli::remote::kept_files(&BTreeMap::from([(path, text)]));
            on_main.insert(number, kept.into_values().next().unwrap_or_default());
        }
    }
    let mut asked = Vec::new();
    for h in &mut held {
        if let Source::Main(text) = &mut h.source {
            match on_main.get(&h.row.number) {
                Some(kept) => kept.clone_into(text),
                None => h.source = Source::Now,
            }
        }
        match h.source {
            Source::Now => asked.push((h.row.number.clone(), None)),
            Source::Released(r) => asked.push((h.row.number.clone(), Some(i64::from(r)))),
            Source::Main(_) => {}
        }
    }
    drop(conn);
    let remote = super::history::remote(state, opened);
    let said = |e: kuria_cli::error::CliError| format!("Kuria's tree: {}", e.message);
    let mut read = remote.files_of(&asked).await.map_err(said)?.into_iter();
    let mut files = BTreeMap::new();
    for h in &held {
        let text = match &h.source {
            Source::Main(text) => text.clone(),
            _ => read.next().unwrap_or_default(),
        };
        files.insert(format!("{}.md", h.row.number), text);
    }
    let (tree, pictures) = remote.normal_tree(&files).await.map_err(said)?;

    let message = message(state, opened, &held).await?;
    let built = build(dir, &main, &tree, &pictures, &message).await?;
    let result = keep(state, opened, &main, &refs, &held, &ids, built.as_ref()).await;
    if let Some(built) = &built {
        cache::forget(&built.scratch).await;
    }
    result?;
    Ok(if again { Passed::Again } else { Passed::Done })
}

/// A commit's author, committer and message.
struct Message {
    author: String,
    committer: String,
    text: String,
}

/// Whoever made the changes: the latest change's author, with each other
/// as a `Co-authored-by:` trailer; Kuria as the committer.
async fn message(state: &GitState, opened: &Opened, held: &[Held]) -> Result<Message, String> {
    let mut conn = state.pool.get().await.map_err(|e| e.to_string())?;
    let mut changes: Vec<(Uuid, DateTime<Utc>)> = Vec::new();
    for h in held.iter().filter(|h| h.due) {
        let after = h.old.unwrap_or(Mark {
            akt_id: h.row.id,
            revision: 0,
            draft_seq: 0,
        });
        changes.extend(
            kuria_store::git::changed_by(&mut conn, opened.tenant, after, h.new)
                .await
                .map_err(|e| e.to_string())?,
        );
    }
    changes.sort_by_key(|(_, at)| *at);
    let subjects: Vec<Uuid> = changes.iter().map(|(s, _)| *s).collect();
    let people: HashMap<Uuid, kuria_store::people::Person> =
        kuria_store::people::find(&mut conn, &subjects)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|p| (p.subject_id, p))
            .collect();
    let named = |subject: Uuid| {
        let person = people.get(&subject);
        format!(
            "{} <{}>",
            person
                .and_then(|p| p.name.clone())
                .unwrap_or_else(|| "Kuria".to_owned()),
            person
                .and_then(|p| p.email.clone())
                .unwrap_or_else(|| format!("{subject}@kuria"))
        )
    };
    let now = Utc::now();
    let committer = format!("{} {} +0000", kuria_of(&state.public_url), now.timestamp());
    let (author, at) = changes.last().map_or_else(
        || (kuria_of(&state.public_url), now),
        |(s, at)| (named(*s), *at),
    );
    let mut others: Vec<String> = Vec::new();
    for (subject, _) in changes.iter().rev() {
        let name = named(*subject);
        if name != author && !others.contains(&name) {
            others.push(name);
        }
    }
    let releases: Vec<String> = held
        .iter()
        .flat_map(|h| h.released.iter().map(|n| format!("{} v{n}", h.row.number)))
        .collect();
    let mut text = if releases.is_empty() {
        let numbers: Vec<&str> = held
            .iter()
            .filter(|h| h.due)
            .map(|h| h.row.number.as_str())
            .collect();
        format!("Change {}", numbers.join(", "))
    } else {
        format!("Release {}", releases.join(", "))
    };
    text.push('\n');
    if !others.is_empty() {
        text.push('\n');
        for other in others {
            let _ = writeln!(text, "Co-authored-by: {other}");
        }
    }
    Ok(Message {
        author: format!("{author} {} +0000", at.timestamp()),
        committer,
        text,
    })
}

/// Kuria, as it commits: named by its public URL's host.
fn kuria_of(public_url: &str) -> String {
    let host = public_url
        .split_once("://")
        .map_or(public_url, |(_, rest)| rest)
        .split(['/', ':'])
        .next()
        .filter(|h| !h.is_empty())
        .unwrap_or("kuria");
    format!("Kuria <kuria@{host}>")
}

/// A commit built in a scratch repository.
struct Built {
    scratch: std::path::PathBuf,
    commit: String,
}

/// The commit of `tree` and `pictures` on `main`, built in a scratch
/// repository beside the cache at `dir`, if its tree isn't `main`'s.
async fn build(
    dir: &std::path::Path,
    main: &str,
    tree: &BTreeMap<String, String>,
    pictures: &BTreeMap<String, Vec<u8>>,
    message: &Message,
) -> Result<Option<Built>, String> {
    let scratch = cache::scratch(dir);
    cache::init(&scratch).await?;
    let objects = std::fs::canonicalize(dir.join("objects")).map_err(|e| e.to_string())?;
    tokio::fs::write(
        scratch.join("objects/info/alternates"),
        format!("{}\n", objects.display()),
    )
    .await
    .map_err(|e| e.to_string())?;
    let mut stream = Vec::new();
    let data = |out: &mut Vec<u8>, bytes: &[u8]| {
        out.extend_from_slice(format!("data {}\n", bytes.len()).as_bytes());
        out.extend_from_slice(bytes);
        out.push(b'\n');
    };
    stream.extend_from_slice(
        format!(
            "commit {MAIN}\nauthor {}\ncommitter {}\n",
            message.author, message.committer
        )
        .as_bytes(),
    );
    data(&mut stream, message.text.as_bytes());
    stream.extend_from_slice(format!("from {main}\ndeleteall\n").as_bytes());
    for (path, text) in tree {
        stream.extend_from_slice(format!("M 100644 inline {path}\n").as_bytes());
        data(&mut stream, text.as_bytes());
    }
    for (path, bytes) in pictures {
        stream.extend_from_slice(format!("M 100644 inline {path}\n").as_bytes());
        data(&mut stream, bytes);
    }
    stream.extend_from_slice(b"\ndone\n");
    let built = async {
        git(
            &scratch,
            &[
                "-c",
                "fastimport.unpackLimit=0",
                "fast-import",
                "--quiet",
                "--done",
            ],
            &[],
            Some(stream),
        )
        .await?;
        let rev = |what: String| {
            let scratch = scratch.clone();
            async move {
                git(&scratch, &["rev-parse", &what], &[], None)
                    .await
                    .map(|out| String::from_utf8_lossy(&out).trim().to_owned())
            }
        };
        let commit = rev(MAIN.to_owned()).await?;
        let same =
            rev(format!("{commit}^{{tree}}")).await? == rev(format!("{main}^{{tree}}")).await?;
        Ok::<_, String>((commit, same))
    };
    match built.await {
        Ok((commit, false)) => Ok(Some(Built { scratch, commit })),
        Ok((_, true)) => {
            cache::forget(&scratch).await;
            Ok(None)
        }
        Err(e) => {
            cache::forget(&scratch).await;
            Err(e)
        }
    }
}

/// Why a pass's transaction was rolled back.
struct Rolled(String);

impl From<diesel::result::Error> for Rolled {
    fn from(error: diesel::result::Error) -> Self {
        Self(error.to_string())
    }
}

/// Keeps a pass: the commit's pack stored, then, in one transaction, `main`
/// moved to it by compare-and-swap, the marks and the new versions' tags;
/// then the pack is the cache's. A `main` moved meanwhile, by a push, keeps
/// nothing: the next pass looks again.
async fn keep(
    state: &GitState,
    opened: &Opened,
    main: &str,
    refs: &BTreeMap<String, String>,
    held: &[Held],
    ids: &[Uuid],
    built: Option<&Built>,
) -> Result<(), String> {
    if let Some(built) = built {
        cache::store_packs(state, opened, &built.scratch.join("objects")).await?;
    }
    let at = built.map_or(main, |b| b.commit.as_str());
    let marks: Vec<Mark> = held.iter().map(|h| h.new).collect();
    let mut conn = state.pool.get().await.map_err(|e| e.to_string())?;
    let kept = conn
        .transaction::<_, Rolled, _>(async |conn| {
            if built.is_some()
                && !kuria_store::git::move_ref(
                    conn,
                    opened.tenant,
                    opened.akt,
                    MAIN,
                    Some(main),
                    Some(at),
                )
                .await?
            {
                return Err(Rolled("main moved meanwhile".to_owned()));
            }
            kuria_store::git::set_marks(conn, opened.tenant, opened.akt, &marks, ids).await?;
            for h in held {
                for n in &h.released {
                    let tag = format!("refs/tags/{}/v{n}", h.row.number);
                    if !refs.contains_key(&tag) {
                        // One pushed meanwhile stays as pushed.
                        kuria_store::git::move_ref(
                            conn,
                            opened.tenant,
                            opened.akt,
                            &tag,
                            None,
                            Some(at),
                        )
                        .await?;
                    }
                }
            }
            Ok(())
        })
        .await;
    match kept {
        Ok(()) => {
            if let Some(built) = built {
                cache::install_packs(&built.scratch.join("objects"), &opened.dir).await?;
            }
            Ok(())
        }
        Err(Rolled(why)) => {
            tracing::info!("not committing to {}: {why}", opened.number);
            Ok(())
        }
    }
}
