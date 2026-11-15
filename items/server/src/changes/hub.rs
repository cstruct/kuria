//! Fanning change events out to the subscriptions of each tenant.
//!
//! A stream entry in Valkey only says that something changed. The hub then
//! reads the tenant's change events after the last one it broadcast, from
//! Postgres, in sequence order, so entries published out of order by
//! different instances still reach subscribers in order.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kuria_store::Pool;
use kuria_store::akter::{self as store, Change};
use redis::aio::{ConnectionManager, MultiplexedConnection};
use redis::streams::StreamReadReply;
use tokio::sync::{Notify, broadcast};
use uuid::Uuid;

use super::relay::stream_key;

/// How many events a subscriber may fall behind before it is caught up
/// from Postgres.
const BACKLOG: usize = 1_024;
/// How many change events one read from Postgres takes.
const PAGE: i64 = 500;
/// How long one wait on Valkey blocks.
const BLOCK_MS: usize = 1_000;
/// How often every tenant is read from Postgres anyway, in case a wake-up
/// was lost.
const SWEEP: Duration = Duration::from_secs(5);

struct Tenant {
    sender: broadcast::Sender<Arc<Change>>,
    /// The last sequence number broadcast.
    last_seq: i64,
    /// The last stream entry seen.
    stream_id: String,
}

struct Inner {
    pool: Pool,
    client: redis::Client,
    tenants: Mutex<HashMap<Uuid, Tenant>>,
    joined: Notify,
}

/// One instance's fan-out of change events, per tenant.
#[derive(Clone)]
pub struct Hub {
    inner: Arc<Inner>,
}

/// A subscriber's place in its tenant's hub.
pub struct Subscription {
    /// Every change event broadcast from now on, in sequence order.
    pub receiver: broadcast::Receiver<Arc<Change>>,
    /// The last sequence number broadcast before the subscriber joined: it
    /// receives every event after it.
    pub joined_at: i64,
}

impl Hub {
    /// A hub reading from `pool`, woken through the Valkey server `client`
    /// names. Call [`Hub::run`] to start it.
    #[must_use]
    pub fn new(pool: Pool, client: redis::Client) -> Self {
        Self {
            inner: Arc::new(Inner {
                pool,
                client,
                tenants: Mutex::new(HashMap::new()),
                joined: Notify::new(),
            }),
        }
    }

    /// The Valkey server the hub is woken through, which live drafts reach
    /// other instances through too (`live.instances`).
    #[must_use]
    pub fn valkey(&self) -> redis::Client {
        self.inner.client.clone()
    }

    /// Joins `tenant`'s hub.
    pub async fn subscribe(&self, tenant: Uuid) -> Result<Subscription, String> {
        if let Some(subscription) = self.join(tenant) {
            return Ok(subscription);
        }
        // A new tenant: start from its current last event and stream entry.
        let mut conn = self.inner.pool.get().await.map_err(|e| e.to_string())?;
        let last_seq = store::last_change_seq(&mut conn, tenant)
            .await
            .map_err(|e| e.to_string())?;
        let mut valkey = ConnectionManager::new(self.inner.client.clone())
            .await
            .map_err(|e| e.to_string())?;
        let last: Vec<(String, HashMap<String, String>)> = redis::cmd("XREVRANGE")
            .arg(stream_key(tenant))
            .arg("+")
            .arg("-")
            .arg("COUNT")
            .arg(1)
            .query_async(&mut valkey)
            .await
            .map_err(|e| e.to_string())?;
        let stream_id = last
            .first()
            .map_or_else(|| "0-0".to_owned(), |(id, _)| id.clone());
        {
            let mut tenants = self.tenants();
            tenants.entry(tenant).or_insert_with(|| Tenant {
                sender: broadcast::channel(BACKLOG).0,
                last_seq,
                stream_id,
            });
        }
        self.inner.joined.notify_one();
        self.join(tenant)
            .ok_or_else(|| "the hub vanished".to_owned())
    }

    fn join(&self, tenant: Uuid) -> Option<Subscription> {
        let tenants = self.tenants();
        let hub = tenants.get(&tenant)?;
        Some(Subscription {
            receiver: hub.sender.subscribe(),
            joined_at: hub.last_seq,
        })
    }

    fn tenants(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, Tenant>> {
        self.inner
            .tenants
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Waits on the tenants' streams and broadcasts what changed, until the
    /// process stops.
    // r[impl changes.subscribe]
    pub async fn run(self) {
        let mut last_sweep = Instant::now();
        let mut valkey: Option<MultiplexedConnection> = None;
        loop {
            // Tenants nobody listens to any more are dropped.
            let watched: Vec<(Uuid, String)> = {
                let mut tenants = self.tenants();
                tenants.retain(|_, hub| hub.sender.receiver_count() > 0);
                tenants
                    .iter()
                    .map(|(tenant, hub)| (*tenant, hub.stream_id.clone()))
                    .collect()
            };
            if watched.is_empty() {
                self.inner.joined.notified().await;
                continue;
            }
            if valkey.is_none() {
                // A connection of its own: a blocking read holds it.
                match self.inner.client.get_multiplexed_async_connection().await {
                    Ok(conn) => valkey = Some(conn),
                    Err(e) => {
                        tracing::warn!("reaching Valkey for change events: {e}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        self.sweep(&watched).await;
                        continue;
                    }
                }
            }
            let mut read = redis::cmd("XREAD");
            read.arg("COUNT")
                .arg(100)
                .arg("BLOCK")
                .arg(BLOCK_MS)
                .arg("STREAMS");
            for (tenant, _) in &watched {
                read.arg(stream_key(*tenant));
            }
            for (_, id) in &watched {
                read.arg(id);
            }
            let Some(conn) = valkey.as_mut() else {
                continue;
            };
            match read.query_async::<Option<StreamReadReply>>(conn).await {
                Ok(Some(reply)) => {
                    for key in reply.keys {
                        let Some((tenant, _)) =
                            watched.iter().find(|(t, _)| stream_key(*t) == key.key)
                        else {
                            continue;
                        };
                        if let Some(last) = key.ids.last() {
                            if let Some(hub) = self.tenants().get_mut(tenant) {
                                hub.stream_id.clone_from(&last.id);
                            }
                            self.wake(*tenant).await;
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("reading change events from Valkey: {e}");
                    valkey = None;
                }
            }
            if last_sweep.elapsed() >= SWEEP {
                self.sweep(&watched).await;
                last_sweep = Instant::now();
            }
        }
    }

    async fn sweep(&self, watched: &[(Uuid, String)]) {
        for (tenant, _) in watched {
            self.wake(*tenant).await;
        }
    }

    /// Reads `tenant`'s change events after the last one broadcast, and
    /// broadcasts them in order.
    pub async fn wake(&self, tenant: Uuid) {
        let Some(mut after) = self.tenants().get(&tenant).map(|hub| hub.last_seq) else {
            return;
        };
        let Ok(mut conn) = self.inner.pool.get().await else {
            tracing::warn!("checking out a connection for change events");
            return;
        };
        loop {
            let changes = match store::changes_after(&mut conn, tenant, after, PAGE).await {
                Ok(changes) => changes,
                Err(e) => {
                    tracing::warn!("reading change events: {e}");
                    return;
                }
            };
            let full = changes.len() == usize::try_from(PAGE).unwrap_or(usize::MAX);
            let mut tenants = self.tenants();
            let Some(hub) = tenants.get_mut(&tenant) else {
                return;
            };
            for change in changes {
                // Another wake may have broadcast these meanwhile.
                if change.seq <= hub.last_seq {
                    continue;
                }
                hub.last_seq = change.seq;
                after = change.seq;
                // No receivers is fine: they may all have just left.
                let _ = hub.sender.send(Arc::new(change));
            }
            drop(tenants);
            if !full {
                return;
            }
        }
    }
}
