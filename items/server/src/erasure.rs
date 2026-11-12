//! Deleting what Kuria holds about subjects Konklav erases.
//!
//! Live notices are the fast path. The durable path is Konklav's list of
//! erased subjects, read from the time stored in Kuria's database whenever
//! the relying client says erasures may have been missed, which it does on
//! every start. Only that read moves the stored time.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use konklav_relying_client::{ErasureNotice, RelyingClient, SubjectErased};
use kuria_store::{Pool, erasure};
use tokio::sync::mpsc::UnboundedReceiver;

/// Where the list of erased subjects comes from: Konklav, through the
/// relying client, or a stand-in in tests.
pub trait ErasedSubjects: Send + Sync + 'static {
    /// The subjects erased at or after `since` (Unix seconds), or all of
    /// them.
    fn erased_subjects(
        &self,
        since: Option<i64>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SubjectErased>, String>> + Send + '_>>;
}

impl ErasedSubjects for RelyingClient {
    fn erased_subjects(
        &self,
        since: Option<i64>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SubjectErased>, String>> + Send + '_>> {
        Box::pin(async move {
            RelyingClient::erased_subjects(self, since)
                .await
                .map_err(|e| e.to_string())
        })
    }
}

/// Handles erasure notices until their sender goes away. A catch-up that
/// fails is tried again every `retry` until it succeeds, while live notices
/// keep being handled.
pub async fn handle_erasures(
    pool: Pool,
    source: impl ErasedSubjects,
    mut notices: UnboundedReceiver<ErasureNotice>,
    retry: Duration,
) {
    let mut catch_up_pending = false;
    loop {
        let notice = if catch_up_pending {
            tokio::select! {
                notice = notices.recv() => notice,
                () = tokio::time::sleep(retry) => {
                    catch_up_pending = !caught_up(&pool, &source).await;
                    continue;
                }
            }
        } else {
            notices.recv().await
        };
        match notice {
            None => return,
            Some(ErasureNotice::Erased(erased)) => {
                // A failure here is harmless: the next catch-up repeats it.
                if let Err(e) = erase(&pool, erased.subject.into()).await {
                    tracing::warn!("deleting an erased subject's data: {e}");
                }
            }
            Some(ErasureNotice::Missed { .. }) => {
                catch_up_pending = !caught_up(&pool, &source).await;
            }
        }
    }
}

/// Deletes what Kuria holds about `subject`.
// r[impl erasure.browser-sessions]
async fn erase(pool: &Pool, subject: uuid::Uuid) -> Result<(), String> {
    let mut conn = pool.get().await.map_err(|e| e.to_string())?;
    let deleted = erasure::erase_subject(&mut conn, subject)
        .await
        .map_err(|e| e.to_string())?;
    if deleted > 0 {
        tracing::info!(%subject, deleted, "deleted an erased subject's browser sessions");
    }
    Ok(())
}

/// Reads the erased subjects from the stored time, deletes what Kuria holds
/// about them, and then moves the stored time forward. Whether it succeeded.
// r[impl erasure.catch-up]
// r[impl erasure.retry]
async fn caught_up(pool: &Pool, source: &impl ErasedSubjects) -> bool {
    let result = async {
        let since = {
            let mut conn = pool.get().await.map_err(|e| e.to_string())?;
            erasure::handled_until(&mut conn)
                .await
                .map_err(|e| e.to_string())?
        };
        let erased = source.erased_subjects(since).await?;
        for subject in &erased {
            erase(pool, subject.subject.into()).await?;
        }
        if let Some(latest) = erased.iter().map(|e| e.erased_at).max() {
            let mut conn = pool.get().await.map_err(|e| e.to_string())?;
            erasure::advance_handled_until(&mut conn, latest)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(erased.len())
    };
    match result.await {
        Ok(read) => {
            tracing::info!(read, "caught up on Konklav's erasures");
            true
        }
        Err(e) => {
            tracing::warn!("catching up on Konklav's erasures, trying again later: {e}");
            false
        }
    }
}
