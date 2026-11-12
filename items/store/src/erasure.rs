//! What erasing a subject deletes, and the time Kuria has handled Konklav's
//! erasures up to.

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::schema::{browser_session, erasure_watermark};

/// Deletes everything Kuria holds about `subject`, and returns how many
/// rows went.
pub async fn erase_subject(conn: &mut Connection, subject: Uuid) -> QueryResult<usize> {
    diesel::delete(browser_session::table.filter(browser_session::subject_id.eq(subject)))
        .execute(conn)
        .await
}

/// The time erasures are handled up to, in Unix seconds, or `None` before
/// any have been.
pub async fn handled_until(conn: &mut Connection) -> QueryResult<Option<i64>> {
    erasure_watermark::table
        .select(erasure_watermark::handled_until)
        .first(conn)
        .await
        .optional()
}

/// Moves the time erasures are handled up to forward to `until`, and never
/// back, so instances catching up at once can't undo each other.
pub async fn advance_handled_until(conn: &mut Connection, until: i64) -> QueryResult<()> {
    diesel::insert_into(erasure_watermark::table)
        .values((
            erasure_watermark::id.eq(1),
            erasure_watermark::handled_until.eq(until),
        ))
        .on_conflict(erasure_watermark::id)
        .do_update()
        .set((
            erasure_watermark::handled_until.eq(diesel::dsl::sql::<diesel::sql_types::BigInt>(
                "greatest(erasure_watermark.handled_until, excluded.handled_until)",
            )),
            erasure_watermark::updated_at.eq(diesel::dsl::now),
        ))
        .execute(conn)
        .await
        .map(drop)
}
