//! Moving akter in their tenant's one priority order.

use chrono::Utc;
use diesel_async::AsyncConnection;
use kuria_engine::rank_between;
use kuria_store::akter::{self as store, AktRow};
use kuria_store::containers;
use uuid::Uuid;

use super::{AktError, Akter, Caller, require, require_read, type_of};
use crate::authorize::Permission;

/// Where to move an akt: directly before or directly after another one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place<'a> {
    Before(&'a str),
    After(&'a str),
}

impl Akter {
    /// Moves the akt `number` directly before or after another akt, in every
    /// container and on every board at once, since they all order by rank.
    // r[impl order.move]
    // r[impl order.not-recorded]
    // r[impl order.concurrent]
    // r[impl authz.move]
    pub async fn move_akt(
        &self,
        caller: Caller<'_>,
        number: &str,
        place: Place<'_>,
    ) -> Result<AktRow, AktError> {
        let Caller { actor, auth } = caller;
        let tenant: Uuid = actor.tenant.into();
        let neighbour_number = match place {
            Place::Before(n) | Place::After(n) => n,
        };
        if neighbour_number == number {
            return Err(AktError::SelfMove);
        }
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                // Every change to a rank holds this until it commits.
                containers::lock_order(conn, tenant).await?;
                let mut akt = store::akt_by_number(conn, tenant, number)
                    .await?
                    .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
                let neighbour = store::akt_by_number(conn, tenant, neighbour_number)
                    .await?
                    .ok_or_else(|| AktError::NotFound(neighbour_number.to_owned()))?;
                let ty = type_of(conn, tenant, &akt).await?;
                require_read(auth, &ty, &akt.number).await?;
                let neighbour_type = type_of(conn, tenant, &neighbour).await?;
                require_read(auth, &neighbour_type, &neighbour.number).await?;
                require(
                    auth,
                    Permission::Edit {
                        key: ty.ty.key().to_owned(),
                    },
                )
                .await?;

                // The akt's own rank doesn't count as a neighbour: if it already
                // sits right there, nothing moves.
                let (before, after) = match place {
                    Place::Before(_) => (
                        containers::rank_before(conn, tenant, &neighbour.rank).await?,
                        Some(neighbour.rank.clone()),
                    ),
                    Place::After(_) => (
                        Some(neighbour.rank.clone()),
                        containers::rank_after(conn, tenant, &neighbour.rank).await?,
                    ),
                };
                if before.as_deref() == Some(akt.rank.as_str())
                    || after.as_deref() == Some(akt.rank.as_str())
                {
                    return Ok(akt);
                }
                let rank = rank_between(before.as_deref(), after.as_deref())
                    .map_err(|e| AktError::Internal(e.to_string()))?;
                containers::set_rank(conn, tenant, akt.id, &rank).await?;
                containers::append_rank_event(conn, tenant, akt.id, Utc::now()).await?;
                akt.rank = rank;
                Ok(akt)
            })
            .await;
        self.committed(result)
    }
}
