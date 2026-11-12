//! The `changes` subscription.

use std::sync::Arc;
use std::time::Duration;

use async_graphql::{Context, Enum, ID, SimpleObject, Subscription};
use futures_util::Stream;
use kuria_store::Pool;
use kuria_store::akter::{self as store, Change};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use super::Principal;
use crate::akter::Akter;
use crate::authorize::{Authorizer, Permission};
use crate::changes::Hub;

/// How many change events one catch-up read takes.
const PAGE: i64 = 500;

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// The akt has a new revision.
    Revision,
    /// The akt moved in its tenant's order.
    Rank,
}

/// That an akt changed. Fetch what you need about it with a query.
#[derive(SimpleObject, Clone)]
pub struct ChangeEvent {
    /// The change's place in its tenant's sequence: pass the last one seen
    /// as `after` when reconnecting.
    seq: i64,
    kind: ChangeKind,
    akt: ID,
    number: String,
    /// The new revision, for a `REVISION` change.
    revision: Option<i32>,
}

impl From<&Change> for ChangeEvent {
    fn from(change: &Change) -> Self {
        Self {
            seq: change.seq,
            kind: if change.kind == "rank" {
                ChangeKind::Rank
            } else {
                ChangeKind::Revision
            },
            akt: ID(change.akt_id.to_string()),
            number: change.number.clone(),
            revision: change.revision,
        }
    }
}

#[derive(Default)]
pub struct ChangesSubscription;

#[Subscription]
impl ChangesSubscription {
    /// The tenant's changes to akter the subscriber may read, in sequence
    /// order: first every one after `after`, if given, then new ones as they
    /// happen. Ends when the connection's access token expires.
    // r[impl changes.subscribe]
    // r[impl changes.catch-up]
    // r[impl changes.lagging]
    // r[impl changes.expiry]
    async fn changes(
        &self,
        ctx: &Context<'_>,
        after: Option<i64>,
    ) -> async_graphql::Result<impl Stream<Item = ChangeEvent> + use<>> {
        let principal = *ctx.data::<Principal>()?;
        let auth = ctx.data::<Arc<Authorizer>>()?.clone();
        let pool = ctx.data::<Akter>()?.pool().clone();
        let hub = ctx
            .data::<Option<Hub>>()?
            .clone()
            .ok_or("subscriptions are not available")?;
        let tenant: Uuid = principal.tenant.into();
        let mut subscription = hub.subscribe(tenant).await?;
        let mut last = after.unwrap_or(subscription.joined_at);
        let remaining = principal.expires_at - chrono::Utc::now().timestamp();
        let deadline = tokio::time::Instant::now()
            + Duration::from_secs(u64::try_from(remaining).unwrap_or(0));

        Ok(async_stream::stream! {
            'catch_up: loop {
                // Everything after `last`, from Postgres.
                loop {
                    let Ok(page) = page_after(&pool, tenant, last).await else {
                        break 'catch_up;
                    };
                    let full = page.len() == usize::try_from(PAGE).unwrap_or(usize::MAX);
                    for change in &page {
                        last = change.seq;
                        if visible(&auth, change).await {
                            yield ChangeEvent::from(change);
                        }
                    }
                    if !full {
                        break;
                    }
                }
                // Then what the hub broadcasts.
                loop {
                    let received = tokio::select! {
                        () = tokio::time::sleep_until(deadline) => break 'catch_up,
                        received = subscription.receiver.recv() => received,
                    };
                    match step(last, &received) {
                        Step::Skip => {}
                        Step::CatchUp => continue 'catch_up,
                        Step::End => break 'catch_up,
                        Step::Deliver => {
                            let change = received.expect("delivered events are received");
                            last = change.seq;
                            if visible(&auth, &change).await {
                                yield ChangeEvent::from(change.as_ref());
                            }
                        }
                    }
                }
            }
        })
    }
}

/// What to do with what the hub sent.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// Already delivered.
    Skip,
    /// The next event: deliver it.
    Deliver,
    /// Events are missing: read them from Postgres first.
    CatchUp,
    /// The hub is gone.
    End,
}

/// What to do with `received` when `last` was delivered last. Sequence
/// numbers have no gaps, so one missing means the hub ran ahead, and a
/// receiver that lagged lost events: both catch up from Postgres.
// r[impl changes.lagging]
fn step(last: i64, received: &Result<Arc<Change>, RecvError>) -> Step {
    match received {
        Ok(change) if change.seq <= last => Step::Skip,
        Ok(change) if change.seq == last + 1 => Step::Deliver,
        Ok(_) | Err(RecvError::Lagged(_)) => Step::CatchUp,
        Err(RecvError::Closed) => Step::End,
    }
}

async fn page_after(pool: &Pool, tenant: Uuid, after: i64) -> Result<Vec<Change>, String> {
    let mut conn = pool.get().await.map_err(|e| e.to_string())?;
    store::changes_after(&mut conn, tenant, after, PAGE)
        .await
        .map_err(|e| e.to_string())
}

/// Whether the subscriber may read the akt that changed. Fails closed.
// r[impl changes.visibility]
async fn visible(auth: &Authorizer, change: &Change) -> bool {
    auth.allows(&Permission::Read {
        key: change.akt_type.clone(),
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(seq: i64) -> Arc<Change> {
        Arc::new(Change {
            seq,
            akt_id: Uuid::nil(),
            kind: "revision".to_owned(),
            revision: Some(1),
            number: "A-1".to_owned(),
            akt_type: "a".to_owned(),
        })
    }

    // r[verify changes.lagging]
    #[test]
    fn catches_up_from_postgres_when_events_were_missed() {
        assert_eq!(step(4, &Ok(change(5))), Step::Deliver);
        assert_eq!(step(4, &Ok(change(4))), Step::Skip);
        assert_eq!(step(4, &Ok(change(2))), Step::Skip);
        assert_eq!(step(4, &Ok(change(7))), Step::CatchUp);
        assert_eq!(step(4, &Err(RecvError::Lagged(3))), Step::CatchUp);
        assert_eq!(step(4, &Err(RecvError::Closed)), Step::End);
    }
}
