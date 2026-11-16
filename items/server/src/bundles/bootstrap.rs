//! A bootstrap bundle: imported into a fresh tenant as the server starts,
//! before it serves (`bundle.bootstrap.*`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use kuria_store::Pool;
use uuid::Uuid;

use super::BundleError;
use super::archive::{self, Unpacked};
use super::import::{Imported, Tenants, import_unpacked};
use crate::attachments::Storage;

/// The bundle to import before serving, and where (`KURIA_BOOTSTRAP_*`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bootstrap {
    /// A bundle's directory, or a file holding its tar archive
    /// (`KURIA_BOOTSTRAP_BUNDLE`).
    pub bundle: PathBuf,
    /// The tenant it goes into (`KURIA_BOOTSTRAP_TENANT`, else
    /// `KURIA_DEFAULT_TENANT`).
    pub tenant: Uuid,
    /// A JSON file mapping the bundle's subjects to the tenant's
    /// (`KURIA_BOOTSTRAP_SUBJECTS`).
    pub subjects: Option<PathBuf>,
}

/// Imports `what`'s bundle into its tenant, if that tenant is fresh, as
/// `POST /import` does, recorded as imported by the nil subject.
// r[impl bundle.bootstrap.import]
// r[impl bundle.bootstrap.recorded]
pub async fn bootstrap(
    pool: &Pool,
    storage: &Storage,
    what: &Bootstrap,
) -> Result<Imported, BundleError> {
    let mut unpacked = read(&what.bundle)?;
    if let Some(subjects) = &what.subjects {
        unpacked.subjects =
            Some(std::fs::read(subjects).map_err(|e| {
                BundleError::Invalid(format!("reading {}: {e}", subjects.display()))
            })?);
    }
    import_unpacked(
        pool,
        storage,
        what.tenant,
        Uuid::nil(),
        &unpacked,
        Tenants::Fresh,
    )
    .await
}

/// The bundle at `path`: a directory of `bundle.json` and
/// `attachments/<sha256>`, or a file holding its tar archive
/// (`bundle.format`).
// r[impl bundle.bootstrap.variables]
fn read(path: &Path) -> Result<Unpacked, BundleError> {
    let unreadable = |at: &Path, e: std::io::Error| {
        BundleError::Invalid(format!("reading {}: {e}", at.display()))
    };
    if !path.is_dir() {
        let archive = std::fs::read(path).map_err(|e| unreadable(path, e))?;
        return archive::unpack(&archive);
    }
    let json = path.join("bundle.json");
    let bundle = std::fs::read(&json).map_err(|e| unreadable(&json, e))?;
    let mut attachments = HashMap::new();
    let dir = path.join("attachments");
    if dir.is_dir() {
        for entry in std::fs::read_dir(&dir).map_err(|e| unreadable(&dir, e))? {
            let entry = entry.map_err(|e| unreadable(&dir, e))?;
            if !entry
                .file_type()
                .map_err(|e| unreadable(&dir, e))?
                .is_file()
            {
                continue;
            }
            let bytes = std::fs::read(entry.path()).map_err(|e| unreadable(&entry.path(), e))?;
            attachments.insert(
                entry.file_name().to_string_lossy().to_ascii_lowercase(),
                bytes,
            );
        }
    }
    Ok(Unpacked {
        bundle,
        attachments,
        subjects: None,
    })
}
