//! The one write path for akt types and akter.
//!
//! Each change loads what it needs, asks the engine, and writes the akt, its
//! revision and its change event in one transaction. No other code writes to
//! the akt tables.

use chrono::{DateTime, Datelike, Utc};
use diesel_async::AsyncConnection;
use konklav_core::{SubjectId, TenantId};
use kuria_config::{AktType, AktTypeDocument, Problem, validate};
use kuria_engine::{Current, Outcome, Refusal, fields_from_json, fields_to_json};
use kuria_store::Pool;
use kuria_store::akter::{self as store, AktRow, AktTypeVersion, RevisionRow};
use serde_json::{Map, Value as Json};
use uuid::Uuid;

/// Who is acting, and in which tenant.
#[derive(Clone, Copy, Debug)]
pub struct Actor {
    pub subject: SubjectId,
    pub tenant: TenantId,
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
    pub async fn define_type(&self, actor: Actor, document: Json) -> Result<StoredType, AktError> {
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

    /// Creates an akt of the akt type `key`.
    // r[impl akt.create]
    // r[impl akt.revision.atomic]
    pub async fn create(
        &self,
        actor: Actor,
        key: &str,
        fields: &Map<String, Json>,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        let tenant: Uuid = actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        conn.transaction::<_, AktError, _>(async |conn| {
            let row = store::latest_type_version(conn, tenant, key)
                .await?
                .ok_or_else(|| AktError::UnknownType(key.to_owned()))?;
            let stored = StoredType::read(row)?;
            let outcome = kuria_engine::create(&stored.ty, fields)?;
            let now = Utc::now();
            let number = next_number(conn, tenant, &stored.ty, now).await?;
            let akt = AktRow {
                id: Uuid::now_v7(),
                tenant_id: tenant,
                number,
                type_version_id: stored.row.id,
                state: outcome.state.clone(),
                fields: Json::Object(fields_to_json(&outcome.fields)),
                revision: 1,
                created_at: now,
                updated_at: now,
            };
            store::insert_akt(conn, &akt).await?;
            store::append_revision(conn, &revision(&akt, actor, None, reason, &outcome, now))
                .await?;
            Ok(akt)
        })
        .await
    }

    /// Edits the akt `number`, based on `expected_revision`.
    // r[impl akt.edit]
    pub async fn edit(
        &self,
        actor: Actor,
        number: &str,
        expected_revision: i32,
        fields: &Map<String, Json>,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        self.change(
            actor,
            number,
            expected_revision,
            None,
            reason,
            |ty, current| kuria_engine::edit(ty, current, fields),
        )
        .await
    }

    /// Performs `transition` on the akt `number`, based on
    /// `expected_revision`.
    // r[impl akt.transition]
    pub async fn transition(
        &self,
        actor: Actor,
        number: &str,
        expected_revision: i32,
        transition: &str,
        fields: &Map<String, Json>,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        self.change(
            actor,
            number,
            expected_revision,
            Some(transition.to_owned()),
            reason,
            |ty, current| kuria_engine::transition(ty, current, transition, fields),
        )
        .await
    }

    // r[impl akt.revision.concurrency]
    // r[impl akt.revision.atomic]
    // r[impl akt.version-pinned]
    async fn change(
        &self,
        actor: Actor,
        number: &str,
        expected_revision: i32,
        transition: Option<String>,
        reason: Option<String>,
        decide: impl FnOnce(&AktType, &Current) -> Result<Outcome, Refusal> + Send,
    ) -> Result<AktRow, AktError> {
        let tenant: Uuid = actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        conn.transaction::<_, AktError, _>(async |conn| {
            let mut akt = store::lock_akt_by_number(conn, tenant, number)
                .await?
                .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
            if akt.revision != expected_revision {
                return Err(AktError::Stale {
                    expected: expected_revision,
                    current: akt.revision,
                });
            }
            // The akt's own version, not the type's latest.
            let row = store::type_versions_by_id(conn, tenant, &[akt.type_version_id])
                .await?
                .pop()
                .ok_or_else(|| AktError::Internal(format!("akt {number} has no akt type")))?;
            let stored = StoredType::read(row)?;
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

/// The next akt number for `ty`, taken from the tenant's counter for the
/// pattern's scope.
// r[impl akt.number.sequence]
async fn next_number(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    ty: &AktType,
    now: DateTime<Utc>,
) -> Result<String, AktError> {
    let (year, month) = (now.year(), now.month());
    let scope = ty.number().scope(year, month);
    let sequence = store::next_counter(conn, tenant, &format!("number:{scope}")).await?;
    let sequence = u64::try_from(sequence).map_err(|e| AktError::Internal(e.to_string()))?;
    Ok(ty.number().render(year, month, sequence))
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
    }
}
