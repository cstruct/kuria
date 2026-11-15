//! Attachments' metadata, and which akter hold them.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde_json::Value as Json;
use uuid::Uuid;

use crate::Connection;
use crate::schema::{akt, akt_attachment, akt_held, akt_type_version, attachment};

/// An attachment, as stored.
#[derive(Clone, Debug, PartialEq, Eq, Insertable, Queryable, Selectable)]
#[diesel(table_name = attachment)]
pub struct AttachmentRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub media_type: String,
    pub size: i64,
    pub sha256: Vec<u8>,
    pub uploaded_by: Uuid,
    pub uploaded_at: DateTime<Utc>,
    pub sbom: Option<Json>,
}

/// Stores an attachment's metadata.
pub async fn insert(conn: &mut Connection, row: &AttachmentRow) -> QueryResult<()> {
    diesel::insert_into(attachment::table)
        .values(row)
        .execute(conn)
        .await
        .map(drop)
}

/// The tenant's attachments `ids`.
pub async fn find(
    conn: &mut Connection,
    tenant: Uuid,
    ids: &[Uuid],
) -> QueryResult<Vec<AttachmentRow>> {
    attachment::table
        .filter(attachment::tenant_id.eq(tenant))
        .filter(attachment::id.eq_any(ids))
        .select(AttachmentRow::as_select())
        .load(conn)
        .await
}

/// The akt type keys of the akter that hold the attachment `id`, or held
/// it at any revision, in a field or as a diagram (`attachment.download`).
pub async fn holder_types(
    conn: &mut Connection,
    tenant: Uuid,
    id: Uuid,
) -> QueryResult<Vec<String>> {
    akt_held::table
        .inner_join(akt::table.on(akt::id.eq(akt_held::akt_id)))
        .inner_join(akt_type_version::table.on(akt_type_version::id.eq(akt::type_version_id)))
        .filter(akt_held::tenant_id.eq(tenant))
        .filter(akt_held::attachment_id.eq(id))
        .select(akt_type_version::key)
        .distinct()
        .load(conn)
        .await
}

/// Records that `akt` holds each of `ids`, as it does from now on, never
/// letting go (`attachment.download`).
pub async fn keep_held(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    ids: &[Uuid],
) -> QueryResult<()> {
    for id in ids {
        diesel::insert_into(akt_held::table)
            .values((
                akt_held::tenant_id.eq(tenant),
                akt_held::akt_id.eq(akt),
                akt_held::attachment_id.eq(id),
            ))
            .on_conflict_do_nothing()
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// Records that `akt`'s fields hold `held`, as field and attachment pairs,
/// replacing what was recorded for it before, and that it has held them
/// (`keep_held`).
pub async fn set_held(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    held: &[(String, Uuid)],
) -> QueryResult<()> {
    diesel::delete(akt_attachment::table.filter(akt_attachment::akt_id.eq(akt)))
        .execute(conn)
        .await?;
    for (field, id) in held {
        diesel::insert_into(akt_attachment::table)
            .values((
                akt_attachment::tenant_id.eq(tenant),
                akt_attachment::akt_id.eq(akt),
                akt_attachment::field.eq(field),
                akt_attachment::attachment_id.eq(id),
            ))
            .execute(conn)
            .await?;
    }
    let ids: Vec<Uuid> = held.iter().map(|(_, id)| *id).collect();
    keep_held(conn, tenant, akt, &ids).await
}
