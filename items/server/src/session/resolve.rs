//! Finding a session's access token, and refreshing it when it is about to
//! expire.

use chrono::{DateTime, TimeDelta, Utc};
use diesel_async::AsyncConnection;
use kuria_store::sessions;

use super::{BrowserSessions, id_hash};
use crate::oidc::{ProviderError, Tokens};

/// How long before it expires an access token is refreshed.
const REFRESH_MARGIN: TimeDelta = TimeDelta::seconds(60);

/// What a session cookie resolves to.
#[derive(Debug)]
pub(crate) enum Resolved {
    /// The session's current access token.
    Token(String),
    /// No session: unknown, ended, or refused by Konklav. It is deleted.
    NoSession,
    /// Konklav could not be reached to refresh the token.
    Unavailable,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ResolveError {
    #[error("checking out a database connection: {0}")]
    Checkout(#[from] diesel_async::pooled_connection::deadpool::PoolError),
    #[error("the database: {0}")]
    Database(#[from] diesel::result::Error),
}

/// When an access token from `tokens`, received at `now`, should be
/// considered expired.
pub(crate) fn expires_at(tokens: &Tokens, now: DateTime<Utc>) -> DateTime<Utc> {
    // Without `expires_in`, treat the token as expiring now, so it is
    // refreshed before each use rather than presented after it expired.
    now + TimeDelta::seconds(tokens.expires_in.unwrap_or(0).max(0))
}

impl BrowserSessions {
    /// The access token of the session `cookie` names, refreshed first if it
    /// expires within a minute.
    // r[impl session.request]
    // r[impl session.unknown]
    pub(crate) async fn access_token(&self, cookie: &str) -> Result<Resolved, ResolveError> {
        let hash = id_hash(cookie);
        let mut conn = self.pool.get().await?;
        let Some(session) = sessions::find_session(&mut conn, &hash).await? else {
            return Ok(Resolved::NoSession);
        };
        let now = Utc::now();
        if session.access_expires_at > now + REFRESH_MARGIN {
            return Ok(self.open_access(&hash, &session));
        }
        // r[impl session.refresh]
        // The row stays locked until the transaction ends, so a second
        // request for this session, on any instance, waits here and then
        // finds the token this one stored.
        let conn: &mut kuria_store::Connection = &mut conn;
        conn.transaction::<_, ResolveError, _>(async |conn| {
            {
                let Some(session) = sessions::lock_session(conn, &hash).await? else {
                    return Ok(Resolved::NoSession);
                };
                let now = Utc::now();
                if session.access_expires_at > now + REFRESH_MARGIN {
                    return Ok(self.open_access(&hash, &session));
                }
                let Ok(refresh_token) = self.cipher.open(&hash, &session.refresh_token) else {
                    sessions::delete_session(conn, &hash).await?;
                    return Ok(Resolved::NoSession);
                };
                match self.provider.refresh(&refresh_token).await {
                    Ok(tokens) => {
                        // Konklav rotates refresh tokens; keep the old one only
                        // if it sent none.
                        let refresh_token = tokens.refresh_token.as_deref().unwrap_or(&refresh_token);
                        sessions::update_tokens(
                            conn,
                            &hash,
                            &self.cipher.seal(&hash, &tokens.access_token),
                            &self.cipher.seal(&hash, refresh_token),
                            expires_at(&tokens, now),
                            now,
                        )
                        .await?;
                        Ok(Resolved::Token(tokens.access_token))
                    }
                    // r[impl session.refresh.failure]
                    Err(ProviderError::Refused(reason)) => {
                        tracing::info!(%reason, "Konklav refused a session's refresh token; ending the session");
                        sessions::delete_session(conn, &hash).await?;
                        Ok(Resolved::NoSession)
                    }
                    Err(ProviderError::Unavailable(reason)) => {
                        tracing::warn!(%reason, "could not refresh a session's access token");
                        Ok(Resolved::Unavailable)
                    }
                }
            }
        })
        .await
    }

    fn open_access(&self, hash: &[u8], session: &sessions::BrowserSession) -> Resolved {
        match self.cipher.open(hash, &session.access_token) {
            Ok(token) => Resolved::Token(token),
            // Under another key: the session can't be used. The next
            // refresh attempt deletes it.
            Err(_) => Resolved::NoSession,
        }
    }
}
