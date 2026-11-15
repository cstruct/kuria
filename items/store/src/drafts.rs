//! Live drafts (`live.*`): each akt's Yjs document, kept as a compacted
//! state and the updates after it, and every update with who made it
//! (`live.history`). What the bytes mean is the engine's (`kuria_engine::live`).

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde_json::Value as Json;
use uuid::Uuid;

use crate::Connection;
use crate::schema::{akt_draft, akt_draft_update};

/// An akt's live draft, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_draft)]
pub struct DraftRow {
    pub tenant_id: Uuid,
    pub akt_id: Uuid,
    /// The draft as of the update `compacted_seq`, as one Yjs v1 update.
    pub state: Vec<u8>,
    /// What the whole draft reads as, as the akt's fields' JSON.
    pub fields: Json,
    pub compacted_seq: i64,
    /// The last update the latest release took.
    pub released_seq: i64,
    pub updated_at: DateTime<Utc>,
}

/// One change to a live draft, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_draft_update)]
pub struct UpdateRow {
    pub tenant_id: Uuid,
    pub akt_id: Uuid,
    pub seq: i64,
    /// The Yjs v1 update.
    pub update: Vec<u8>,
    pub by: Uuid,
    pub at: DateTime<Utc>,
    /// The keys of the fields it changed, as a JSON list.
    pub fields: Json,
}

/// Stores a new live draft.
pub async fn insert(conn: &mut Connection, row: &DraftRow) -> QueryResult<()> {
    diesel::insert_into(akt_draft::table)
        .values(row)
        .execute(conn)
        .await
        .map(drop)
}

/// `akt`'s live draft and the updates after its compacted state, oldest
/// first; `None` if it has none yet.
pub async fn load(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
) -> QueryResult<Option<(DraftRow, Vec<UpdateRow>)>> {
    let Some(draft) = akt_draft::table
        .filter(akt_draft::tenant_id.eq(tenant))
        .filter(akt_draft::akt_id.eq(akt))
        .select(DraftRow::as_select())
        .first(conn)
        .await
        .optional()?
    else {
        return Ok(None);
    };
    let updates = updates_after(conn, tenant, akt, draft.compacted_seq).await?;
    Ok(Some((draft, updates)))
}

/// What the live drafts of `akter` read as, by akt, for those that have
/// one.
pub async fn fields_of(conn: &mut Connection, akter: &[Uuid]) -> QueryResult<Vec<(Uuid, Json)>> {
    if akter.is_empty() {
        return Ok(Vec::new());
    }
    akt_draft::table
        .filter(akt_draft::akt_id.eq_any(akter))
        .select((akt_draft::akt_id, akt_draft::fields))
        .load(conn)
        .await
}

/// `akt`'s live draft's updates after `seq`, oldest first.
pub async fn updates_after(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    seq: i64,
) -> QueryResult<Vec<UpdateRow>> {
    akt_draft_update::table
        .filter(akt_draft_update::tenant_id.eq(tenant))
        .filter(akt_draft_update::akt_id.eq(akt))
        .filter(akt_draft_update::seq.gt(seq))
        .order(akt_draft_update::seq)
        .select(UpdateRow::as_select())
        .load(conn)
        .await
}

/// Appends `update`, made by `by` at `at` and changing the fields `keys`,
/// to `akt`'s live draft, which then reads as `fields`, and answers its
/// sequence number. The caller holds the akt's row lock.
#[allow(
    clippy::too_many_arguments,
    reason = "one update and what the draft reads as after it"
)]
pub async fn append(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    update: &[u8],
    by: Uuid,
    at: DateTime<Utc>,
    keys: &[String],
    fields: &Json,
) -> QueryResult<i64> {
    let last: Option<i64> = akt_draft_update::table
        .filter(akt_draft_update::akt_id.eq(akt))
        .select(diesel::dsl::max(akt_draft_update::seq))
        .first(conn)
        .await?;
    let seq = last.unwrap_or(0) + 1;
    diesel::insert_into(akt_draft_update::table)
        .values(&UpdateRow {
            tenant_id: tenant,
            akt_id: akt,
            seq,
            update: update.to_vec(),
            by,
            at,
            fields: Json::from(keys.to_vec()),
        })
        .execute(conn)
        .await?;
    diesel::update(
        akt_draft::table
            .filter(akt_draft::tenant_id.eq(tenant))
            .filter(akt_draft::akt_id.eq(akt)),
    )
    .set((akt_draft::fields.eq(fields), akt_draft::updated_at.eq(at)))
    .execute(conn)
    .await?;
    Ok(seq)
}

/// Makes `state` `akt`'s live draft as of its update `through`, so that
/// loading it reads only the updates after.
pub async fn compact(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    state: &[u8],
    through: i64,
) -> QueryResult<()> {
    diesel::update(
        akt_draft::table
            .filter(akt_draft::tenant_id.eq(tenant))
            .filter(akt_draft::akt_id.eq(akt)),
    )
    .set((
        akt_draft::state.eq(state),
        akt_draft::compacted_seq.eq(through),
    ))
    .execute(conn)
    .await
    .map(drop)
}

/// Records that a release took `akt`'s live draft as of its update
/// `through`.
pub async fn released(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    through: i64,
) -> QueryResult<()> {
    diesel::update(
        akt_draft::table
            .filter(akt_draft::tenant_id.eq(tenant))
            .filter(akt_draft::akt_id.eq(akt)),
    )
    .set(akt_draft::released_seq.eq(through))
    .execute(conn)
    .await
    .map(drop)
}

/// The sequence number of `akt`'s live draft's last update a release took,
/// if it has a live draft.
pub async fn released_seq(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
) -> QueryResult<Option<i64>> {
    akt_draft::table
        .filter(akt_draft::tenant_id.eq(tenant))
        .filter(akt_draft::akt_id.eq(akt))
        .select(akt_draft::released_seq)
        .first(conn)
        .await
        .optional()
}
