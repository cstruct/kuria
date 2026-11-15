//! Each repository's local cache: a bare repository under the cache
//! directory, `<tenant>/<akt>.git`, that the `git` binary serves. Its
//! packfiles are those in object storage, under `git/<tenant>/<akt>/`,
//! downloaded when missing; its refs are written from the database before
//! each request (`git.repo.storage`). A cache can be thrown away at any
//! time: it is filled again from storage.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use tokio::io::AsyncWriteExt as _;
use uuid::Uuid;

use super::{GitState, Opened};

/// The caches of the repositories one server serves.
pub(crate) struct Caches {
    root: PathBuf,
    /// One lock per repository: a request has its cache to itself.
    locks: Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>,
}

impl Caches {
    /// Caches under `root`.
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            locks: Mutex::new(HashMap::new()),
        }
    }

    /// Where the repository `akt` of `tenant` is cached.
    pub(crate) fn dir(&self, tenant: Uuid, akt: Uuid) -> PathBuf {
        self.root
            .join(tenant.to_string())
            .join(format!("{akt}.git"))
    }

    /// The repository at `dir`, to itself until the guard is dropped.
    pub(crate) async fn lock(&self, dir: &Path) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = self
            .locks
            .lock()
            .expect("the caches' locks")
            .entry(dir.to_owned())
            .or_default()
            .clone();
        lock.lock_owned().await
    }
}

/// Where a repository's packfiles are in object storage.
pub(crate) fn prefix(tenant: Uuid, akt: Uuid) -> String {
    format!("git/{tenant}/{akt}")
}

/// Runs `git args…` in the repository at `dir`, with `env` and `input` on
/// its standard input, answering its standard output.
pub(crate) async fn git(
    dir: &Path,
    args: &[&str],
    env: &[(&str, String)],
    input: Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let mut command = tokio::process::Command::new("git");
    command
        .arg("--git-dir")
        .arg(dir)
        .args(args)
        .current_dir(dir)
        // Only the repository's own configuration counts.
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|e| format!("running git: {e}"))?;
    let writing = match (input, child.stdin.take()) {
        (Some(input), Some(mut stdin)) => Some(tokio::spawn(async move {
            // git may stop reading early, such as when it refuses.
            let _ = stdin.write_all(&input).await;
        })),
        _ => None,
    };
    let out = child
        .wait_with_output()
        .await
        .map_err(|e| format!("running git: {e}"))?;
    if let Some(writing) = writing {
        let _ = writing.await;
    }
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Makes a bare repository at `dir`, unless there is one.
pub(crate) async fn init(dir: &Path) -> Result<(), String> {
    if dir.join("HEAD").is_file() {
        return Ok(());
    }
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    tokio::fs::create_dir_all(dir).await.map_err(io)?;
    let made = tokio::process::Command::new("git")
        .args(["init", "--bare", "--quiet", "--initial-branch=main"])
        .arg(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .await
        .map_err(|e| format!("running git: {e}"))?;
    if !made.status.success() {
        return Err(format!(
            "git init: {}",
            String::from_utf8_lossy(&made.stderr).trim()
        ));
    }
    for (key, value) in [
        // Packs are storage's; the cache never repacks them.
        ("gc.auto", "0"),
        ("core.logAllRefUpdates", "false"),
    ] {
        git(dir, &["config", key, value], &[], None).await?;
    }
    Ok(())
}

/// The repository's cache, as storage and the database have it: made if it
/// isn't there, each packfile it lacks downloaded, and its refs written.
/// Answers the refs, as `(name, target)`.
// r[impl git.repo.storage]
pub(crate) async fn fill(
    state: &GitState,
    opened: &Opened,
) -> Result<Vec<(String, String)>, String> {
    let dir = &opened.dir;
    init(dir).await?;
    let packs = dir.join("objects/pack");
    let prefix = prefix(opened.tenant, opened.akt);
    let stored = state
        .storage
        .list_named(&prefix)
        .await
        .map_err(|e| format!("listing {prefix}: {e}"))?;
    for stem in stored.iter().filter_map(|n| n.strip_suffix(".pack")) {
        let (name, idx) = (format!("{stem}.pack"), format!("{stem}.idx"));
        if packs.join(&name).is_file() || !stored.contains(&idx) {
            continue;
        }
        // The pack before its index, so git never sees an index alone.
        for file in [name.as_str(), idx.as_str()] {
            let bytes = state
                .storage
                .get_named(&format!("{prefix}/{file}"))
                .await
                .map_err(|e| format!("reading {prefix}/{file}: {e}"))?;
            write_atomically(&packs.join(file), &bytes).await?;
        }
    }
    let refs = {
        let mut conn = state.pool.get().await.map_err(|e| e.to_string())?;
        kuria_store::git::refs(&mut conn, opened.tenant, opened.akt)
            .await
            .map_err(|e| e.to_string())?
    };
    write_refs(dir, &refs).await?;
    Ok(refs)
}

/// Writes `bytes` to `path` by renaming a file written beside it.
async fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", path.display());
    let partial = path.with_extension(format!("partial-{}", Uuid::now_v7().simple()));
    tokio::fs::write(&partial, bytes).await.map_err(io)?;
    tokio::fs::rename(&partial, path).await.map_err(io)
}

/// The cache's refs as `refs` says, and no others.
async fn write_refs(dir: &Path, refs: &[(String, String)]) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    for kind in ["refs/heads", "refs/tags", "refs/kuria"] {
        let path = dir.join(kind);
        if path.exists() {
            tokio::fs::remove_dir_all(&path).await.map_err(io)?;
        }
    }
    tokio::fs::create_dir_all(dir.join("refs/heads"))
        .await
        .map_err(io)?;
    let packed = refs.iter().fold(String::new(), |mut out, (name, target)| {
        let _ = writeln!(out, "{target} {name}");
        out
    });
    write_atomically(&dir.join("packed-refs"), packed.as_bytes()).await
}

/// The packfiles in the objects directory `objects`, as `(pack, index)`
/// file names.
pub(crate) async fn packs_in(objects: &Path) -> Result<Vec<(String, String)>, String> {
    let mut found = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(objects.join("pack")).await else {
        return Ok(found);
    };
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(stem) = name.strip_suffix(".pack") {
            found.push((name.clone(), format!("{stem}.idx")));
        }
    }
    found.sort();
    Ok(found)
}

/// Keeps the packfiles of the objects directory `objects` in storage, the
/// repository's own from then on: content-addressed, never changed.
pub(crate) async fn store_packs(
    state: &GitState,
    opened: &Opened,
    objects: &Path,
) -> Result<(), String> {
    let prefix = prefix(opened.tenant, opened.akt);
    for (pack, idx) in packs_in(objects).await? {
        // The pack before its index, as they are read.
        for file in [&pack, &idx] {
            let path = objects.join("pack").join(file);
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|e| format!("{}: {e}", path.display()))?;
            state
                .storage
                .put_named(&format!("{prefix}/{file}"), bytes)
                .await
                .map_err(|e| format!("storing {prefix}/{file}: {e}"))?;
        }
    }
    Ok(())
}

/// Moves the packfiles of the objects directory `objects` into the cache at
/// `dir`, once storage keeps them.
pub(crate) async fn install_packs(objects: &Path, dir: &Path) -> Result<(), String> {
    for (pack, idx) in packs_in(objects).await? {
        for file in [&pack, &idx] {
            let to = dir.join("objects/pack").join(file);
            tokio::fs::rename(objects.join("pack").join(file), &to)
                .await
                .map_err(|e| format!("{}: {e}", to.display()))?;
        }
    }
    Ok(())
}

/// A directory of the cache at `dir` for one request's objects, apart from
/// the repository's own: a push's quarantine, or a history being built.
pub(crate) fn scratch(dir: &Path) -> PathBuf {
    dir.join("kuria-scratch")
        .join(Uuid::now_v7().simple().to_string())
}

/// Removes a scratch directory, whatever it holds.
pub(crate) async fn forget(path: &Path) {
    if let Err(e) = tokio::fs::remove_dir_all(path).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("removing {}: {e}", path.display());
    }
}
