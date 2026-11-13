//! Typed links between akter (`link.*`).

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::akter::AktRow;
use crate::schema::{akt, akt_link};

/// One link, as stored.
#[derive(Clone, Debug, PartialEq, Eq, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_link)]
pub struct LinkRow {
    pub tenant_id: Uuid,
    pub source_id: Uuid,
    pub kind: String,
    pub target_id: Uuid,
    pub pinned_revision: Option<i32>,
    pub added_at: DateTime<Utc>,
    pub added_by: Uuid,
}

/// Stores `link`, and answers whether it is new: a link of the same kind
/// between the same akter is kept as it was.
// r[impl link.add]
pub async fn insert(conn: &mut Connection, link: &LinkRow) -> QueryResult<bool> {
    let inserted = diesel::insert_into(akt_link::table)
        .values(link)
        .on_conflict_do_nothing()
        .execute(conn)
        .await?;
    Ok(inserted == 1)
}

/// Deletes the link of `kind` from `source` to `target`, and answers whether
/// there was one.
// r[impl link.remove]
pub async fn delete(
    conn: &mut Connection,
    tenant: Uuid,
    source: Uuid,
    kind: &str,
    target: Uuid,
) -> QueryResult<bool> {
    let deleted = diesel::delete(
        akt_link::table
            .filter(akt_link::tenant_id.eq(tenant))
            .filter(akt_link::source_id.eq(source))
            .filter(akt_link::kind.eq(kind))
            .filter(akt_link::target_id.eq(target)),
    )
    .execute(conn)
    .await?;
    Ok(deleted == 1)
}

/// The links from `akt`, each with its target, by kind and then the
/// target's rank.
// r[impl link.read]
pub async fn from(
    conn: &mut Connection,
    tenant: Uuid,
    akt_id: Uuid,
) -> QueryResult<Vec<(LinkRow, AktRow)>> {
    akt_link::table
        .inner_join(akt::table.on(akt::id.eq(akt_link::target_id)))
        .filter(akt_link::tenant_id.eq(tenant))
        .filter(akt_link::source_id.eq(akt_id))
        .order((akt_link::kind, akt::rank))
        .select((LinkRow::as_select(), AktRow::as_select()))
        .load(conn)
        .await
}

/// The links to `akt`, each with its source, by kind and then the source's
/// rank.
// r[impl link.read]
pub async fn to(
    conn: &mut Connection,
    tenant: Uuid,
    akt_id: Uuid,
) -> QueryResult<Vec<(LinkRow, AktRow)>> {
    akt_link::table
        .inner_join(akt::table.on(akt::id.eq(akt_link::source_id)))
        .filter(akt_link::tenant_id.eq(tenant))
        .filter(akt_link::target_id.eq(akt_id))
        .order((akt_link::kind, akt::rank))
        .select((LinkRow::as_select(), AktRow::as_select()))
        .load(conn)
        .await
}
