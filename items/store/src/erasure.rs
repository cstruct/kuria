//! What erasing a subject deletes, and the time Kuria has handled Konklav's
//! erasures up to.

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::schema::{browser_session, contact, erasure_watermark, person};

/// Deletes everything Kuria holds about `subject`: their browser sessions,
/// their name and email, and the names and emails of the contacts linked to
/// them. Returns how many rows went.
pub async fn erase_subject(conn: &mut Connection, subject: Uuid) -> QueryResult<usize> {
    let sessions =
        diesel::delete(browser_session::table.filter(browser_session::subject_id.eq(subject)))
            .execute(conn)
            .await?;
    // r[impl person.erasure]
    let people = diesel::delete(person::table.find(subject))
        .execute(conn)
        .await?;
    // r[impl contact.erasure]
    let contacts = diesel::update(contact::table.filter(contact::linked_subject.eq(subject)))
        .set((
            contact::name.eq(None::<String>),
            contact::email.eq(None::<String>),
        ))
        .execute(conn)
        .await?;
    Ok(sessions + people + contacts)
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
