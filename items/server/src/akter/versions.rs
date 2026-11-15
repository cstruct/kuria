//! Versions (`version.*`): releasing an akt's current revision as its next
//! version, and reading an akt as it was at a revision.

use chrono::Utc;
use diesel_async::AsyncConnection;
use kuria_engine::markdown::{self, Target, TargetVersion};
use kuria_engine::versions;
use kuria_store::akter::{self as store, AktRow};
use kuria_store::containers;
use kuria_store::versions::{self as stored, VersionRow};
use serde_json::{Map, Value as Json};
use uuid::Uuid;

use super::{AktError, Akter, Caller, StoredType, require, require_read, type_of};
use crate::authorize::{Authorizer, Permission};

/// An akt's fields and state as they were at `revision`, replayed from its
/// revisions; `None` if it never reached it.
pub async fn at_revision(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    akt: Uuid,
    revision: i32,
) -> Result<Option<(Map<String, Json>, String)>, AktError> {
    let mut fields = Map::new();
    let mut state = None;
    for row in store::revisions(conn, tenant, akt).await? {
        if row.revision > revision {
            break;
        }
        if let Some(changes) = row.changes.as_object() {
            for (key, value) in changes {
                if value.is_null() {
                    fields.remove(key);
                } else {
                    fields.insert(key.clone(), value.clone());
                }
            }
        }
        state = Some(row.state.clone());
    }
    Ok(state.map(|state| (fields, state)))
}

/// `akt`'s row as it was at `revision`: its fields, state and title then;
/// `None` if it never reached it.
pub async fn at(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
    revision: i32,
) -> Result<Option<AktRow>, AktError> {
    let Some((fields, state)) = at_revision(conn, akt.tenant_id, akt.id, revision).await? else {
        return Ok(None);
    };
    let values = kuria_engine::fields_from_json(&ty.ty, &fields)
        .map_err(|e| AktError::Internal(format!("akt {}: {e}", akt.number)))?;
    let mut row = akt.clone();
    row.title = super::title_of(&ty.ty, &values);
    row.fields = Json::Object(fields);
    row.state = state;
    row.revision = revision;
    Ok(Some(row))
}

/// `akt` as it was at its version `number`: its row with the fields, state
/// and title of that revision, and the version; `None` if it has no such
/// version (`version.read`).
// r[impl version.read]
pub async fn at_version(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
    number: i32,
) -> Result<Option<(AktRow, VersionRow)>, AktError> {
    let Some(version) = stored::one(conn, akt.tenant_id, akt.id, number).await? else {
        return Ok(None);
    };
    let Some((fields, state)) = at_revision(conn, akt.tenant_id, akt.id, version.revision).await?
    else {
        return Ok(None);
    };
    let values = kuria_engine::fields_from_json(&ty.ty, &fields)
        .map_err(|e| AktError::Internal(format!("akt {}: {e}", akt.number)))?;
    let mut row = akt.clone();
    row.title = super::title_of(&ty.ty, &values);
    row.fields = Json::Object(fields);
    row.state = state;
    row.revision = version.revision;
    Ok(Some((row, version)))
}

/// The akt type version `akt` was created with.
pub async fn type_of_akt(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    akt: &AktRow,
) -> Result<StoredType, AktError> {
    type_of(conn, tenant, akt).await
}

/// What the akt `number` has for a reference to it, as a subject `auth`
/// allows to read it: `None` if it doesn't exist or they may not read it.
pub async fn target(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    auth: &Authorizer,
    number: &str,
) -> Result<Option<Target>, AktError> {
    let Some(akt) = store::akt_by_number(conn, tenant, number).await? else {
        return Ok(None);
    };
    let ty = type_of(conn, tenant, &akt).await?;
    if require_read(auth, &ty, number).await.is_err() {
        return Ok(None);
    }
    let headings = |ty: &StoredType, fields: &Map<String, Json>| -> Result<Vec<String>, AktError> {
        let values = kuria_engine::fields_from_json(&ty.ty, fields)
            .map_err(|e| AktError::Internal(format!("akt {number}: {e}")))?;
        Ok(markdown::heading_ids(&ty.ty, &values))
    };
    let mut found = Target {
        headings: headings(&ty, akt.fields.as_object().unwrap_or(&Map::new()))?,
        versions: Vec::new(),
        revision: akt.revision,
    };
    let mut rows = stored::of(conn, tenant, akt.id).await?;
    rows.reverse();
    for version in rows {
        let at = at_revision(conn, tenant, akt.id, version.revision)
            .await?
            .map(|(fields, _)| fields)
            .unwrap_or_default();
        found.versions.push(TargetVersion {
            number: u32::try_from(version.number).unwrap_or_default(),
            revision: version.revision,
            headings: headings(&ty, &at)?,
        });
    }
    Ok(Some(found))
}

impl Akter {
    /// Releases the akt `number`'s current revision, which must be
    /// `revision`, as its next version, and returns the version.
    // r[impl version.release]
    // r[impl version.number]
    // r[impl authz.release]
    pub async fn release(
        &self,
        caller: Caller<'_>,
        number: &str,
        revision: i32,
        label: Option<String>,
    ) -> Result<VersionRow, AktError> {
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                release_in(conn, caller, number, revision, label.clone()).await
            })
            .await;
        self.committed(result)
    }
}

async fn release_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    number: &str,
    revision: i32,
    label: Option<String>,
) -> Result<VersionRow, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let akt = containers::lock_akter(conn, tenant, &[number])
        .await?
        .pop()
        .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
    let ty = type_of(conn, tenant, &akt).await?;
    require_read(caller.auth, &ty, number).await?;
    let Some(declared) = &ty.ty.document().versions else {
        return Err(AktError::BadRequest(format!(
            "`{}` akter don't release versions",
            ty.ty.key()
        )));
    };
    require(
        caller.auth,
        Permission::Release {
            key: ty.ty.key().to_owned(),
            action: declared.action.clone(),
        },
    )
    .await?;
    if akt.revision != revision {
        return Err(AktError::Stale {
            expected: revision,
            current: akt.revision,
        });
    }
    let label = label.map(|l| l.trim().to_owned()).filter(|l| !l.is_empty());
    if label
        .as_ref()
        .is_some_and(|l| l.chars().count() > 100 || l.contains(['\n', '\r']))
    {
        return Err(AktError::BadRequest(
            "a label is one line of at most 100 characters".to_owned(),
        ));
    }
    let existing = stored::of(conn, tenant, akt.id).await?;
    if existing.iter().any(|v| v.revision == revision) {
        return Err(AktError::BadRequest(format!(
            "revision {revision} of {number} is already a version"
        )));
    }
    let fields =
        kuria_engine::fields_from_json(&ty.ty, akt.fields.as_object().unwrap_or(&Map::new()))
            .map_err(|e| AktError::Internal(format!("akt {number}: {e}")))?;
    // The targets first, since the check can't wait on the database.
    let mut targets = std::collections::HashMap::new();
    for (_, reference) in versions::references_of(&ty.ty, &fields) {
        if !targets.contains_key(&reference.number) {
            let found = target(conn, tenant, caller.auth, &reference.number).await?;
            targets.insert(reference.number.clone(), found);
        }
    }
    let pins = versions::check(&ty.ty, &fields, &|n| targets.get(n).cloned().flatten()).map_err(
        |broken| AktError::BrokenReferences(broken.iter().map(ToString::to_string).collect()),
    )?;
    let row = VersionRow {
        tenant_id: tenant,
        akt_id: akt.id,
        number: existing.first().map_or(1, |v| v.number + 1),
        revision,
        label,
        released_by: caller.actor.subject.into(),
        released_at: Utc::now(),
        pins: serde_json::to_value(pins).map_err(|e| AktError::Internal(e.to_string()))?,
    };
    stored::insert(conn, &row).await?;
    Ok(row)
}
