//! Login attempts and browser sessions.
//!
//! The tokens in a browser session are stored as the caller encrypted them;
//! this module never sees them in the clear.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::schema::{browser_session, login_attempt};

/// A sign-in a browser has started and not completed.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = login_attempt)]
pub struct NewLoginAttempt {
    pub state: String,
    pub code_verifier: String,
    pub return_to: String,
    pub expires_at: DateTime<Utc>,
}

/// Records a login attempt.
pub async fn insert_login_attempt(
    conn: &mut Connection,
    attempt: &NewLoginAttempt,
) -> QueryResult<()> {
    diesel::insert_into(login_attempt::table)
        .values(attempt)
        .execute(conn)
        .await
        .map(drop)
}

/// Removes and returns the login attempt for `state`, unless it has expired
/// by `now`. Expired attempts are removed too, so a `state` is usable once.
pub async fn take_login_attempt(
    conn: &mut Connection,
    state: &str,
    now: DateTime<Utc>,
) -> QueryResult<Option<NewLoginAttempt>> {
    let taken: Option<NewLoginAttempt> = diesel::delete(login_attempt::table.find(state))
        .returning(NewLoginAttempt::as_returning())
        .get_result(conn)
        .await
        .optional()?;
    Ok(taken.filter(|attempt| attempt.expires_at > now))
}

/// Removes login attempts that expired before `now`, and returns how many.
pub async fn delete_expired_login_attempts(
    conn: &mut Connection,
    now: DateTime<Utc>,
) -> QueryResult<usize> {
    diesel::delete(login_attempt::table.filter(login_attempt::expires_at.le(now)))
        .execute(conn)
        .await
}

/// A browser session as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = browser_session)]
pub struct BrowserSession {
    pub id_hash: Vec<u8>,
    pub subject_id: Uuid,
    pub tenant_id: Uuid,
    pub access_token: Vec<u8>,
    pub refresh_token: Vec<u8>,
    pub access_expires_at: DateTime<Utc>,
}

/// Stores a new session.
pub async fn insert_session(conn: &mut Connection, session: &BrowserSession) -> QueryResult<()> {
    diesel::insert_into(browser_session::table)
        .values(session)
        .execute(conn)
        .await
        .map(drop)
}

/// The session with the hash `id_hash`.
pub async fn find_session(
    conn: &mut Connection,
    id_hash: &[u8],
) -> QueryResult<Option<BrowserSession>> {
    browser_session::table
        .find(id_hash)
        .select(BrowserSession::as_select())
        .first(conn)
        .await
        .optional()
}

/// The session with the hash `id_hash`, locked until the surrounding
/// transaction ends, so that only one transaction at a time, on any server
/// instance, can refresh it.
pub async fn lock_session(
    conn: &mut Connection,
    id_hash: &[u8],
) -> QueryResult<Option<BrowserSession>> {
    browser_session::table
        .find(id_hash)
        .select(BrowserSession::as_select())
        .for_update()
        .first(conn)
        .await
        .optional()
}

/// Replaces a session's tokens after a refresh.
pub async fn update_tokens(
    conn: &mut Connection,
    id_hash: &[u8],
    access_token: &[u8],
    refresh_token: &[u8],
    access_expires_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> QueryResult<()> {
    diesel::update(browser_session::table.find(id_hash))
        .set((
            browser_session::access_token.eq(access_token),
            browser_session::refresh_token.eq(refresh_token),
            browser_session::access_expires_at.eq(access_expires_at),
            browser_session::refreshed_at.eq(now),
        ))
        .execute(conn)
        .await
        .map(drop)
}

/// Removes a session and returns it, if it existed.
pub async fn delete_session(
    conn: &mut Connection,
    id_hash: &[u8],
) -> QueryResult<Option<BrowserSession>> {
    diesel::delete(browser_session::table.find(id_hash))
        .returning(BrowserSession::as_returning())
        .get_result(conn)
        .await
        .optional()
}
