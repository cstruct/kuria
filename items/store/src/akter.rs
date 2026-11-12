//! Akt types, akter, their revisions, and the change-event outbox.
//!
//! Every function takes the tenant, and every query filters on it. Writes
//! that belong together are meant to run in one transaction, which the
//! caller holds.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde_json::Value as Json;
use uuid::Uuid;

use crate::Connection;
use crate::schema::{akt, akt_revision, akt_type, akt_type_version, change_event, tenant_counter};

/// Takes the next value of a tenant's counter `name`, counting from 1. The
/// counter's row stays locked until the transaction ends.
pub async fn next_counter(conn: &mut Connection, tenant: Uuid, name: &str) -> QueryResult<i64> {
    diesel::insert_into(tenant_counter::table)
        .values((
            tenant_counter::tenant_id.eq(tenant),
            tenant_counter::name.eq(name),
            tenant_counter::value.eq(1_i64),
        ))
        .on_conflict((tenant_counter::tenant_id, tenant_counter::name))
        .do_update()
        .set(tenant_counter::value.eq(tenant_counter::value + 1))
        .returning(tenant_counter::value)
        .get_result(conn)
        .await
}

/// An akt type version, as stored.
#[derive(Clone, Debug, Queryable, Selectable)]
#[diesel(table_name = akt_type_version)]
pub struct AktTypeVersion {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub key: String,
    pub version: i32,
    pub definition: Json,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Stores `definition` as the next version of the akt type `key`, and
/// returns it.
pub async fn insert_type_version(
    conn: &mut Connection,
    tenant: Uuid,
    key: &str,
    definition: &Json,
    created_by: Uuid,
    now: DateTime<Utc>,
) -> QueryResult<AktTypeVersion> {
    let version: i32 = diesel::insert_into(akt_type::table)
        .values((
            akt_type::tenant_id.eq(tenant),
            akt_type::key.eq(key),
            akt_type::latest_version.eq(1),
        ))
        .on_conflict((akt_type::tenant_id, akt_type::key))
        .do_update()
        .set(akt_type::latest_version.eq(akt_type::latest_version + 1))
        .returning(akt_type::latest_version)
        .get_result(conn)
        .await?;
    diesel::insert_into(akt_type_version::table)
        .values((
            akt_type_version::id.eq(Uuid::now_v7()),
            akt_type_version::tenant_id.eq(tenant),
            akt_type_version::key.eq(key),
            akt_type_version::version.eq(version),
            akt_type_version::definition.eq(definition),
            akt_type_version::created_by.eq(created_by),
            akt_type_version::created_at.eq(now),
        ))
        .returning(AktTypeVersion::as_returning())
        .get_result(conn)
        .await
}

/// The latest version of the akt type `key`.
pub async fn latest_type_version(
    conn: &mut Connection,
    tenant: Uuid,
    key: &str,
) -> QueryResult<Option<AktTypeVersion>> {
    akt_type_version::table
        .inner_join(
            akt_type::table.on(akt_type::tenant_id
                .eq(akt_type_version::tenant_id)
                .and(akt_type::key.eq(akt_type_version::key))
                .and(akt_type::latest_version.eq(akt_type_version::version))),
        )
        .filter(akt_type_version::tenant_id.eq(tenant))
        .filter(akt_type_version::key.eq(key))
        .select(AktTypeVersion::as_select())
        .first(conn)
        .await
        .optional()
}

/// A version of the akt type `key`.
pub async fn type_version(
    conn: &mut Connection,
    tenant: Uuid,
    key: &str,
    version: i32,
) -> QueryResult<Option<AktTypeVersion>> {
    akt_type_version::table
        .filter(akt_type_version::tenant_id.eq(tenant))
        .filter(akt_type_version::key.eq(key))
        .filter(akt_type_version::version.eq(version))
        .select(AktTypeVersion::as_select())
        .first(conn)
        .await
        .optional()
}

/// Akt type versions by id, in the tenant.
pub async fn type_versions_by_id(
    conn: &mut Connection,
    tenant: Uuid,
    ids: &[Uuid],
) -> QueryResult<Vec<AktTypeVersion>> {
    akt_type_version::table
        .filter(akt_type_version::tenant_id.eq(tenant))
        .filter(akt_type_version::id.eq_any(ids))
        .select(AktTypeVersion::as_select())
        .load(conn)
        .await
}

/// The latest version of every akt type in the tenant, by key.
pub async fn latest_type_versions(
    conn: &mut Connection,
    tenant: Uuid,
) -> QueryResult<Vec<AktTypeVersion>> {
    akt_type_version::table
        .inner_join(
            akt_type::table.on(akt_type::tenant_id
                .eq(akt_type_version::tenant_id)
                .and(akt_type::key.eq(akt_type_version::key))
                .and(akt_type::latest_version.eq(akt_type_version::version))),
        )
        .filter(akt_type_version::tenant_id.eq(tenant))
        .order(akt_type_version::key)
        .select(AktTypeVersion::as_select())
        .load(conn)
        .await
}

/// An akt's current state, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt)]
pub struct AktRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub number: String,
    pub type_version_id: Uuid,
    pub state: String,
    pub fields: Json,
    pub revision: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An akt revision, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_revision)]
pub struct RevisionRow {
    pub akt_id: Uuid,
    pub revision: i32,
    pub tenant_id: Uuid,
    pub author: Uuid,
    pub transition: Option<String>,
    pub reason: Option<String>,
    pub changes: Json,
    pub state: String,
    pub hash: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

/// Stores a new akt.
pub async fn insert_akt(conn: &mut Connection, row: &AktRow) -> QueryResult<()> {
    diesel::insert_into(akt::table)
        .values(row)
        .execute(conn)
        .await
        .map(drop)
}

/// Replaces an akt's state, fields and revision, if it is still at
/// `expected_revision`. Returns whether it was.
pub async fn update_akt(
    conn: &mut Connection,
    row: &AktRow,
    expected_revision: i32,
) -> QueryResult<bool> {
    let updated = diesel::update(
        akt::table
            .filter(akt::tenant_id.eq(row.tenant_id))
            .filter(akt::id.eq(row.id))
            .filter(akt::revision.eq(expected_revision)),
    )
    .set((
        akt::state.eq(&row.state),
        akt::fields.eq(&row.fields),
        akt::revision.eq(row.revision),
        akt::updated_at.eq(row.updated_at),
    ))
    .execute(conn)
    .await?;
    Ok(updated == 1)
}

/// Appends a revision, and its change event with the tenant's next change
/// sequence number, which it returns.
pub async fn append_revision(conn: &mut Connection, row: &RevisionRow) -> QueryResult<i64> {
    diesel::insert_into(akt_revision::table)
        .values(row)
        .execute(conn)
        .await?;
    let seq = next_counter(conn, row.tenant_id, "change_event").await?;
    diesel::insert_into(change_event::table)
        .values((
            change_event::tenant_id.eq(row.tenant_id),
            change_event::seq.eq(seq),
            change_event::akt_id.eq(row.akt_id),
            change_event::revision.eq(row.revision),
            change_event::created_at.eq(row.created_at),
        ))
        .execute(conn)
        .await?;
    Ok(seq)
}

/// The akt numbered `number`.
pub async fn akt_by_number(
    conn: &mut Connection,
    tenant: Uuid,
    number: &str,
) -> QueryResult<Option<AktRow>> {
    akt::table
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt::number.eq(number))
        .select(AktRow::as_select())
        .first(conn)
        .await
        .optional()
}

/// The akt numbered `number`, locked until the transaction ends.
pub async fn lock_akt_by_number(
    conn: &mut Connection,
    tenant: Uuid,
    number: &str,
) -> QueryResult<Option<AktRow>> {
    akt::table
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt::number.eq(number))
        .select(AktRow::as_select())
        .for_update()
        .first(conn)
        .await
        .optional()
}

/// Up to `limit` akter of the akt types `keys`, newest first, optionally only
/// in `state`.
pub async fn list_akter(
    conn: &mut Connection,
    tenant: Uuid,
    keys: &[String],
    state: Option<&str>,
    limit: i64,
) -> QueryResult<Vec<AktRow>> {
    let mut query = akt::table
        .inner_join(akt_type_version::table)
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt_type_version::key.eq_any(keys))
        .select(AktRow::as_select())
        .order(akt::created_at.desc())
        .limit(limit)
        .into_boxed();
    if let Some(state) = state {
        query = query.filter(akt::state.eq(state.to_owned()));
    }
    query.load(conn).await
}

/// An akt's revisions, oldest first.
pub async fn revisions(
    conn: &mut Connection,
    tenant: Uuid,
    akt_id: Uuid,
) -> QueryResult<Vec<RevisionRow>> {
    akt_revision::table
        .filter(akt_revision::tenant_id.eq(tenant))
        .filter(akt_revision::akt_id.eq(akt_id))
        .order(akt_revision::revision)
        .select(RevisionRow::as_select())
        .load(conn)
        .await
}

/// A change event, as stored.
#[derive(Clone, Debug, Queryable, Selectable)]
#[diesel(table_name = change_event)]
pub struct ChangeEventRow {
    pub tenant_id: Uuid,
    pub seq: i64,
    pub akt_id: Uuid,
    pub revision: i32,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

/// The tenant's change events after `after`, in order, up to `limit`.
pub async fn change_events_after(
    conn: &mut Connection,
    tenant: Uuid,
    after: i64,
    limit: i64,
) -> QueryResult<Vec<ChangeEventRow>> {
    change_event::table
        .filter(change_event::tenant_id.eq(tenant))
        .filter(change_event::seq.gt(after))
        .order(change_event::seq)
        .limit(limit)
        .select(ChangeEventRow::as_select())
        .load(conn)
        .await
}
