//! Versions (`version.*`): revisions of an akt named by releasing them.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde_json::Value as Json;
use uuid::Uuid;

use crate::Connection;
use crate::akter::next_counter;
use crate::schema::{akt_version, change_event};

/// One version, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_version)]
pub struct VersionRow {
    pub tenant_id: Uuid,
    pub akt_id: Uuid,
    pub number: i32,
    pub revision: i32,
    pub label: Option<String>,
    pub released_by: Uuid,
    pub released_at: DateTime<Utc>,
    pub pins: Json,
}

/// Stores `row`, and its change event with the tenant's next change
/// sequence number, which it returns (`version.history`).
// r[impl version.history]
pub async fn insert(conn: &mut Connection, row: &VersionRow) -> QueryResult<i64> {
    diesel::insert_into(akt_version::table)
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
            change_event::created_at.eq(row.released_at),
            change_event::kind.eq("version"),
        ))
        .execute(conn)
        .await?;
    Ok(seq)
}

/// The versions of `akt`, latest first.
// r[impl version.list]
pub async fn of(conn: &mut Connection, tenant: Uuid, akt: Uuid) -> QueryResult<Vec<VersionRow>> {
    akt_version::table
        .filter(akt_version::tenant_id.eq(tenant))
        .filter(akt_version::akt_id.eq(akt))
        .order(akt_version::number.desc())
        .select(VersionRow::as_select())
        .load(conn)
        .await
}

/// The version `number` of `akt`, if it has one.
pub async fn one(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    number: i32,
) -> QueryResult<Option<VersionRow>> {
    akt_version::table
        .filter(akt_version::tenant_id.eq(tenant))
        .filter(akt_version::akt_id.eq(akt))
        .filter(akt_version::number.eq(number))
        .select(VersionRow::as_select())
        .first(conn)
        .await
        .optional()
}
