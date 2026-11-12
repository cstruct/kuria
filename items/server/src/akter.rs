//! The one write path for akt types and akter.
//!
//! Each change loads what it needs, asks the engine, and writes the akt, its
//! revision and its change event in one transaction. No other code writes to
//! the akt tables.
//!
//! - [`create`]: creating akter, also inside containers.
//! - [`membership`]: adding members to containers and removing them.
//! - [`ordering`]: moving akter in their tenant's order.
//! - [`draft`]: drafting a field from the akter around its akt.
//!
//! Locks are always taken in one order, so no two changes deadlock: the
//! tenant's order lock, then akt rows (by id), then number counters, then
//! the change-event counter.

mod create;
mod draft;
mod membership;
mod ordering;

pub use ordering::Place;

use chrono::{DateTime, Datelike, Utc};
use diesel_async::AsyncConnection;
use konklav_core::{SubjectId, TenantId};
use kuria_config::{AktType, AktTypeDocument, NumberContext, Problem, validate};

use crate::authorize::{Authorizer, Permission, Unavailable};
use kuria_engine::{Current, Outcome, Refusal, fields_from_json, fields_to_json, rank_between};
use kuria_store::Pool;
use kuria_store::akter::{self as store, AktRow, AktTypeVersion, RevisionRow};
use kuria_store::containers;
use serde_json::{Map, Value as Json};
use uuid::Uuid;

/// Who is acting, and in which tenant.
#[derive(Clone, Copy, Debug)]
pub struct Actor {
    pub subject: SubjectId,
    pub tenant: TenantId,
}

/// Who is acting, with what decides their permissions.
#[derive(Clone, Copy)]
pub struct Caller<'a> {
    pub actor: Actor,
    pub auth: &'a Authorizer,
}

/// Why a change to akt types or akter failed.
#[derive(Debug, thiserror::Error)]
pub enum AktError {
    #[error("the akt type document is invalid")]
    InvalidType(Vec<Problem>),
    #[error("the akt type document can't be read: {0}")]
    UnreadableType(String),
    #[error("there is no akt type `{0}`")]
    UnknownType(String),
    #[error("there is no akt `{0}`")]
    NotFound(String),
    #[error("{0}")]
    Refused(#[from] Refusal),
    #[error("the akt is at revision {current}, not {expected}")]
    Stale { expected: i32, current: i32 },
    #[error("not permitted")]
    Forbidden,
    #[error("`{container}` can't contain akter of the type `{member}`")]
    NotContainable { container: String, member: String },
    #[error("`{member}` is already a member of `{container}`")]
    AlreadyMember { container: String, member: String },
    #[error("`{member}` is not a member of `{container}`")]
    NotMember { container: String, member: String },
    #[error("`{member}` is already in `{other}`, and can be in only one `{akt_type}`")]
    Exclusive {
        member: String,
        other: String,
        akt_type: String,
    },
    #[error("`{member}` contains `{container}`, so it can't also be in it")]
    Cycle { container: String, member: String },
    #[error("`{0}` is in a final state, so its members can't change")]
    Frozen(String),
    #[error("an akt of the type `{akt_type}` must be created in one of: {}", .allowed.join(", "))]
    CreatedIn {
        akt_type: String,
        allowed: Vec<String>,
    },
    #[error("an akt can't be moved next to itself")]
    SelfMove,
    #[error("the field `{0}` has no draft recipe")]
    NotDraftable(String),
    #[error("{0}")]
    Unavailable(#[from] Unavailable),
    #[error("internal error")]
    Internal(String),
}

impl From<diesel::result::Error> for AktError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<diesel_async::pooled_connection::deadpool::PoolError> for AktError {
    fn from(error: diesel_async::pooled_connection::deadpool::PoolError) -> Self {
        Self::Internal(error.to_string())
    }
}

/// An akt type version, read and validated.
#[derive(Clone, Debug)]
pub struct StoredType {
    pub row: AktTypeVersion,
    pub ty: AktType,
}

impl StoredType {
    /// Reads a stored version. Stored definitions were validated when they
    /// were submitted, so one that no longer validates is an internal error.
    pub fn read(row: AktTypeVersion) -> Result<Self, AktError> {
        let document: AktTypeDocument = serde_json::from_value(row.definition.clone())
            .map_err(|e| AktError::Internal(format!("stored akt type {}: {e}", row.key)))?;
        let ty = validate(document).map_err(|problems| {
            AktError::Internal(format!(
                "stored akt type {} no longer validates: {problems:?}",
                row.key
            ))
        })?;
        Ok(Self { row, ty })
    }
}

/// Akt types and akter, in Postgres.
#[derive(Clone)]
pub struct Akter {
    pool: Pool,
}

impl Akter {
    #[must_use]
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    #[must_use]
    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Validates `document` and stores it as the next version of its key.
    // r[impl akt.type.document]
    // r[impl akt.type.tenant]
    // r[impl authz.define]
    pub async fn define_type(
        &self,
        caller: Caller<'_>,
        document: Json,
    ) -> Result<StoredType, AktError> {
        let Caller { actor, auth } = caller;
        if !auth.allows(&Permission::DefineTypes).await? {
            return Err(AktError::Forbidden);
        }
        let parsed: AktTypeDocument = serde_json::from_value(document)
            .map_err(|e| AktError::UnreadableType(e.to_string()))?;
        let ty = validate(parsed).map_err(AktError::InvalidType)?;
        // Stored as it validated, so what is kept is exactly what was checked.
        let definition =
            serde_json::to_value(ty.document()).map_err(|e| AktError::Internal(e.to_string()))?;
        let mut conn = self.pool.get().await?;
        let row = store::insert_type_version(
            &mut conn,
            actor.tenant.into(),
            ty.key(),
            &definition,
            actor.subject.into(),
            Utc::now(),
        )
        .await?;
        Ok(StoredType { row, ty })
    }

    /// Edits the akt `number`, based on `expected_revision`.
    // r[impl akt.edit]
    // r[impl authz.edit]
    pub async fn edit(
        &self,
        caller: Caller<'_>,
        number: &str,
        expected_revision: i32,
        fields: &Map<String, Json>,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        self.change(
            caller,
            number,
            expected_revision,
            None,
            reason,
            |ty| Permission::Edit {
                key: ty.key().to_owned(),
            },
            |ty, current| kuria_engine::edit(ty, current, fields),
        )
        .await
    }

    /// Performs `transition` on the akt `number`, based on
    /// `expected_revision`.
    // r[impl akt.transition]
    // r[impl authz.transition]
    pub async fn transition(
        &self,
        caller: Caller<'_>,
        number: &str,
        expected_revision: i32,
        transition: &str,
        fields: &Map<String, Json>,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        self.change(
            caller,
            number,
            expected_revision,
            Some(transition.to_owned()),
            reason,
            |ty| Permission::Transition {
                key: ty.key().to_owned(),
                action: ty.transition(transition).and_then(|t| t.action.clone()),
            },
            |ty, current| kuria_engine::transition(ty, current, transition, fields),
        )
        .await
    }

    // r[impl akt.revision.concurrency]
    // r[impl akt.revision.atomic]
    // r[impl akt.version-pinned]
    // r[impl authz.read]
    #[allow(
        clippy::too_many_arguments,
        reason = "the one internal path for edits and transitions"
    )]
    async fn change(
        &self,
        caller: Caller<'_>,
        number: &str,
        expected_revision: i32,
        transition: Option<String>,
        reason: Option<String>,
        permission: impl FnOnce(&AktType) -> Permission + Send,
        decide: impl FnOnce(&AktType, &Current) -> Result<Outcome, Refusal> + Send,
    ) -> Result<AktRow, AktError> {
        let Caller { actor, auth } = caller;
        let tenant: Uuid = actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        conn.transaction::<_, AktError, _>(async |conn| {
            let mut akt = store::lock_akt_by_number(conn, tenant, number)
                .await?
                .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
            // The akt's own version, not the type's latest.
            let row = store::type_versions_by_id(conn, tenant, &[akt.type_version_id])
                .await?
                .pop()
                .ok_or_else(|| AktError::Internal(format!("akt {number} has no akt type")))?;
            let stored = StoredType::read(row)?;
            // An akt the subject can't read doesn't exist for them, so this
            // comes before anything that would tell them otherwise.
            if !auth
                .allows(&Permission::Read {
                    key: stored.ty.key().to_owned(),
                })
                .await?
            {
                return Err(AktError::NotFound(number.to_owned()));
            }
            if !auth.allows(&permission(&stored.ty)).await? {
                return Err(AktError::Forbidden);
            }
            if akt.revision != expected_revision {
                return Err(AktError::Stale {
                    expected: expected_revision,
                    current: akt.revision,
                });
            }
            let current = Current {
                state: akt.state.clone(),
                fields: fields_from_json(&stored.ty, akt.fields.as_object().unwrap_or(&Map::new()))
                    .map_err(|e| AktError::Internal(format!("akt {number}: {e}")))?,
            };
            let outcome = decide(&stored.ty, &current)?;
            let now = Utc::now();
            akt.state = outcome.state.clone();
            akt.fields = Json::Object(fields_to_json(&outcome.fields));
            akt.revision = expected_revision + 1;
            akt.updated_at = now;
            if !store::update_akt(conn, &akt, expected_revision).await? {
                return Err(AktError::Internal(format!(
                    "akt {number} moved while locked"
                )));
            }
            store::append_revision(
                conn,
                &revision(&akt, actor, transition, reason, &outcome, now),
            )
            .await?;
            Ok(akt)
        })
        .await
    }
}

/// A rank after every akt of the tenant. The caller holds the tenant's
/// order lock.
// r[impl order.rank]
async fn rank_after_last(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
) -> Result<String, AktError> {
    let last = containers::last_rank(conn, tenant).await?;
    rank_between(last.as_deref(), None).map_err(|e| AktError::Internal(e.to_string()))
}

/// The next akt number for `ty`, taken from the tenant's counter for the
/// pattern's scope.
// r[impl akt.number.sequence]
async fn next_number(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    ty: &AktType,
    parent: Option<&str>,
    now: DateTime<Utc>,
) -> Result<String, AktError> {
    let context = NumberContext {
        year: now.year(),
        month: now.month(),
        parent,
    };
    let scope = ty.number().scope(&context);
    let sequence = store::next_counter(conn, tenant, &format!("number:{scope}")).await?;
    let sequence = u64::try_from(sequence).map_err(|e| AktError::Internal(e.to_string()))?;
    Ok(ty.number().render(&context, sequence))
}

/// The revision recording `outcome` on `akt`.
// r[impl akt.revision.record]
// r[impl akt.change-event]
fn revision(
    akt: &AktRow,
    actor: Actor,
    transition: Option<String>,
    reason: Option<String>,
    outcome: &Outcome,
    now: DateTime<Utc>,
) -> RevisionRow {
    let changes: Map<String, Json> = outcome
        .changes
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                value
                    .as_ref()
                    .map_or(Json::Null, kuria_engine::Value::to_json),
            )
        })
        .collect();
    RevisionRow {
        akt_id: akt.id,
        revision: akt.revision,
        tenant_id: akt.tenant_id,
        author: actor.subject.into(),
        transition,
        reason: reason.filter(|r| !r.trim().is_empty()),
        changes: Json::Object(changes),
        state: outcome.state.clone(),
        hash: outcome.hash.to_vec(),
        created_at: now,
        membership: None,
    }
}

/// The akt type version `akt` was created with.
async fn type_of(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    akt: &AktRow,
) -> Result<StoredType, AktError> {
    let row = store::type_versions_by_id(conn, tenant, &[akt.type_version_id])
        .await?
        .pop()
        .ok_or_else(|| AktError::Internal(format!("akt {} has no akt type", akt.number)))?;
    StoredType::read(row)
}

/// Refuses an akt the caller may not read as if it didn't exist.
// r[impl authz.read]
async fn require_read(auth: &Authorizer, ty: &StoredType, number: &str) -> Result<(), AktError> {
    if auth
        .allows(&Permission::Read {
            key: ty.ty.key().to_owned(),
        })
        .await?
    {
        Ok(())
    } else {
        Err(AktError::NotFound(number.to_owned()))
    }
}

/// Refuses a permission the caller lacks.
async fn require(auth: &Authorizer, permission: Permission) -> Result<(), AktError> {
    if auth.allows(&permission).await? {
        Ok(())
    } else {
        Err(AktError::Forbidden)
    }
}

/// The hash of `akt`'s current state and field values.
fn current_hash(ty: &StoredType, akt: &AktRow) -> Result<[u8; 32], AktError> {
    let fields = fields_from_json(&ty.ty, akt.fields.as_object().unwrap_or(&Map::new()))
        .map_err(|e| AktError::Internal(format!("akt {}: {e}", akt.number)))?;
    Ok(kuria_engine::state_hash(&akt.state, &fields))
}

/// Whether `akt` is in a final state of its akt type.
fn is_final(ty: &StoredType, akt: &AktRow) -> bool {
    ty.ty.state(&akt.state).is_some_and(|s| s.is_final)
}

/// One side of a membership, as a membership revision records it.
fn membership_entry(akt: &AktRow) -> Json {
    serde_json::json!({ "akt": akt.id.to_string(), "number": akt.number })
}

/// Appends a membership revision to `akt`, which changes neither its state
/// nor its fields, and moves its current revision on.
// r[impl container.revisions]
async fn record_membership(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &mut AktRow,
    actor: Actor,
    reason: Option<String>,
    membership: Json,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    let hash = current_hash(ty, akt)?;
    let expected = akt.revision;
    akt.revision += 1;
    akt.updated_at = now;
    if !store::update_akt(conn, akt, expected).await? {
        return Err(AktError::Internal(format!(
            "akt {} moved while locked",
            akt.number
        )));
    }
    store::append_revision(
        conn,
        &RevisionRow {
            akt_id: akt.id,
            revision: akt.revision,
            tenant_id: akt.tenant_id,
            author: actor.subject.into(),
            transition: None,
            reason: reason.filter(|r| !r.trim().is_empty()),
            changes: Json::Object(Map::new()),
            state: akt.state.clone(),
            hash: hash.to_vec(),
            created_at: now,
            membership: Some(membership),
        },
    )
    .await?;
    Ok(())
}
