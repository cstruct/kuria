//! Archiving and unarchiving akter (`archive.*`): by a subject, through
//! `archiveAkt` and `unarchiveAkt`, or by an akt type's rules.

use chrono::{DateTime, Utc};
use diesel_async::AsyncConnection;
use kuria_store::akter::{self as store, AktRow, Archive, RevisionRow};
use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{AktError, Akter, Caller, StoredType, require, require_read, type_of};
use crate::authorize::Permission;

/// Who archives or unarchives an akt.
#[derive(Clone, Copy, Debug)]
pub(crate) enum By {
    /// A subject, through the API, and who really acted as them, if
    /// another (`akt.revision.actor`).
    Subject(Uuid, Option<Uuid>),
    /// One of its akt type's rules, by its kind (`archive.rules`), and for
    /// `with_container` the container it went with.
    Rule {
        kind: &'static str,
        container: Option<Uuid>,
    },
}

impl By {
    fn json(self) -> Json {
        match self {
            Self::Subject(id, _) => json!({ "subject": id }),
            Self::Rule {
                kind,
                container: None,
            } => json!({ "rule": kind }),
            Self::Rule {
                kind,
                container: Some(container),
            } => json!({ "rule": kind, "container": container }),
        }
    }

    /// The author of the revision it makes: the subject, or Kuria, whose
    /// subject is the nil id (`archive.history`).
    fn author(self) -> Uuid {
        match self {
            Self::Subject(id, _) => id,
            Self::Rule { .. } => Uuid::nil(),
        }
    }
}

/// Refuses a write to `akt` while it is archived (`archive.readonly`).
// r[impl archive.readonly]
pub(crate) fn writable(akt: &AktRow) -> Result<(), AktError> {
    if akt.archived {
        Err(AktError::Archived(akt.number.clone()))
    } else {
        Ok(())
    }
}

/// Whether a rule archived `akt`.
pub(crate) fn by_rule(akt: &AktRow) -> Option<String> {
    akt.archived_by
        .as_ref()
        .and_then(|by| by["rule"].as_str())
        .map(str::to_owned)
}

/// A reason given to archive or unarchive: one line of at most 200
/// characters, or none.
fn reason_of(reason: Option<String>) -> Result<Option<String>, AktError> {
    let reason = reason
        .map(|r| r.trim().to_owned())
        .filter(|r| !r.is_empty());
    if reason
        .as_ref()
        .is_some_and(|r| r.chars().count() > 200 || r.contains(['\n', '\r']))
    {
        return Err(AktError::BadRequest(
            "a reason is one line of at most 200 characters".to_owned(),
        ));
    }
    Ok(reason)
}

/// Archives `akt`, locked and live, or unarchives it, locked and archived:
/// a revision with the transition `archive` or `unarchive` and its change
/// event, then the move (`archive.history`, `archive.storage`). Answers the
/// akt as it then is.
// r[impl archive.history]
// r[impl archive.flag]
pub(crate) async fn set(
    conn: &mut kuria_store::Connection,
    mut akt: AktRow,
    ty: &StoredType,
    archive: bool,
    by: By,
    reason: Option<String>,
    now: DateTime<Utc>,
) -> Result<AktRow, AktError> {
    let tenant = akt.tenant_id;
    let number = akt.number.clone();
    let values = super::versions::values_of(ty, &akt)?;
    let expected = akt.revision;
    akt.revision += 1;
    akt.updated_at = now;
    // Unarchiving moves the akt back first, so its new revision is live;
    // archiving appends it first, then moves the akt with it.
    if !archive {
        // r[impl archive.rules.kept]
        let kept = matches!(by, By::Subject(..)) && by_rule(&akt).is_some();
        store::set_archived(conn, tenant, akt.id, None, kept).await?;
        akt.archived = false;
        akt.archived_at = None;
        akt.archived_by = None;
        akt.archive_reason = None;
        akt.archive_kept = kept;
    }
    if !store::update_akt(conn, &akt, expected).await? {
        return Err(AktError::Internal(format!(
            "akt {number} moved while locked"
        )));
    }
    let rule = match by {
        By::Rule { kind, .. } => Some(format!("by the rule `{kind}`")),
        By::Subject(..) => None,
    };
    let actor = match by {
        By::Subject(_, actor) => actor,
        By::Rule { .. } => None,
    };
    let revision = RevisionRow {
        akt_id: akt.id,
        revision: akt.revision,
        tenant_id: tenant,
        author: by.author(),
        transition: Some(if archive { "archive" } else { "unarchive" }.to_owned()),
        reason: match (rule, reason.clone()) {
            (Some(rule), Some(reason)) => Some(format!("{rule}: {reason}")),
            (rule, reason) => rule.or(reason),
        },
        changes: Json::Object(serde_json::Map::new()),
        state: akt.state.clone(),
        hash: kuria_engine::state_hash(&akt.state, &values).to_vec(),
        created_at: now,
        membership: None,
        actor,
    };
    store::append_revision(conn, &revision).await?;
    if archive {
        let archived = Archive {
            at: now,
            by: by.json(),
            reason: reason.clone(),
        };
        store::set_archived(conn, tenant, akt.id, Some(&archived), false).await?;
        akt.archived = true;
        akt.archived_at = Some(now);
        akt.archived_by = Some(archived.by);
        akt.archive_reason = reason;
        akt.archive_kept = false;
    }
    Ok(akt)
}

impl Akter {
    /// Archives the akt `number`, or with `archive` false unarchives it, as
    /// `caller` (`archive.act`, `archive.authz`).
    // r[impl archive.act]
    // r[impl archive.authz]
    pub async fn set_archived(
        &self,
        caller: Caller<'_>,
        number: &str,
        archive: bool,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        let reason = reason_of(reason)?;
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                let tenant: Uuid = caller.actor.tenant.into();
                let akt = store::lock_akt_by_number(conn, tenant, number)
                    .await?
                    .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
                let ty = type_of(conn, tenant, &akt).await?;
                require_read(caller.auth, &ty, number).await?;
                require(
                    caller.auth,
                    Permission::Archive {
                        key: ty.ty.key().to_owned(),
                    },
                )
                .await?;
                if akt.archived == archive {
                    return Err(AktError::BadRequest(format!(
                        "{number} is {} already",
                        if archive { "archived" } else { "live" }
                    )));
                }
                let (subject, acting) = caller.actor.by();
                let by = By::Subject(subject, acting);
                let now = Utc::now();
                let akt = set(conn, akt, &ty, archive, by, reason.clone(), now).await?;
                super::rules::follow(conn, tenant, &akt, &ty, now).await?;
                Ok(akt)
            })
            .await;
        self.committed(result)
    }

    /// Applies every archive rule to the akter already there, a tenant at a
    /// time, skipping a tenant another instance is sweeping (`archive.sweep`).
    /// Answers how many akter it archived.
    // r[impl archive.sweep]
    pub async fn sweep(&self, now: DateTime<Utc>) -> Result<usize, AktError> {
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let mut tenants: Vec<(Uuid, Vec<String>)> = Vec::new();
        for (tenant, key) in store::types_with_archive_rules(conn).await? {
            match tenants.last_mut() {
                Some((last, keys)) if *last == tenant => keys.push(key),
                _ => tenants.push((tenant, vec![key])),
            }
        }
        let mut archived = 0;
        for (tenant, keys) in tenants {
            let result = conn
                .transaction::<_, AktError, _>(async |conn| {
                    if !store::try_lock_sweep(conn, tenant).await? {
                        return Ok(0);
                    }
                    super::rules::sweep_tenant(conn, tenant, &keys, now).await
                })
                .await;
            archived += self.committed(result)?;
        }
        Ok(archived)
    }
}
