//! Memberships, ranks and their change events.
//!
//! Every function takes the tenant and filters on it. Callers hold the
//! transaction, and the locks that make a change consistent: both akter's
//! rows for a membership, and the tenant's order lock for a rank.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Bool, Uuid as SqlUuid};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::akter::{AktRow, next_counter};
use crate::schema::{akt, akt_membership, akt_type_version, change_event};

/// Takes the tenant's order lock until the transaction ends. Every change
/// to a rank takes it first, so two of them never compute the same key.
pub async fn lock_order(conn: &mut Connection, tenant: Uuid) -> QueryResult<()> {
    next_counter(conn, tenant, "order").await.map(drop)
}

/// The highest rank in the tenant, if it has any akter.
pub async fn last_rank(conn: &mut Connection, tenant: Uuid) -> QueryResult<Option<String>> {
    akt::table
        .filter(akt::tenant_id.eq(tenant))
        .order(akt::rank.desc())
        .select(akt::rank)
        .first(conn)
        .await
        .optional()
}

/// The rank directly before `rank` in the tenant's order, if any.
pub async fn rank_before(
    conn: &mut Connection,
    tenant: Uuid,
    rank: &str,
) -> QueryResult<Option<String>> {
    akt::table
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt::rank.lt(rank))
        .order(akt::rank.desc())
        .select(akt::rank)
        .first(conn)
        .await
        .optional()
}

/// The rank directly after `rank` in the tenant's order, if any.
pub async fn rank_after(
    conn: &mut Connection,
    tenant: Uuid,
    rank: &str,
) -> QueryResult<Option<String>> {
    akt::table
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt::rank.gt(rank))
        .order(akt::rank.asc())
        .select(akt::rank)
        .first(conn)
        .await
        .optional()
}

/// Gives the akt `id` the rank `rank`.
pub async fn set_rank(
    conn: &mut Connection,
    tenant: Uuid,
    id: Uuid,
    rank: &str,
) -> QueryResult<()> {
    diesel::update(
        akt::table
            .filter(akt::tenant_id.eq(tenant))
            .filter(akt::id.eq(id)),
    )
    .set(akt::rank.eq(rank))
    .execute(conn)
    .await
    .map(drop)
}

/// Appends a `rank` change event for the akt `id`, and returns its sequence
/// number.
pub async fn append_rank_event(
    conn: &mut Connection,
    tenant: Uuid,
    id: Uuid,
    now: DateTime<Utc>,
) -> QueryResult<i64> {
    let seq = next_counter(conn, tenant, "change_event").await?;
    diesel::insert_into(change_event::table)
        .values((
            change_event::tenant_id.eq(tenant),
            change_event::seq.eq(seq),
            change_event::akt_id.eq(id),
            change_event::created_at.eq(now),
            change_event::kind.eq("rank"),
        ))
        .execute(conn)
        .await?;
    Ok(seq)
}

/// The akter `numbers`, locked until the transaction ends, in a fixed order,
/// so that two transactions locking the same akter can't deadlock.
pub async fn lock_akter(
    conn: &mut Connection,
    tenant: Uuid,
    numbers: &[&str],
) -> QueryResult<Vec<AktRow>> {
    akt::table
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt::number.eq_any(numbers))
        .order(akt::id)
        .select(AktRow::as_select())
        .for_update()
        .load(conn)
        .await
}

/// One membership, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_membership)]
pub struct MembershipRow {
    pub tenant_id: Uuid,
    pub container_id: Uuid,
    pub member_id: Uuid,
    pub added_at: DateTime<Utc>,
    pub added_by: Uuid,
}

/// Stores a membership. The primary key refuses a second one of the same
/// pair, and a check refuses an akt in itself.
// r[impl container.many]
pub async fn insert_membership(conn: &mut Connection, row: &MembershipRow) -> QueryResult<()> {
    diesel::insert_into(akt_membership::table)
        .values(row)
        .execute(conn)
        .await
        .map(drop)
}

/// Ends a membership, and returns whether there was one.
pub async fn delete_membership(
    conn: &mut Connection,
    tenant: Uuid,
    container: Uuid,
    member: Uuid,
) -> QueryResult<bool> {
    let deleted = diesel::delete(
        akt_membership::table
            .filter(akt_membership::tenant_id.eq(tenant))
            .filter(akt_membership::container_id.eq(container))
            .filter(akt_membership::member_id.eq(member)),
    )
    .execute(conn)
    .await?;
    Ok(deleted == 1)
}

/// Whether `member` is a member of `container`.
pub async fn is_member(
    conn: &mut Connection,
    tenant: Uuid,
    container: Uuid,
    member: Uuid,
) -> QueryResult<bool> {
    diesel::select(diesel::dsl::exists(
        akt_membership::table
            .filter(akt_membership::tenant_id.eq(tenant))
            .filter(akt_membership::container_id.eq(container))
            .filter(akt_membership::member_id.eq(member)),
    ))
    .get_result(conn)
    .await
}

/// The containers `member` is in, each with its akt type's key, in rank
/// order.
pub async fn containers_of(
    conn: &mut Connection,
    tenant: Uuid,
    member: Uuid,
) -> QueryResult<Vec<(AktRow, String)>> {
    akt_membership::table
        .inner_join(akt::table.on(akt::id.eq(akt_membership::container_id)))
        .inner_join(akt_type_version::table.on(akt_type_version::id.eq(akt::type_version_id)))
        .filter(akt_membership::tenant_id.eq(tenant))
        .filter(akt_membership::member_id.eq(member))
        .order(akt::rank)
        .select((AktRow::as_select(), akt_type_version::key))
        .load(conn)
        .await
}

/// Every member of `container`, each with its akt type's key, in rank
/// order.
pub async fn members_with_types(
    conn: &mut Connection,
    tenant: Uuid,
    container: Uuid,
) -> QueryResult<Vec<(AktRow, String)>> {
    akt_membership::table
        .inner_join(akt::table.on(akt::id.eq(akt_membership::member_id)))
        .inner_join(akt_type_version::table.on(akt_type_version::id.eq(akt::type_version_id)))
        .filter(akt_membership::tenant_id.eq(tenant))
        .filter(akt_membership::container_id.eq(container))
        .order(akt::rank)
        .select((AktRow::as_select(), akt_type_version::key))
        .load(conn)
        .await
}

/// The members of `containers` whose akt types are among `keys`, in rank
/// order, each with the container it was found in, up to `limit` per
/// container.
// r[impl order.everywhere]
pub async fn members_of(
    conn: &mut Connection,
    tenant: Uuid,
    containers: &[Uuid],
    keys: &[String],
    limit: i64,
) -> QueryResult<Vec<(Uuid, AktRow)>> {
    let rows: Vec<(Uuid, AktRow)> = akt_membership::table
        .inner_join(akt::table.on(akt::id.eq(akt_membership::member_id)))
        .inner_join(akt_type_version::table.on(akt_type_version::id.eq(akt::type_version_id)))
        .filter(akt_membership::tenant_id.eq(tenant))
        .filter(akt_membership::container_id.eq_any(containers))
        .filter(akt_type_version::key.eq_any(keys))
        .order((akt_membership::container_id, akt::rank))
        .select((akt_membership::container_id, AktRow::as_select()))
        .load(conn)
        .await?;
    // Keep the first `limit` of each container. Boards are small, and one
    // query for all containers beats one per container.
    let mut taken: std::collections::HashMap<Uuid, i64> = std::collections::HashMap::new();
    Ok(rows
        .into_iter()
        .filter(|(container, _)| {
            let count = taken.entry(*container).or_default();
            *count += 1;
            *count <= limit
        })
        .collect())
}

/// Up to `limit` members of `container` of the akt types `keys`, meeting
/// every condition, in rank order, after the member ranked `after`. Ranks are unique in a tenant, so
/// a rank says where a page ends.
// r[impl akt.page]
pub async fn member_page(
    conn: &mut Connection,
    tenant: Uuid,
    container: Uuid,
    keys: &[String],
    conditions: &[crate::akter::Condition],
    after: Option<&str>,
    limit: i64,
) -> QueryResult<Vec<AktRow>> {
    let mut query = akt_membership::table
        .inner_join(akt::table.on(akt::id.eq(akt_membership::member_id)))
        .inner_join(akt_type_version::table.on(akt_type_version::id.eq(akt::type_version_id)))
        .filter(akt_membership::tenant_id.eq(tenant))
        .filter(akt_membership::container_id.eq(container))
        .filter(akt_type_version::key.eq_any(keys))
        .order(akt::rank)
        .limit(limit)
        .select(AktRow::as_select())
        .into_boxed();
    if let Some(ids) = crate::akter::meeting(tenant, conditions) {
        query = query.filter(akt::id.eq_any(ids));
    }
    if let Some(after) = after {
        query = query.filter(akt::rank.gt(after.to_owned()));
    }
    query.load(conn).await
}

#[derive(QueryableByName)]
struct Reachable {
    #[diesel(sql_type = Bool)]
    reachable: bool,
}

/// Whether `descendant` is `ancestor` or contained in it, directly or
/// through other containers. Adding `container` into `member`'s containers
/// would close a cycle exactly when `contains(member, container)`.
// r[related container.acyclic]
pub async fn contains(
    conn: &mut Connection,
    tenant: Uuid,
    ancestor: Uuid,
    descendant: Uuid,
) -> QueryResult<bool> {
    if ancestor == descendant {
        return Ok(true);
    }
    let found: Reachable = diesel::sql_query(
        "with recursive below(id) as (
             select member_id from akt_membership where tenant_id = $1 and container_id = $2
             union
             select m.member_id from akt_membership m
             join below b on m.container_id = b.id
             where m.tenant_id = $1
         )
         select exists (select 1 from below where id = $3) as reachable",
    )
    .bind::<SqlUuid, _>(tenant)
    .bind::<SqlUuid, _>(ancestor)
    .bind::<SqlUuid, _>(descendant)
    .get_result(conn)
    .await?;
    Ok(found.reachable)
}
