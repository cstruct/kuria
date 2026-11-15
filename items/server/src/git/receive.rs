//! Pushes (`git.repo.push`): `git-receive-pack`'s requests, answered as git
//! answers them, with the checks Kuria makes before anything is kept.
//!
//! The pushed pack is indexed by `git index-pack` into a quarantine, an
//! objects directory of its own beside the cache, where git finds it
//! besides the repository's objects while the push is checked. Every ref's
//! update is checked; a push to `main` is made into batches, one per
//! commit, by the remote helper's own code. Only if all of it passes are
//! the batches run, the refs moved by compare-and-swap and the pack
//! stored, in one transaction: a refused push keeps nothing.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use diesel_async::AsyncConnection as _;

use super::cache::{self, git};
use super::history::{self, Planned};
use super::{GitState, Opened, pkt};
use crate::akter::{Caller, Operation};

/// No object: a ref created, or deleted.
const ZERO: &str = "0000000000000000000000000000000000000000";

/// The branch that is the container and its members (`git.repo.main`).
const MAIN: &str = "refs/heads/main";

/// One ref's update.
struct Update {
    old: String,
    new: String,
    name: String,
}

impl Update {
    fn from(&self) -> Option<&str> {
        (self.old != ZERO).then_some(self.old.as_str())
    }

    fn to(&self) -> Option<&str> {
        (self.new != ZERO).then_some(self.new.as_str())
    }
}

/// What a push asks: its updates, the capabilities it takes, and its pack.
struct Asked<'a> {
    updates: Vec<Update>,
    capabilities: Vec<String>,
    pack: &'a [u8],
}

/// Reads a push's request.
fn asked(body: &[u8]) -> Result<Asked<'_>, String> {
    let (lines, pack) = pkt::read(body)?;
    let mut updates = Vec::new();
    let mut capabilities = Vec::new();
    for line in lines {
        let line = String::from_utf8_lossy(line);
        let line = line.trim_end_matches('\n');
        let (command, caps) = line.split_once('\0').unwrap_or((line, ""));
        if !caps.is_empty() {
            capabilities.extend(caps.split(' ').map(str::to_owned));
        }
        if command.starts_with("shallow ") {
            continue;
        }
        let mut parts = command.splitn(3, ' ');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(old), Some(new), Some(name)) => updates.push(Update {
                old: old.to_owned(),
                new: new.to_owned(),
                name: name.to_owned(),
            }),
            _ => return Err(format!("a command git doesn't send: {command}")),
        }
    }
    Ok(Asked {
        updates,
        capabilities,
        pack,
    })
}

/// The answer git reads: the pack's unpacking, then each ref's update, `ok`
/// or `ng` and why, on side band 1 if git takes it.
fn report(
    asked: &Asked<'_>,
    unpacked: Result<(), &str>,
    refused: &HashMap<String, String>,
) -> Vec<u8> {
    let has = |c: &str| asked.capabilities.iter().any(|a| a == c);
    if !has("report-status") && !has("report-status-v2") {
        return Vec::new();
    }
    let mut status = match unpacked {
        Ok(()) => pkt::line(b"unpack ok\n"),
        Err(why) => pkt::line(format!("unpack {why}\n").as_bytes()),
    };
    for update in &asked.updates {
        let line = match refused.get(&update.name) {
            None => format!("ok {}\n", update.name),
            Some(why) => format!("ng {} {}\n", update.name, why.replace('\n', " ")),
        };
        status.extend_from_slice(&pkt::line(line.as_bytes()));
    }
    status.extend_from_slice(pkt::FLUSH);
    if has("side-band-64k") || has("side-band") {
        let mut out = pkt::banded(1, &status);
        out.extend_from_slice(pkt::FLUSH);
        out
    } else {
        status
    }
}

/// Every update refused: `why` for those it names, the rest for being in a
/// push that keeps nothing.
fn all_refused(asked: &Asked<'_>, mut why: HashMap<String, String>) -> HashMap<String, String> {
    for update in &asked.updates {
        why.entry(update.name.clone())
            .or_insert_with(|| "refused with the rest of the push".to_owned());
    }
    why
}

/// Answers a push, keeping it only if all of it passes.
pub(crate) async fn receive(
    state: &GitState,
    opened: &Opened,
    body: &[u8],
) -> Result<Vec<u8>, String> {
    let asked = asked(body)?;
    let quarantine = cache::scratch(&opened.dir).join("objects");
    let unpacked = unpack(&opened.dir, &quarantine, asked.pack).await;
    let answered = match unpacked {
        Err(why) => {
            let refused = all_refused(&asked, HashMap::new());
            report(&asked, Err(&why), &refused)
        }
        Ok(()) => {
            let refused = match checked(state, opened, &asked, &quarantine).await {
                Ok(planned) => kept(state, opened, &asked, planned, &quarantine)
                    .await
                    .err()
                    .map(|why| all_refused(&asked, HashMap::from([(MAIN.to_owned(), why)])))
                    .unwrap_or_default(),
                Err(why) => all_refused(&asked, why),
            };
            report(&asked, Ok(()), &refused)
        }
    };
    if let Some(scratch) = quarantine.parent() {
        cache::forget(scratch).await;
    }
    Ok(answered)
}

/// Indexes `pack` into the quarantine `objects`, completing a thin pack
/// from the repository at `dir`'s objects.
async fn unpack(dir: &Path, objects: &Path, pack: &[u8]) -> Result<(), String> {
    let pack_dir = objects.join("pack");
    tokio::fs::create_dir_all(&pack_dir)
        .await
        .map_err(|e| e.to_string())?;
    // A push of deletions alone sends no pack; one of refs to objects the
    // repository has may send an empty one.
    let objects_in = pack
        .get(8..12)
        .map_or(0, |n| u32::from_be_bytes([n[0], n[1], n[2], n[3]]));
    if objects_in == 0 {
        return Ok(());
    }
    let incoming = pack_dir.join("incoming.pack");
    let indexed = git(
        dir,
        &[
            "index-pack",
            "--stdin",
            "--fix-thin",
            &incoming.to_string_lossy(),
        ],
        &[],
        Some(pack.to_vec()),
    )
    .await
    .map_err(|e| {
        tracing::info!("a pushed pack: {e}");
        "the pack couldn't be read".to_owned()
    })?;
    let said = String::from_utf8_lossy(&indexed);
    let id = said.split_whitespace().last().unwrap_or_default();
    for extension in ["pack", "idx"] {
        tokio::fs::rename(
            pack_dir.join(format!("incoming.{extension}")),
            pack_dir.join(format!("pack-{id}.{extension}")),
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The environment git finds a push's objects with: the repository's, and
/// the quarantine's.
fn with_quarantine(objects: &Path) -> Vec<(&'static str, String)> {
    vec![(
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        objects.to_string_lossy().into_owned(),
    )]
}

/// Checks every update before anything is kept (`git.repo.push`): each a
/// branch or a tag, from where the ref is, to objects the push and the
/// repository hold; `main` only moved forward, and as the remote helper
/// takes a push (`git.repo.main`). Answers the batches a push to `main`
/// makes, or why each refused update was refused.
// r[impl git.repo.push]
// r[impl git.repo.branches]
async fn checked(
    state: &GitState,
    opened: &Opened,
    asked: &Asked<'_>,
    objects: &Path,
) -> Result<Option<Planned>, HashMap<String, String>> {
    let env = with_quarantine(objects);
    let refs: BTreeMap<String, String> = {
        let mut conn = state
            .pool
            .get()
            .await
            .map_err(|e| one(MAIN, &e.to_string()))?;
        kuria_store::git::refs(&mut conn, opened.tenant, opened.akt)
            .await
            .map_err(|e| one(MAIN, &e.to_string()))?
            .into_iter()
            .collect()
    };
    let mut refused = HashMap::new();
    for update in &asked.updates {
        if let Err(why) = check(&opened.dir, &env, &refs, update).await {
            refused.insert(update.name.clone(), why);
        }
    }
    if !refused.is_empty() {
        return Err(refused);
    }
    match asked.updates.iter().find(|u| u.name == MAIN) {
        Some(main) => {
            let (Some(old), Some(new)) = (main.from(), main.to()) else {
                return Err(one(
                    MAIN,
                    "main is the container: it can't be created or deleted",
                ));
            };
            history::plan_main(state, opened, &env, old, new)
                .await
                .map(Some)
                .map_err(|why| one(MAIN, &why))
        }
        None => Ok(None),
    }
}

fn one(name: &str, why: &str) -> HashMap<String, String> {
    HashMap::from([(name.to_owned(), why.to_owned())])
}

/// Checks one ref's update: a branch or tag, from where the ref is, to an
/// object of its kind that the push and the repository hold, with all it
/// needs.
async fn check(
    dir: &Path,
    env: &[(&str, String)],
    refs: &BTreeMap<String, String>,
    update: &Update,
) -> Result<(), String> {
    let branch = update.name.starts_with("refs/heads/");
    if !branch && !update.name.starts_with("refs/tags/") {
        return Err("only branches and tags are kept".to_owned());
    }
    if git(dir, &["check-ref-format", &update.name], env, None)
        .await
        .is_err()
    {
        return Err("not a valid ref name".to_owned());
    }
    if refs.get(&update.name).map(String::as_str) != update.from() {
        return Err("stale: the ref moved; fetch first".to_owned());
    }
    let Some(new) = update.to() else {
        return Ok(());
    };
    let kind = git(dir, &["cat-file", "-t", new], env, None)
        .await
        .map_err(|_| "missing objects".to_owned())?;
    let kind = String::from_utf8_lossy(&kind).trim().to_owned();
    let fits = kind == "commit" || (kind == "tag" && !branch);
    if !fits {
        return Err(format!(
            "a {} can't point to a {kind}",
            if branch { "branch" } else { "tag" }
        ));
    }
    let complete = git(
        dir,
        &["rev-list", "--objects", "--quiet", new, "--not", "--all"],
        env,
        None,
    )
    .await;
    complete.map(drop).map_err(|_| "missing objects".to_owned())
}

/// Why a push's transaction was rolled back.
struct Refused(String);

impl From<diesel::result::Error> for Refused {
    fn from(error: diesel::result::Error) -> Self {
        Self(error.to_string())
    }
}

/// Keeps a checked push, in one transaction: the batches of its commits to
/// `main`, once the akter they were checked against are as they were; its
/// refs, each moved by compare-and-swap; and its pack, stored. Then the
/// pack is the cache's too.
// r[impl git.repo.storage]
async fn kept(
    state: &GitState,
    opened: &Opened,
    asked: &Asked<'_>,
    planned: Option<Planned>,
    objects: &Path,
) -> Result<(), String> {
    let caller = Caller {
        actor: opened.principal.actor(),
        auth: &opened.authorizer,
        storage: state.akter.storage(),
        renderer: state.akter.renderer(),
    };
    let mut conn = state.pool.get().await.map_err(|e| e.to_string())?;
    let result = conn
        .transaction::<_, Refused, _>(async |conn| {
            if let Some(planned) = &planned {
                run_batches(conn, caller, opened.tenant, planned).await?;
            }
            for update in &asked.updates {
                let moved = kuria_store::git::move_ref(
                    conn,
                    opened.tenant,
                    opened.akt,
                    &update.name,
                    update.from(),
                    update.to(),
                )
                .await?;
                if !moved {
                    return Err(Refused(
                        "stale: the ref moved during the push; fetch first".to_owned(),
                    ));
                }
            }
            cache::store_packs(state, opened, objects)
                .await
                .map_err(Refused)?;
            Ok(())
        })
        .await;
    state.akter.woken(result).map_err(|Refused(why)| why)?;
    cache::install_packs(objects, &opened.dir).await
}

/// Runs a push's batches, each its commit's, after locking every akt they
/// were checked against and refusing if one changed since.
async fn run_batches(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    tenant: uuid::Uuid,
    planned: &Planned,
) -> Result<(), Refused> {
    let numbers: Vec<&str> = planned.seen.keys().map(String::as_str).collect();
    let rows = kuria_store::containers::lock_akter(conn, tenant, &numbers).await?;
    for row in &rows {
        if planned.seen.get(&row.number) != Some(&i64::from(row.revision)) {
            return Err(Refused(format!(
                "Kuria changed {} during the push: fetch and push again",
                row.number
            )));
        }
    }
    // Each akt's revision as the push's commits leave it.
    let mut revisions: HashMap<String, i32> = HashMap::new();
    for (id, operations, commit) in &planned.batches {
        let mut operations = crate::graphql::operations_from_json(operations.clone())
            .map_err(|e| Refused(format!("commit {id}: {e}")))?;
        for operation in &mut operations {
            if let Operation::Edit {
                number, revision, ..
            } = operation
                && let Some(now) = revisions.get(number.as_str())
            {
                *revision = *now;
            }
        }
        let rows = crate::akter::batch_in(conn, caller, operations, Some(commit))
            .await
            .map_err(|refused| {
                let at = refused
                    .operation
                    .map(|n| format!(" (operation {n})"))
                    .unwrap_or_default();
                Refused(format!("commit {id}{at}: {}", refused.error))
            })?;
        for row in rows {
            revisions.insert(row.number, row.revision);
        }
    }
    Ok(())
}
