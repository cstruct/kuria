//! Where a profile's session is kept: the DPoP key and the tokens, in the
//! operating system's credential store, else in a file only the user may
//! read.
//!
//! `KURIA_SECRET_STORE=file` keeps them in the file even where there is a
//! credential store, such as in tests and on a headless machine.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CliError;
use crate::profile::io_error;

/// The credential store's service name.
const SERVICE: &str = "kuria";

/// A saved session, never printed or written anywhere but its store.
#[derive(Clone, Serialize, Deserialize)]
pub struct Secrets {
    /// The DPoP key's private scalar, base64url-encoded.
    pub dpop_key: String,
    pub refresh_token: Option<String>,
    pub access_token: Option<String>,
    /// When the access token expires, in Unix seconds.
    pub expires_at: Option<i64>,
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secrets { .. }")
    }
}

/// Where secrets go.
pub struct Store {
    dir: PathBuf,
    keychain: bool,
}

impl Store {
    /// The store for the client's directory `dir`.
    #[must_use]
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_owned(),
            keychain: std::env::var("KURIA_SECRET_STORE").as_deref() != Ok("file"),
        }
    }

    fn file(&self, profile: &str) -> PathBuf {
        self.dir.join("secrets").join(format!("{profile}.json"))
    }

    fn entry(profile: &str) -> Option<keyring::Entry> {
        keyring::Entry::new(SERVICE, profile).ok()
    }

    /// The secrets saved for `profile`, if any.
    // r[impl cli.secret]
    pub fn load(&self, profile: &str) -> Result<Option<Secrets>, CliError> {
        if self.keychain
            && let Some(entry) = Self::entry(profile)
            && let Ok(text) = entry.get_password()
        {
            return parse(&text).map(Some);
        }
        let path = self.file(profile);
        match std::fs::read_to_string(&path) {
            Ok(text) => parse(&text).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_error(&path, &e)),
        }
    }

    /// Saves `secrets` for `profile`: in the credential store if it takes
    /// them, else in the file.
    // r[impl cli.secret]
    pub fn save(&self, profile: &str, secrets: &Secrets) -> Result<(), CliError> {
        let text = serde_json::to_string(secrets).expect("serializable");
        if self.keychain
            && let Some(entry) = Self::entry(profile)
            && entry.set_password(&text).is_ok()
        {
            // Nothing stale left in a file from before.
            self.remove_file(profile)?;
            return Ok(());
        }
        write_private(&self.file(profile), &text)
    }

    /// Forgets `profile`'s secrets, wherever they are.
    pub fn forget(&self, profile: &str) -> Result<(), CliError> {
        if self.keychain
            && let Some(entry) = Self::entry(profile)
        {
            // Nothing saved there is nothing to forget.
            drop(entry.delete_credential());
        }
        self.remove_file(profile)
    }

    fn remove_file(&self, profile: &str) -> Result<(), CliError> {
        let path = self.file(profile);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_error(&path, &e)),
        }
    }
}

fn parse(text: &str) -> Result<Secrets, CliError> {
    serde_json::from_str(text).map_err(|_| {
        CliError::not_signed_in("The saved session can't be read; run `kuria login` again")
    })
}

/// Writes `text` to `path`, readable by the user only, in a directory only
/// the user may enter.
fn write_private(path: &Path, text: &str) -> Result<(), CliError> {
    use std::io::Write as _;
    let dir = path.parent().expect("a file in a directory");
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir).map_err(|e| io_error(dir, &e))?;
    // Written whole, then renamed over the old one.
    let partial = path.with_extension("json.partial");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&partial).map_err(|e| io_error(&partial, &e))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| io_error(&partial, &e))?;
    std::fs::rename(&partial, path).map_err(|e| io_error(path, &e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secrets() -> Secrets {
        Secrets {
            dpop_key: "key".to_owned(),
            refresh_token: Some("refresh".to_owned()),
            access_token: None,
            expires_at: None,
        }
    }

    fn file_store(dir: &Path) -> Store {
        Store {
            dir: dir.to_owned(),
            keychain: false,
        }
    }

    // r[verify cli.secret]
    #[test]
    fn keeps_secrets_in_a_file_only_the_user_may_read() {
        let dir = tempfile::tempdir().unwrap();
        let store = file_store(dir.path());
        assert!(store.load("default").unwrap().is_none());
        store.save("default", &secrets()).unwrap();
        let loaded = store.load("default").unwrap().unwrap();
        assert_eq!(loaded.refresh_token.as_deref(), Some("refresh"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&store.file("default")), 0o600);
            assert_eq!(mode(&dir.path().join("secrets")), 0o700);
        }
        store.forget("default").unwrap();
        assert!(store.load("default").unwrap().is_none());
        store.forget("default").unwrap();
    }

    // r[verify cli.secret]
    #[test]
    fn never_shows_secrets_in_debug_output() {
        assert_eq!(format!("{:?}", secrets()), "Secrets { .. }");
    }
}
