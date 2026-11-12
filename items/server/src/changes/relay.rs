//! Publishing committed change events to Valkey.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use diesel_async::AsyncConnection;
use kuria_store::Pool;
use kuria_store::akter as store;
use redis::aio::ConnectionManager;
use tokio::sync::Notify;
use uuid::Uuid;

/// How many change events one pass publishes at most.
const BATCH: i64 = 500;
/// How often the relay looks for unpublished events without being woken.
const POLL: Duration = Duration::from_millis(500);
/// How many entries a tenant's stream keeps.
const STREAM_LENGTH: usize = 10_000;

/// Why a pass of the relay failed. The events it took stay unpublished.
#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("the database: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("checking out a connection: {0}")]
    Checkout(#[from] diesel_async::pooled_connection::deadpool::PoolError),
    #[error("valkey: {0}")]
    Valkey(#[from] redis::RedisError),
}

/// The Valkey stream of `tenant`'s change events.
#[must_use]
pub fn stream_key(tenant: Uuid) -> String {
    format!("kuria:changes:{tenant}")
}

/// Publishes change events from the outbox to Valkey. Every instance runs
/// one; they never publish the same row at once.
#[derive(Clone)]
pub struct Relay {
    pool: Pool,
    valkey: ConnectionManager,
    wake: Arc<Notify>,
}

impl Relay {
    #[must_use]
    pub fn new(pool: Pool, valkey: ConnectionManager) -> Self {
        Self {
            pool,
            valkey,
            wake: Arc::new(Notify::new()),
        }
    }

    /// What wakes the relay: notify it after a write commits, so the new
    /// events are published at once.
    #[must_use]
    pub fn waker(&self) -> Arc<Notify> {
        self.wake.clone()
    }

    /// Publishes until the process stops: when woken, and every half second.
    pub async fn run(self) {
        loop {
            match self.publish_pending().await {
                // A full batch: there may be more.
                Ok(n) if n == usize::try_from(BATCH).unwrap_or(usize::MAX) => continue,
                Ok(_) => {}
                Err(e) => tracing::warn!("publishing change events: {e}"),
            }
            tokio::select! {
                () = self.wake.notified() => {}
                () = tokio::time::sleep(POLL) => {}
            }
        }
    }

    /// Publishes one batch of unpublished change events, and returns how
    /// many it published.
    // r[impl changes.publish]
    pub async fn publish_pending(&self) -> Result<usize, RelayError> {
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let valkey = self.valkey.clone();
        conn.transaction::<_, RelayError, _>(async |conn| {
            let events = store::take_unpublished(conn, BATCH).await?;
            if events.is_empty() {
                return Ok(0);
            }
            let mut pipe = redis::pipe();
            for (tenant, seq) in &events {
                // r[impl changes.identifiers]
                pipe.cmd("XADD")
                    .arg(stream_key(*tenant))
                    .arg("MAXLEN")
                    .arg("~")
                    .arg(STREAM_LENGTH)
                    .arg("*")
                    .arg("seq")
                    .arg(*seq)
                    .ignore();
            }
            let mut valkey = valkey;
            // If Valkey refuses, the transaction rolls back and the rows
            // stay unpublished for the next pass.
            pipe.query_async::<()>(&mut valkey).await?;
            store::mark_published(conn, &events, Utc::now()).await?;
            Ok(events.len())
        })
        .await
    }
}
