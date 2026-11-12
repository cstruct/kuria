//! Adding members to containers and removing them.

use chrono::Utc;
use diesel_async::AsyncConnection;
use kuria_store::akter::AktRow;
use kuria_store::containers::{self, MembershipRow};
use serde_json::json;
use uuid::Uuid;

use super::{
    AktError, Akter, Caller, StoredType, is_final, membership_entry, record_membership, require,
    require_read, type_of,
};
use crate::authorize::Permission;

/// Both sides of a membership, locked and with their akt types, after the
/// checks every membership change makes.
struct Pair {
    container: AktRow,
    container_type: StoredType,
    member: AktRow,
    member_type: StoredType,
}

impl Akter {
    /// Adds the akt `member` to the container `container`.
    // r[impl container.add]
    // r[impl container.unversioned]
    pub async fn add_member(
        &self,
        caller: Caller<'_>,
        container: &str,
        member: &str,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        let Caller { actor, .. } = caller;
        let tenant: Uuid = actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        conn.transaction::<_, AktError, _>(async |conn| {
            let mut pair = lock_pair(conn, caller, tenant, container, member).await?;
            let Pair {
                container: c,
                container_type,
                member: m,
                member_type,
            } = &pair;

            // r[impl container.contains]
            let Some(contains) = container_type.ty.contains(member_type.ty.key()) else {
                return Err(AktError::NotContainable {
                    container: c.number.clone(),
                    member: member_type.ty.key().to_owned(),
                });
            };
            if containers::is_member(conn, tenant, c.id, m.id).await? {
                return Err(AktError::AlreadyMember {
                    container: c.number.clone(),
                    member: m.number.clone(),
                });
            }
            // r[impl container.exclusive]
            if contains.exclusive
                && let Some((other, _)) = containers::containers_of(conn, tenant, m.id)
                    .await?
                    .into_iter()
                    .find(|(_, key)| key == container_type.ty.key())
            {
                return Err(AktError::Exclusive {
                    member: m.number.clone(),
                    other: other.number,
                    akt_type: container_type.ty.key().to_owned(),
                });
            }
            // r[impl container.acyclic]
            if containers::contains(conn, tenant, m.id, c.id).await? {
                return Err(AktError::Cycle {
                    container: c.number.clone(),
                    member: m.number.clone(),
                });
            }

            let now = Utc::now();
            containers::insert_membership(
                conn,
                &MembershipRow {
                    tenant_id: tenant,
                    container_id: c.id,
                    member_id: m.id,
                    added_at: now,
                    added_by: actor.subject.into(),
                },
            )
            .await?;
            record_both(conn, &mut pair, caller, reason, "added", now).await?;
            Ok(pair.container)
        })
        .await
    }

    /// Removes the akt `member` from the container `container`.
    // r[impl container.remove]
    // r[impl container.unversioned]
    pub async fn remove_member(
        &self,
        caller: Caller<'_>,
        container: &str,
        member: &str,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        let tenant: Uuid = caller.actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        conn.transaction::<_, AktError, _>(async |conn| {
            let mut pair = lock_pair(conn, caller, tenant, container, member).await?;
            if !containers::delete_membership(conn, tenant, pair.container.id, pair.member.id)
                .await?
            {
                return Err(AktError::NotMember {
                    container: pair.container.number.clone(),
                    member: pair.member.number.clone(),
                });
            }
            record_both(conn, &mut pair, caller, reason, "removed", Utc::now()).await?;
            Ok(pair.container)
        })
        .await
    }
}

/// Locks both akter and checks what adding and removing have in common: both
/// exist and are readable, the caller may edit the container, and it isn't
/// frozen.
// r[impl authz.membership]
async fn lock_pair(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    tenant: Uuid,
    container: &str,
    member: &str,
) -> Result<Pair, AktError> {
    let auth = caller.auth;
    if container == member {
        // r[impl container.acyclic]
        return Err(AktError::Cycle {
            container: container.to_owned(),
            member: member.to_owned(),
        });
    }
    let mut locked = containers::lock_akter(conn, tenant, &[container, member]).await?;
    let mut take = |number: &str| {
        locked
            .iter()
            .position(|row| row.number == number)
            .map(|i| locked.swap_remove(i))
            .ok_or_else(|| AktError::NotFound(number.to_owned()))
    };
    let (c, m) = (take(container)?, take(member)?);
    let container_type = type_of(conn, tenant, &c).await?;
    require_read(auth, &container_type, &c.number).await?;
    let member_type = type_of(conn, tenant, &m).await?;
    require_read(auth, &member_type, &m.number).await?;
    require(
        auth,
        Permission::Edit {
            key: container_type.ty.key().to_owned(),
        },
    )
    .await?;
    // r[impl container.frozen]
    if is_final(&container_type, &c) {
        return Err(AktError::Frozen(c.number));
    }
    Ok(Pair {
        container: c,
        container_type,
        member: m,
        member_type,
    })
}

/// Records the change on both akter: `added` or `removed` on the member,
/// `added_member` or `removed_member` on the container.
async fn record_both(
    conn: &mut kuria_store::Connection,
    pair: &mut Pair,
    caller: Caller<'_>,
    reason: Option<String>,
    change: &str,
    now: chrono::DateTime<Utc>,
) -> Result<(), AktError> {
    let on_member = json!({ change: [membership_entry(&pair.container)] });
    let on_container = json!({ format!("{change}_member"): [membership_entry(&pair.member)] });
    record_membership(
        conn,
        &pair.member_type,
        &mut pair.member,
        caller.actor,
        reason.clone(),
        on_member,
        now,
    )
    .await?;
    record_membership(
        conn,
        &pair.container_type,
        &mut pair.container,
        caller.actor,
        reason,
        on_container,
        now,
    )
    .await
}
