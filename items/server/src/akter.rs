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
//! - [`batch`]: several of these in one transaction.
//!
//! Locks are always taken in one order, so no two changes deadlock: the
//! tenant's order lock, then akt rows (by id), then number counters, then
//! the change-event counter.

mod batch;
mod copy;
mod create;
pub mod diagrams;
mod draft;
mod links;
mod membership;
mod ordering;
pub mod outcomes;
mod references;
pub mod versions;

pub(crate) use batch::batch_in;
pub use batch::{BatchError, MOST as BATCH_MOST, Operation};
pub use ordering::Place;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use diesel_async::AsyncConnection;
use konklav_core::{SubjectId, TenantId};
use kuria_config::{
    AktType, AktTypeDocument, AttachmentFormat, FieldKind, Problem, check_field_references,
    check_state_references, validate,
};
use tokio::sync::Notify;

use crate::authorize::{Authorizer, Permission, Unavailable};
use kuria_engine::rules::{AktRef, Broken, numbering, records};
use kuria_engine::{Current, Outcome, Refusal, fields_from_json, fields_to_json, rank_between};
use kuria_store::Pool;
use kuria_store::akter::{self as store, AktRow, AktTypeVersion, RevisionRow};
use kuria_store::{attachments, containers};
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
    /// Where a write keeps the diagrams it renders
    /// (`akt.field.markdown.diagrams`); without it, a write that renders
    /// one is refused.
    pub storage: Option<&'a crate::attachments::Storage>,
    /// What renders diagrams; without it, a write that renders one is
    /// refused.
    pub renderer: Option<&'a kuria_typst::Renderer>,
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
    #[error("diagram {block} in `{field}` doesn't compile: {message}")]
    InvalidDiagram {
        field: String,
        block: usize,
        message: String,
    },
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
    #[error("`{container}` doesn't take new `{member_type}` akter while `{state}`")]
    NotAccepting {
        container: String,
        member_type: String,
        state: String,
    },
    #[error("an akt of the type `{akt_type}` must be created in one of: {}", .allowed.join(", "))]
    CreatedIn {
        akt_type: String,
        allowed: Vec<String>,
    },
    #[error("an akt can't be moved next to itself")]
    SelfMove,
    #[error("{0}")]
    InvalidLink(String),
    #[error("`{source_number}` has no `{kind}` link to `{target}`")]
    NotLinked {
        source_number: String,
        kind: String,
        target: String,
    },
    #[error("the field `{0}` has no draft recipe")]
    NotDraftable(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("the release has broken references: {}", .0.join("; "))]
    BrokenReferences(Vec<String>),
    #[error("{0}")]
    Unavailable(#[from] Unavailable),
    #[error("internal error")]
    Internal(String),
}

impl From<Broken> for AktError {
    /// The engine's containment, link and ordering refusals, as the codes
    /// the server has always answered.
    fn from(broken: Broken) -> Self {
        match broken {
            Broken::NotContainable { container, member } => {
                Self::NotContainable { container, member }
            }
            Broken::AlreadyMember { container, member } => {
                Self::AlreadyMember { container, member }
            }
            Broken::NotMember { container, member } => Self::NotMember { container, member },
            Broken::Exclusive {
                member,
                other,
                akt_type,
            } => Self::Exclusive {
                member,
                other,
                akt_type,
            },
            Broken::Cycle { container, member } => Self::Cycle { container, member },
            Broken::Frozen(number) => Self::Frozen(number),
            Broken::NotAccepting {
                container,
                member_type,
                state,
            } => Self::NotAccepting {
                container,
                member_type,
                state,
            },
            Broken::CreatedIn { akt_type, allowed } => Self::CreatedIn { akt_type, allowed },
            Broken::InvalidLink(reason) => Self::InvalidLink(reason),
            Broken::NotLinked {
                source_number,
                kind,
                target,
            } => Self::NotLinked {
                source_number,
                kind,
                target,
            },
            Broken::SelfMove => Self::SelfMove,
        }
    }
}

/// `akt` as the engine's rules see it.
pub(crate) fn akt_ref<'a>(ty: &'a StoredType, akt: &'a AktRow) -> AktRef<'a> {
    AktRef {
        number: &akt.number,
        state: &akt.state,
        ty: &ty.ty,
        revision: akt.revision,
    }
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
    /// Where rendered diagrams are kept.
    storage: Option<crate::attachments::Storage>,
    /// What renders diagrams.
    renderer: Option<kuria_typst::Renderer>,
    /// Woken after a change commits, so its change event is published at
    /// once.
    published: Option<Arc<Notify>>,
}

impl Akter {
    #[must_use]
    pub fn new(pool: Pool, published: Option<Arc<Notify>>) -> Self {
        Self {
            pool,
            storage: None,
            renderer: None,
            published,
        }
    }

    /// The same, keeping rendered diagrams in `storage`.
    #[must_use]
    pub fn with_storage(mut self, storage: crate::attachments::Storage) -> Self {
        self.storage = Some(storage);
        self
    }

    /// The same, rendering diagrams with `renderer`.
    #[must_use]
    pub fn with_renderer(mut self, renderer: kuria_typst::Renderer) -> Self {
        self.renderer = Some(renderer);
        self
    }

    /// What renders diagrams, if anything.
    #[must_use]
    pub fn renderer(&self) -> Option<&kuria_typst::Renderer> {
        self.renderer.as_ref()
    }

    /// Where rendered diagrams are kept, if anywhere.
    #[must_use]
    pub fn storage(&self) -> Option<&crate::attachments::Storage> {
        self.storage.as_ref()
    }

    /// Wakes the relay: a change has committed.
    fn committed<T>(&self, result: Result<T, AktError>) -> Result<T, AktError> {
        if result.is_ok()
            && let Some(published) = &self.published
        {
            published.notify_one();
        }
        result
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
        let Caller { actor, auth, .. } = caller;
        if !auth.allows(&Permission::DefineTypes).await? {
            return Err(AktError::Forbidden);
        }
        let (ty, definition) = read_document(document)?;
        let tenant: Uuid = actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        conn.transaction::<_, AktError, _>(async |conn| {
            define_in(conn, tenant, actor.subject.into(), ty, &definition).await
        })
        .await
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
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                edit_in(
                    conn,
                    caller,
                    number,
                    expected_revision,
                    fields,
                    reason.clone(),
                )
                .await
            })
            .await;
        self.committed(result)
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
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                transition_in(
                    conn,
                    caller,
                    number,
                    expected_revision,
                    transition,
                    fields,
                    reason.clone(),
                )
                .await
            })
            .await;
        self.committed(result)
    }
}

/// The body of [`Akter::edit`], in the transaction `conn` is in.
pub(crate) async fn edit_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    number: &str,
    expected_revision: i32,
    fields: &Map<String, Json>,
    reason: Option<String>,
) -> Result<AktRow, AktError> {
    change_in(
        conn,
        caller,
        number,
        expected_revision,
        None,
        reason,
        |ty| Permission::Edit {
            key: ty.key().to_owned(),
        },
        |ty, current, change, _| kuria_engine::edit(ty, current, fields, change),
    )
    .await
}

/// The body of [`Akter::transition`], in the transaction `conn` is in.
pub(crate) async fn transition_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    number: &str,
    expected_revision: i32,
    transition: &str,
    fields: &Map<String, Json>,
    reason: Option<String>,
) -> Result<AktRow, AktError> {
    change_in(
        conn,
        caller,
        number,
        expected_revision,
        Some(transition.to_owned()),
        reason,
        |ty| Permission::Transition {
            key: ty.key().to_owned(),
            action: ty.transition(transition).and_then(|t| t.action.clone()),
        },
        |ty, current, change, held| {
            kuria_engine::transition(ty, current, transition, fields, change, held)
        },
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
async fn change_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    number: &str,
    expected_revision: i32,
    transition: Option<String>,
    reason: Option<String>,
    permission: impl FnOnce(&AktType) -> Permission + Send,
    decide: impl FnOnce(
        &AktType,
        &Current,
        kuria_engine::Change,
        Option<kuria_engine::outcomes::Outcome>,
    ) -> Result<Outcome, Refusal>
    + Send,
) -> Result<AktRow, AktError> {
    let Caller { actor, auth, .. } = caller;
    let tenant: Uuid = actor.tenant.into();
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
    // r[impl akt.transition.outcomes]
    let held = match transition.as_deref().and_then(|t| stored.ty.transition(t)) {
        Some(def) if !def.outcomes.is_empty() => {
            outcomes::held_locked(conn, tenant, &stored.ty, &akt).await?
        }
        _ => None,
    };
    let now = Utc::now();
    let outcome = decide(
        &stored.ty,
        &current,
        kuria_engine::Change {
            by: actor.subject.into(),
            at: now,
        },
        held,
    )?;
    akt.state = outcome.state.clone();
    akt.fields = Json::Object(fields_to_json(&outcome.fields));
    akt.revision = expected_revision + 1;
    akt.title = title_of(&stored.ty, &outcome.fields);
    akt.updated_at = now;
    if !store::update_akt(conn, &akt, expected_revision).await? {
        return Err(AktError::Internal(format!(
            "akt {number} moved while locked"
        )));
    }
    check_sections(conn, tenant, &stored.ty, Some(akt.id), &outcome).await?;
    check_contacts(conn, tenant, &outcome).await?;
    hold_attachments(conn, caller, &stored.ty, &akt, &outcome).await?;
    diagrams::render_written(conn, caller, &stored.ty, &akt, &outcome).await?;
    let mut recorded = revision(&akt, actor, transition, reason, &outcome, now);
    references::record(conn, &stored.ty, &akt, &outcome.fields, &mut recorded).await?;
    store::append_revision(conn, &recorded).await?;
    Ok(akt)
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
    let sequence = store::next_counter(conn, tenant, &numbering::counter(ty, now, parent)).await?;
    let sequence = u64::try_from(sequence).map_err(|e| AktError::Internal(e.to_string()))?;
    Ok(numbering::render(ty, now, parent, sequence))
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

/// The title the fields give an akt of `ty`, if its type has a title field
/// and it holds text.
// r[impl akt.title]
pub(crate) fn title_of(
    ty: &AktType,
    fields: &std::collections::BTreeMap<String, kuria_engine::Value>,
) -> Option<String> {
    let field = ty.title_field()?;
    match fields.get(&field.key)? {
        kuria_engine::Value::Text(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        _ => None,
    }
}

/// Refuses a change whose new values name a contact the tenant doesn't
/// have.
// r[impl contact.reference]
async fn check_contacts(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    outcome: &Outcome,
) -> Result<(), AktError> {
    for (key, value) in &outcome.changes {
        let Some(value) = value else { continue };
        let named: Vec<Uuid> = value
            .named()
            .iter()
            .filter_map(kuria_engine::Someone::contact)
            .collect();
        if named.is_empty() {
            continue;
        }
        let found = kuria_store::contacts::find(conn, tenant, &named).await?;
        if let Some(missing) = named.iter().find(|id| !found.iter().any(|c| c.id == **id)) {
            return Err(AktError::Refused(Refusal::InvalidValue {
                field: key.clone(),
                reason: format!("there is no contact `{missing}`"),
            }));
        }
    }
    Ok(())
}

/// Checks the attachments `outcome` puts in `akt`'s fields, and records
/// which attachments the akt holds. Only attachments of the tenant the
/// caller may read are taken, and only SBOMs in `sbom` fields.
// r[impl akt.field.attachment]
// r[impl attachment.references]
async fn hold_attachments(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    ty: &AktType,
    akt: &AktRow,
    outcome: &Outcome,
) -> Result<(), AktError> {
    let is_attachment = |key: &str| {
        ty.field(key)
            .is_some_and(|f| f.kind == FieldKind::Attachment)
    };
    if !outcome.changes.keys().any(|key| is_attachment(key)) {
        return Ok(());
    }
    for (key, value) in &outcome.changes {
        let Some(kuria_engine::Value::Attachment(id)) = value else {
            continue;
        };
        let refused = |reason: &str| {
            AktError::Refused(Refusal::InvalidValue {
                field: key.clone(),
                reason: reason.to_owned(),
            })
        };
        let row = attachments::find(conn, akt.tenant_id, &[*id])
            .await?
            .pop()
            .ok_or_else(|| refused("there is no such attachment"))?;
        if !crate::attachments::may_read(conn, caller.auth, caller.actor.subject.into(), &row)
            .await?
        {
            return Err(refused("there is no such attachment"));
        }
        let wants_sbom = ty
            .field(key)
            .is_some_and(|f| f.format == Some(AttachmentFormat::Sbom));
        if wants_sbom && row.sbom.is_none() {
            return Err(refused("the attachment is not an SBOM"));
        }
        // r[impl attachment.media-types]
        if let Some(field) = ty.field(key)
            && !field.media_types.is_empty()
            && !field.media_types.contains(&row.media_type)
        {
            return Err(refused(&format!(
                "the attachment is {}, and the field takes {}",
                row.media_type,
                field.media_types.join(", ")
            )));
        }
    }
    let held: Vec<(String, Uuid)> = outcome
        .fields
        .iter()
        .filter_map(|(key, value)| match value {
            kuria_engine::Value::Attachment(id) => Some((key.clone(), *id)),
            _ => None,
        })
        .collect();
    attachments::set_held(conn, akt.tenant_id, akt.id, &held).await?;
    Ok(())
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

/// An akt type document, validated, and as it is stored: exactly what was
/// checked.
pub(crate) fn read_document(document: Json) -> Result<(AktType, Json), AktError> {
    let parsed: AktTypeDocument =
        serde_json::from_value(document).map_err(|e| AktError::UnreadableType(e.to_string()))?;
    let ty = validate(parsed).map_err(AktError::InvalidType)?;
    let definition =
        serde_json::to_value(ty.document()).map_err(|e| AktError::Internal(e.to_string()))?;
    Ok((ty, definition))
}

/// Stores `ty` as the next version of its key in `tenant`, in the caller's
/// transaction, once what it names of the tenant's other akt types checks.
pub(crate) async fn define_in(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    by: Uuid,
    ty: AktType,
    definition: &Json,
) -> Result<StoredType, AktError> {
    // r[impl akt.type.state-references]
    // One definition at a time per tenant, so two can't each pass against
    // the other's old version.
    store::next_counter(conn, tenant, "types").await?;
    let others = store::latest_type_versions(conn, tenant)
        .await?
        .into_iter()
        .filter(|row| row.key != ty.key())
        .map(|row| StoredType::read(row).map(|stored| stored.ty))
        .collect::<Result<Vec<_>, _>>()?;
    let mut problems = check_state_references(&ty, &others);
    problems.extend(check_field_references(&ty, &others));
    if !problems.is_empty() {
        return Err(AktError::InvalidType(problems));
    }
    let row =
        store::insert_type_version(conn, tenant, ty.key(), definition, by, Utc::now()).await?;
    Ok(StoredType { row, ty })
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

/// Refuses an outcome whose `sections` name an akt that isn't one of the
/// akt `akt_id`'s members, or one more than once, or of a type its `of`
/// leaves out; a new akt has no members yet.
// r[impl akt.field.sections]
async fn check_sections(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    ty: &AktType,
    akt_id: Option<Uuid>,
    outcome: &Outcome,
) -> Result<(), AktError> {
    for field in ty
        .document()
        .fields
        .iter()
        .filter(|f| f.kind == FieldKind::Sections)
    {
        let Some(Some(kuria_engine::Value::Sections(items))) = outcome.changes.get(&field.key)
        else {
            continue;
        };
        let members = match akt_id {
            Some(id) => containers::members_with_types(conn, tenant, id).await?,
            None => Vec::new(),
        };
        let named: Vec<(&str, &str)> = members
            .iter()
            .map(|(row, key)| (row.number.as_str(), key.as_str()))
            .collect();
        kuria_engine::sections::check(items, &named, &field.of).map_err(|reason| {
            AktError::Refused(Refusal::InvalidValue {
                field: field.key.clone(),
                reason,
            })
        })?;
    }
    Ok(())
}

/// Takes `member` out of `akt`'s sections, as when it leaves the container,
/// and answers the field changes that made, for its revision.
// r[impl akt.field.sections]
fn leave_sections(
    ty: &StoredType,
    akt: &mut AktRow,
    member: &str,
) -> Result<Map<String, Json>, AktError> {
    let mut changes = Map::new();
    let Some(field) = ty
        .ty
        .document()
        .fields
        .iter()
        .find(|f| f.kind == FieldKind::Sections)
    else {
        return Ok(changes);
    };
    let Some(fields) = akt.fields.as_object_mut() else {
        return Ok(changes);
    };
    let Some(raw) = fields.get(&field.key) else {
        return Ok(changes);
    };
    let Ok(kuria_engine::Value::Sections(items)) = kuria_engine::Value::from_json(field, raw)
    else {
        return Err(AktError::Internal(format!(
            "akt {}: unreadable sections",
            akt.number
        )));
    };
    if !kuria_engine::sections::numbers(&items).contains(&member) {
        return Ok(changes);
    }
    let kept =
        kuria_engine::Value::Sections(kuria_engine::sections::without(&items, member)).to_json();
    fields.insert(field.key.clone(), kept.clone());
    changes.insert(field.key.clone(), kept);
    Ok(changes)
}

/// One side of a membership, as a membership revision records it.
fn membership_entry(akt: &AktRow) -> Json {
    records::membership_entry(&akt.id.to_string(), &akt.number)
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
    record_membership_changing(conn, ty, akt, actor, reason, membership, Map::new(), now).await
}

/// [`record_membership`], also recording `changes` the membership made to
/// the akt's fields, already set in `akt.fields`: a member leaving its
/// sections (`akt.field.sections`).
#[allow(
    clippy::too_many_arguments,
    reason = "record_membership's, and the changes"
)]
async fn record_membership_changing(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &mut AktRow,
    actor: Actor,
    reason: Option<String>,
    membership: Json,
    changes: Map<String, Json>,
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
            changes: Json::Object(changes),
            state: akt.state.clone(),
            hash: hash.to_vec(),
            created_at: now,
            membership: Some(membership),
        },
    )
    .await?;
    Ok(())
}
