//! Creating akter, on their own or inside containers.

use chrono::Utc;
use diesel_async::AsyncConnection;
use kuria_engine::{Change, fields_to_json};
use kuria_store::akter::{self as store, AktRow};
use kuria_store::containers::{self, MembershipRow};
use serde_json::{Map, Value as Json, json};
use uuid::Uuid;

use kuria_engine::rules::containment;

use super::{
    AktError, Akter, Caller, StoredType, akt_ref, check_contacts, hold_attachments,
    membership_entry, next_number, rank_after_last, record_membership, require, require_read,
    revision, title_of, type_of,
};
use crate::authorize::Permission;

impl Akter {
    /// Creates an akt of the akt type `key`, as a member of the containers
    /// named in `inside`, the first of which is its parent.
    // r[impl akt.create]
    // r[impl akt.revision.atomic]
    // r[impl authz.create]
    // r[impl container.create-inside]
    // r[impl authz.create-inside]
    pub async fn create(
        &self,
        caller: Caller<'_>,
        key: &str,
        fields: &Map<String, Json>,
        reason: Option<String>,
        inside: &[String],
        number: Option<&str>,
    ) -> Result<AktRow, AktError> {
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                create_in(conn, caller, key, fields, reason.clone(), inside, number).await
            })
            .await;
        self.committed(result)
    }
}

/// The body of [`Akter::create`], in the transaction `conn` is in.
pub(super) async fn create_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    key: &str,
    fields: &Map<String, Json>,
    reason: Option<String>,
    inside: &[String],
    given: Option<&str>,
) -> Result<AktRow, AktError> {
    let Caller { actor, auth, .. } = caller;
    let tenant: Uuid = actor.tenant.into();
    let row = store::latest_type_version(conn, tenant, key)
        .await?
        .ok_or_else(|| AktError::UnknownType(key.to_owned()))?;
    if !auth.sees_type(key).await? {
        return Err(AktError::UnknownType(key.to_owned()));
    }
    require(
        auth,
        Permission::Create {
            key: key.to_owned(),
        },
    )
    .await?;
    let stored = StoredType::read(row)?;

    // The order lock first, then the containers' rows, as every
    // write path takes them.
    containers::lock_order(conn, tenant).await?;
    let mut parents = lock_containers(conn, tenant, inside).await?;
    let mut parent_types = Vec::with_capacity(parents.len());
    for parent in &parents {
        let ty = type_of(conn, tenant, parent).await?;
        require_read(auth, &ty, &parent.number).await?;
        require(
            auth,
            Permission::Edit {
                key: ty.ty.key().to_owned(),
            },
        )
        .await?;
        parent_types.push(ty);
    }
    check_parents(&stored, &parents, &parent_types)?;

    let now = Utc::now();
    let outcome = kuria_engine::create(
        &stored.ty,
        fields,
        Change {
            by: actor.subject.into(),
            at: now,
        },
    )?;
    super::check_sections(conn, tenant, &stored.ty, None, &outcome).await?;
    let parent = parents.first().map(|p| p.number.as_str());
    let number = number_for(conn, tenant, &stored.ty, given, parent, now).await?;
    // r[impl archive.rules.state-since]: created in its initial state now.
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
        rank: rank_after_last(conn, tenant).await?,
        title: title_of(&stored.ty, &outcome.fields),
        state_since: now,
        ..AktRow::default()
    };
    store::insert_akt(conn, &akt).await?;
    check_contacts(conn, tenant, &outcome).await?;
    hold_attachments(conn, caller, &stored.ty, &akt, &outcome).await?;
    super::diagrams::render_written(conn, caller, &stored.ty, &akt, &outcome).await?;
    let mut first = revision(&akt, actor, None, reason.clone(), &outcome, now);
    if !parents.is_empty() {
        first.membership = Some(json!({
            "added": parents.iter().map(membership_entry).collect::<Vec<_>>(),
        }));
    }
    super::references::record(conn, &stored.ty, &akt, &outcome.fields, &mut first).await?;
    store::append_revision(conn, &first).await?;
    // r[impl live.edit]
    // Its first revision, and its live draft as that revision has it.
    if super::live::has_live_draft(&stored) {
        super::live::start(conn, &stored, &akt, &outcome.fields, now).await?;
    }

    for (parent, ty) in parents.iter_mut().zip(&parent_types) {
        containers::insert_membership(
            conn,
            &MembershipRow {
                tenant_id: tenant,
                container_id: parent.id,
                member_id: akt.id,
                added_at: now,
                added_by: actor.subject.into(),
            },
        )
        .await?;
        record_membership(
            conn,
            ty,
            parent,
            actor,
            reason.clone(),
            json!({ "added_member": [membership_entry(&akt)] }),
            now,
        )
        .await?;
    }
    Ok(akt)
}

/// The containers `numbers` names, locked and in the order named.
async fn lock_containers(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    numbers: &[String],
) -> Result<Vec<AktRow>, AktError> {
    let refs: Vec<&str> = numbers.iter().map(String::as_str).collect();
    containment::check_distinct(&refs)?;
    let mut locked = containers::lock_akter(conn, tenant, &refs).await?;
    numbers
        .iter()
        .map(|number| {
            locked
                .iter()
                .position(|row| &row.number == number)
                .map(|i| locked.swap_remove(i))
                .ok_or_else(|| AktError::NotFound(number.clone()))
                .and_then(|row| super::archive::writable(&row).map(|()| row))
        })
        .collect()
}

/// Refuses containers an akt of `ty` can't be created in.
fn check_parents(
    ty: &StoredType,
    parents: &[AktRow],
    types: &[StoredType],
) -> Result<(), AktError> {
    let kinds: Vec<&kuria_config::AktType> = types.iter().map(|t| &t.ty).collect();
    containment::check_created_in(&ty.ty, &kinds)?;
    let refs: Vec<_> = parents
        .iter()
        .zip(types)
        .map(|(p, t)| akt_ref(t, p))
        .collect();
    Ok(containment::check_create_inside(&ty.ty, &refs)?)
}

/// The number of a new akt of `ty` created in `parent` at `now`: the one
/// `given`, else its next.
async fn number_for(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    ty: &kuria_config::AktType,
    given: Option<&str>,
    parent: Option<&str>,
    now: chrono::DateTime<Utc>,
) -> Result<String, AktError> {
    match given {
        Some(given) => given_number(conn, tenant, ty, given, parent).await,
        None => next_number(conn, tenant, ty, parent, now).await,
    }
}

/// `number`, given to an akt of `ty` created in `parent`, checked as one
/// the pattern gives and free, its counter raised past it. The caller holds
/// the tenant's order lock.
// r[impl akt.number.given]
async fn given_number(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    ty: &kuria_config::AktType,
    number: &str,
    parent: Option<&str>,
) -> Result<String, AktError> {
    let counter =
        kuria_engine::rules::numbering::given(ty, number, parent).map_err(AktError::BadRequest)?;
    let like = number
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    if !store::numbers_like(conn, tenant, &like).await?.is_empty() {
        return Err(AktError::NumberTaken(number.to_owned()));
    }
    if let Some((name, sequence)) = counter {
        let value = i64::try_from(sequence).unwrap_or(i64::MAX);
        kuria_store::bundles::raise_counter(conn, tenant, &name, value).await?;
    }
    Ok(number.to_owned())
}
