//! The remote helper's history of a repository's container, built in the
//! server by the helper's own code (`kuria_cli::remote`), its GraphQL
//! answered in-process by the server's schema as the caller, so that every
//! commit has the id a clone made through `git-remote-kuria` gives it
//! (`git.repo.seed`, `git.identity`). A push to `main` is checked and made
//! into batches by the same code (`git.repo.push`).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use async_graphql::Variables;
use kuria_cli::error::CliError;
use kuria_cli::remote::{Exported, History, Remote};
use kuria_cli::session::{Answering, Api, Session};
use serde_json::Value as Json;
use uuid::Uuid;

use super::cache::{self, git};
use super::{GitState, Opened};
use crate::attachments::Storage;
use crate::authorize::Authorizer;
use crate::graphql::{KuriaSchema, Principal};

/// Kuria's API, answered by the server itself as the caller.
struct InProcess {
    schema: KuriaSchema,
    principal: Principal,
    authorizer: Arc<Authorizer>,
    pool: kuria_store::Pool,
    storage: Storage,
}

impl Api for InProcess {
    fn graphql<'a>(&'a self, query: &'a str, variables: Json) -> Answering<'a, Json> {
        Box::pin(async move {
            let request = async_graphql::Request::new(query)
                .variables(Variables::from_json(variables))
                .data(self.principal)
                .data(self.authorizer.clone());
            let response = self.schema.execute(request).await;
            serde_json::to_value(&response).map_err(|e| CliError::unavailable(e.to_string()))
        })
    }

    fn attachment<'a>(&'a self, id: &'a str) -> Answering<'a, Vec<u8>> {
        Box::pin(async move {
            let missing = || CliError::refused("NOT_FOUND", format!("no attachment {id}"));
            let id = Uuid::parse_str(id).map_err(|_| missing())?;
            let tenant: Uuid = self.principal.tenant.into();
            let unavailable = |e: String| CliError::unavailable(e);
            let mut conn = self
                .pool
                .get()
                .await
                .map_err(|e| unavailable(e.to_string()))?;
            let row = kuria_store::attachments::find(&mut conn, tenant, &[id])
                .await
                .map_err(|e| unavailable(e.to_string()))?
                .pop()
                .ok_or_else(missing)?;
            let subject: Uuid = self.principal.subject.into();
            if !crate::attachments::may_read(&mut conn, &self.authorizer, subject, &row)
                .await
                .map_err(|e| unavailable(e.to_string()))?
            {
                return Err(missing());
            }
            self.storage
                .get(tenant, &row.sha256)
                .await
                .map_err(|e| unavailable(e.to_string()))
        })
    }
}

/// The repository's container as the remote helper sees it, signed in as
/// the caller, keeping what it reads in the cache (`git.fetch.cache`).
pub(crate) fn remote(state: &GitState, opened: &Opened) -> Remote {
    let api = InProcess {
        schema: state.schema.clone(),
        principal: opened.principal,
        authorizer: opened.authorizer.clone(),
        pool: state.pool.clone(),
        storage: state.storage.clone(),
    };
    Remote {
        session: Session::answered_by(state.public_url.clone(), Arc::new(api)),
        number: opened.number.clone(),
        name: "kuria".to_owned(),
        git_dir: opened.dir.clone(),
    }
}

/// A history built into a scratch repository beside the cache, its objects
/// apart from the repository's until they are kept.
pub(crate) struct Built {
    /// The scratch repository, whose objects directory holds them.
    pub scratch: std::path::PathBuf,
    /// Its last commit, `main`'s, if it has any.
    pub main: Option<String>,
    /// Its versions' tags, `refs/tags/<number>/v<n>`, to their commits.
    pub tags: Vec<(String, String)>,
}

/// `history` as git objects, by `git fast-import`, in a scratch repository
/// that borrows the cache at `dir`'s objects (`git.history`).
pub(crate) async fn build(dir: &Path, history: &History) -> Result<Built, String> {
    let scratch = cache::scratch(dir);
    cache::init(&scratch).await?;
    let objects = std::fs::canonicalize(dir.join("objects")).map_err(|e| e.to_string())?;
    tokio::fs::write(
        scratch.join("objects/info/alternates"),
        format!("{}\n", objects.display()),
    )
    .await
    .map_err(|e| e.to_string())?;
    let marks = scratch.join("marks");
    let stream = kuria_cli::remote::fast_import(history, "refs/heads/main");
    let imported = git(
        &scratch,
        &[
            // Packed, however few: packs are what storage keeps.
            "-c",
            "fastimport.unpackLimit=0",
            "fast-import",
            "--quiet",
            &format!("--export-marks={}", marks.display()),
        ],
        &[],
        Some(stream),
    )
    .await;
    if let Err(e) = imported {
        cache::forget(&scratch).await;
        return Err(e);
    }
    let marked: BTreeMap<usize, String> = tokio::fs::read_to_string(&marks)
        .await
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (mark, id) = l.strip_prefix(':')?.split_once(' ')?;
            Some((mark.parse().ok()?, id.to_owned()))
        })
        .collect();
    let main = history
        .commits
        .len()
        .checked_sub(1)
        .and_then(|i| marked.get(&(i + 1)).cloned());
    let tags = history
        .tags
        .iter()
        .filter_map(|(tag, i)| Some((format!("refs/tags/{tag}"), marked.get(&(i + 1))?.clone())))
        .collect();
    Ok(Built {
        scratch,
        main,
        tags,
    })
}

/// Why a history couldn't be read.
fn unread(e: &CliError) -> String {
    format!("the container's history: {}", e.message)
}

/// Starts the repository, which has no refs, from the history the remote
/// helper builds for its container: its objects packed and stored, then
/// `main` and the versions' tags created, unless a racing request created
/// them first (`git.repo.seed`).
// r[impl git.repo.seed]
pub(crate) async fn seed(state: &GitState, opened: &Opened) -> Result<(), String> {
    let history = remote(state, opened)
        .history()
        .await
        .map_err(|e| unread(&e))?;
    let built = build(&opened.dir, &history).await?;
    let kept = keep_seed(state, opened, &built).await;
    cache::forget(&built.scratch).await;
    kept
}

async fn keep_seed(state: &GitState, opened: &Opened, built: &Built) -> Result<(), String> {
    use diesel_async::AsyncConnection as _;
    let Some(main) = &built.main else {
        return Ok(());
    };
    let objects = built.scratch.join("objects");
    cache::store_packs(state, opened, &objects).await?;
    let mut refs = vec![("refs/heads/main".to_owned(), main.clone())];
    refs.extend(built.tags.iter().cloned());
    let mut conn = state.pool.get().await.map_err(|e| e.to_string())?;
    let seeded = conn
        .transaction::<_, diesel::result::Error, _>(async |conn| {
            for (name, target) in &refs {
                let made = kuria_store::git::move_ref(
                    conn,
                    opened.tenant,
                    opened.akt,
                    name,
                    None,
                    Some(target),
                )
                .await?;
                if !made {
                    // Seeded meanwhile, by another server.
                    return Err(diesel::result::Error::RollbackTransaction);
                }
            }
            Ok(())
        })
        .await;
    match seeded {
        Ok(()) => cache::install_packs(&objects, &opened.dir).await,
        Err(diesel::result::Error::RollbackTransaction) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// The batches a push to `main` makes, each a commit's, and the akter's
/// revisions it was checked against.
pub(crate) struct Planned {
    /// Each commit's id, its batch's operations as the `batch` mutation
    /// takes them, and the commit Kuria keeps with its revisions.
    pub batches: Vec<(String, Vec<Json>, Json)>,
    /// Each akt of the container's history, by number, at its revision.
    pub seen: BTreeMap<String, i64>,
}

/// A commit's tree: each file's path, to its text as git shows it.
pub(crate) async fn files(
    dir: &Path,
    env: &[(&str, String)],
    commit: &str,
) -> Result<BTreeMap<String, String>, String> {
    let listed = git(dir, &["ls-tree", "-r", "-z", commit], env, None).await?;
    let mut entries = Vec::new();
    for entry in listed.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = String::from_utf8_lossy(entry);
        let (meta, path) = entry.split_once('\t').ok_or("git's ls-tree")?;
        let mut parts = meta.split(' ');
        let (kind, id) = (
            parts.nth(1).unwrap_or_default(),
            parts.next().unwrap_or_default(),
        );
        if kind != "blob" {
            return Err(format!("{path} isn't a file"));
        }
        entries.push((path.to_owned(), id.to_owned()));
    }
    let mut tree = BTreeMap::new();
    for (path, id) in entries {
        let bytes = git(dir, &["cat-file", "blob", &id], env, None).await?;
        tree.insert(path, String::from_utf8_lossy(&bytes).into_owned());
    }
    Ok(tree)
}

/// The commit `id`, as `git fast-export` would give it, refusing one Kuria
/// can't keep as it is: one with headers besides its tree, parent, author
/// and committer, such as a signature, which every clone built again
/// wouldn't have (`git.identity`).
pub(crate) async fn exported(
    dir: &Path,
    env: &[(&str, String)],
    id: &str,
) -> Result<Exported, String> {
    let raw = git(dir, &["cat-file", "commit", id], env, None).await?;
    let raw = String::from_utf8(raw).map_err(|_| format!("commit {id} isn't UTF-8"))?;
    let (head, message) = raw.split_once("\n\n").unwrap_or((&raw, ""));
    let mut commit = Exported {
        message: message.to_owned(),
        ..Exported::default()
    };
    for line in head.lines() {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "tree" => {}
            "parent" => commit.from = Some(value.to_owned()),
            "author" => value.clone_into(&mut commit.author),
            "committer" => value.clone_into(&mut commit.committer),
            _ => {
                return Err(format!(
                    "commit {id} has a `{key}` header, which Kuria can't keep: push it without (unsigned, in UTF-8)"
                ));
            }
        }
    }
    Ok(commit)
}

/// A commit's files and its pictures (`git.pictures`), apart.
fn split(tree: BTreeMap<String, String>) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    tree.into_iter()
        .partition(|(path, _)| !kuria_cli::remote::is_picture_path(path))
}

/// Checks a push moving `main` from `old` to `new`, whose objects `env`
/// finds, as the remote helper checks a push, and makes each new commit a
/// batch, as it applies them (`git.repo.push`, `git.push`): a fast-forward
/// of `main` as Kuria has it, linear, each commit in normal form and
/// changing something in Kuria.
// r[impl git.repo.push]
// r[impl git.push.fast-forward]
pub(crate) async fn plan_main(
    state: &GitState,
    opened: &Opened,
    env: &[(&str, String)],
    old: &str,
    new: &str,
) -> Result<Planned, String> {
    let dir = &opened.dir;
    if git(dir, &["merge-base", "--is-ancestor", old, new], env, None)
        .await
        .is_err()
    {
        return Err("not a fast-forward: pull and push again".to_owned());
    }
    let listed = git(
        dir,
        &[
            "rev-list",
            "--reverse",
            "--topo-order",
            "--parents",
            &format!("{old}..{new}"),
        ],
        env,
        None,
    )
    .await?;
    let mut commits = Vec::new();
    for line in String::from_utf8_lossy(&listed).lines() {
        let ids: Vec<&str> = line.split(' ').collect();
        if ids.len() != 2 {
            return Err("a merge commit can't be pushed to main yet: rebase instead".to_owned());
        }
        commits.push(ids[0].to_owned());
    }
    let remote = remote(state, opened);
    let history = remote.history().await.map_err(|e| unread(&e))?;
    let built = build(dir, &history).await?;
    cache::forget(&built.scratch).await;
    if built.main.as_deref() != Some(old) {
        return Err(
            "Kuria has changes main doesn't have yet: they can't be pushed over".to_owned(),
        );
    }
    let mut batches = Vec::new();
    let (mut before, _) = split(files(dir, env, old).await?);
    for id in commits {
        let commit = exported(dir, env, &id).await?;
        let (tree, pictures) = split(files(dir, env, &id).await?);
        let said = |e: CliError| format!("commit {id}: {}", e.message);
        remote.check_normal(&tree, &pictures).await.map_err(said)?;
        let kept = kuria_cli::remote::kept_files;
        let operations = remote
            .operations(&kept(&before), &kept(&tree), &commit)
            .await
            .map_err(said)?;
        if operations.is_empty() {
            return Err(format!("commit {id} changes nothing in Kuria"));
        }
        batches.push((id, operations, kuria_cli::remote::commit_of(&commit)));
        before = tree;
    }
    Ok(Planned {
        batches,
        seen: history.seen,
    })
}
