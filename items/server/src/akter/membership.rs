//! Adding members to containers, removing them, and transferring them from
//! one container to another.

use chrono::{DateTime, Utc};
use diesel_async::AsyncConnection;
use kuria_store::akter::AktRow;
use kuria_store::containers::{self, MembershipRow};
use serde_json::{Value as Json, json};
use uuid::Uuid;

use kuria_engine::rules::containment;

use super::{
    AktError, Akter, Caller, StoredType, akt_ref, membership_entry, record_membership, require,
    require_read, type_of,
};
use crate::authorize::Permission;

/// An akt, locked, with its akt type.
struct Locked {
    akt: AktRow,
    ty: StoredType,
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
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                add_member_in(conn, caller, container, member, reason.clone()).await
            })
            .await;
        self.committed(result)
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
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                remove_member_in(conn, caller, container, member, reason.clone()).await
            })
            .await;
        self.committed(result)
    }

    /// Transfers the akt `member` from the container `from` to the container
    /// `to`, as one change, and returns `to`.
    // r[impl container.transfer]
    // r[impl authz.transfer]
    pub async fn transfer_member(
        &self,
        caller: Caller<'_>,
        member: &str,
        from: &str,
        to: &str,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                transfer_member_in(conn, caller, member, from, to, reason.clone()).await
            })
            .await;
        self.committed(result)
    }
}

/// The body of [`Akter::add_member`], in the transaction `conn` is in.
pub(super) async fn add_member_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    container: &str,
    member: &str,
    reason: Option<String>,
) -> Result<AktRow, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let [mut c, mut m] = lock(conn, caller, tenant, [container, member]).await?;
    require_edit(caller, &c).await?;
    check_unfrozen(&c)?;
    check_accepts(conn, tenant, &c, &m, None).await?;
    let now = Utc::now();
    insert(conn, caller, tenant, &c, &m, now).await?;
    record(
        conn,
        caller,
        &mut m,
        reason.as_deref(),
        json!({ "added": [membership_entry(&c.akt)] }),
        now,
    )
    .await?;
    record(
        conn,
        caller,
        &mut c,
        reason.as_deref(),
        json!({ "added_member": [membership_entry(&m.akt)] }),
        now,
    )
    .await?;
    Ok(c.akt)
}

/// The body of [`Akter::remove_member`], in the transaction `conn` is in.
pub(super) async fn remove_member_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    container: &str,
    member: &str,
    reason: Option<String>,
) -> Result<AktRow, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let [mut c, mut m] = lock(conn, caller, tenant, [container, member]).await?;
    require_edit(caller, &c).await?;
    check_unfrozen(&c)?;
    end(conn, tenant, &c, &m).await?;
    let now = Utc::now();
    record(
        conn,
        caller,
        &mut m,
        reason.as_deref(),
        json!({ "removed": [membership_entry(&c.akt)] }),
        now,
    )
    .await?;
    let left = m.akt.number.clone();
    record_left(
        conn,
        caller,
        &mut c,
        &left,
        reason.as_deref(),
        json!({ "removed_member": [membership_entry(&m.akt)] }),
        now,
    )
    .await?;
    Ok(c.akt)
}

/// The body of [`Akter::transfer_member`], in the transaction `conn` is in.
pub(super) async fn transfer_member_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    member: &str,
    from: &str,
    to: &str,
    reason: Option<String>,
) -> Result<AktRow, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    if from == to {
        return Err(AktError::AlreadyMember {
            container: to.to_owned(),
            member: member.to_owned(),
        });
    }
    let [mut source, mut target, mut m] = lock(conn, caller, tenant, [from, to, member]).await?;
    require_edit(caller, &source).await?;
    require_edit(caller, &target).await?;
    check_unfrozen(&source)?;
    check_unfrozen(&target)?;
    end(conn, tenant, &source, &m).await?;
    // The membership just ended doesn't count against exclusivity.
    check_accepts(conn, tenant, &target, &m, Some(source.akt.id)).await?;
    let now = Utc::now();
    insert(conn, caller, tenant, &target, &m, now).await?;
    record(
        conn,
        caller,
        &mut m,
        reason.as_deref(),
        json!({
            "removed": [membership_entry(&source.akt)],
            "added": [membership_entry(&target.akt)],
        }),
        now,
    )
    .await?;
    let left = m.akt.number.clone();
    record_left(
        conn,
        caller,
        &mut source,
        &left,
        reason.as_deref(),
        json!({ "removed_member": [membership_entry(&m.akt)] }),
        now,
    )
    .await?;
    record(
        conn,
        caller,
        &mut target,
        reason.as_deref(),
        json!({ "added_member": [membership_entry(&m.akt)] }),
        now,
    )
    .await?;
    Ok(target.akt)
}

/// Locks the akter `numbers`, refusing a missing or unreadable one as not
/// found, and returns them in the order named. The last is the member;
/// naming one akt twice is a cycle.
// r[impl authz.membership]
async fn lock<const N: usize>(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    tenant: Uuid,
    numbers: [&str; N],
) -> Result<[Locked; N], AktError> {
    containment::check_not_itself(&numbers[..N - 1], numbers[N - 1])?;
    let mut rows = containers::lock_akter(conn, tenant, &numbers).await?;
    let mut locked = Vec::with_capacity(N);
    for number in numbers {
        let akt = rows
            .iter()
            .position(|row| row.number == number)
            .map(|i| rows.swap_remove(i))
            .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
        let ty = type_of(conn, tenant, &akt).await?;
        require_read(caller.auth, &ty, number).await?;
        locked.push(Locked { akt, ty });
    }
    Ok(locked
        .try_into()
        .unwrap_or_else(|_| unreachable!("one per number")))
}

/// Refuses a caller who may not edit the container `c`.
async fn require_edit(caller: Caller<'_>, c: &Locked) -> Result<(), AktError> {
    require(
        caller.auth,
        Permission::Edit {
            key: c.ty.ty.key().to_owned(),
        },
    )
    .await
}

/// Refuses a container in a final state.
fn check_unfrozen(c: &Locked) -> Result<(), AktError> {
    Ok(containment::check_unfrozen(&akt_ref(&c.ty, &c.akt))?)
}

/// Refuses what would stop `c` from gaining `m` as a member (not counting
/// the membership `ending`), from what the store knows of the two.
async fn check_accepts(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    c: &Locked,
    m: &Locked,
    ending: Option<Uuid>,
) -> Result<(), AktError> {
    let other = containers::containers_of(conn, tenant, m.akt.id)
        .await?
        .into_iter()
        .find(|(row, key)| key == c.ty.ty.key() && row.id != c.akt.id && Some(row.id) != ending)
        .map(|(row, _)| row.number);
    let known = containment::Joining {
        already: containers::is_member(conn, tenant, c.akt.id, m.akt.id).await?,
        other_of_type: other.as_deref(),
        contains_container: containers::contains(conn, tenant, m.akt.id, c.akt.id).await?,
    };
    Ok(containment::check_joining(
        &akt_ref(&c.ty, &c.akt),
        &akt_ref(&m.ty, &m.akt),
        known,
    )?)
}

async fn insert(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    tenant: Uuid,
    c: &Locked,
    m: &Locked,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    containers::insert_membership(
        conn,
        &MembershipRow {
            tenant_id: tenant,
            container_id: c.akt.id,
            member_id: m.akt.id,
            added_at: now,
            added_by: caller.actor.subject.into(),
        },
    )
    .await?;
    Ok(())
}

/// Ends `m`'s membership in `c`, refusing if there is none.
async fn end(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    c: &Locked,
    m: &Locked,
) -> Result<(), AktError> {
    if containers::delete_membership(conn, tenant, c.akt.id, m.akt.id).await? {
        Ok(())
    } else {
        Err(AktError::NotMember {
            container: c.akt.number.clone(),
            member: m.akt.number.clone(),
        })
    }
}

async fn record(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    akt: &mut Locked,
    reason: Option<&str>,
    membership: Json,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    record_membership(
        conn,
        &akt.ty,
        &mut akt.akt,
        caller.actor,
        reason.map(str::to_owned),
        membership,
        now,
    )
    .await
}

/// [`record`] of a container `member` left, taking it out of the
/// container's sections in the same revision (`akt.field.sections`).
async fn record_left(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    container: &mut Locked,
    member: &str,
    reason: Option<&str>,
    membership: Json,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    // r[impl live.revisions]
    // A container with a live draft loses the member from its draft's
    // sections; its revision keeps its latest revision's fields.
    if super::live::has_live_draft(&container.ty) {
        super::live::leave_sections(
            conn,
            &container.ty,
            &container.akt,
            member,
            caller.actor.subject.into(),
            now,
        )
        .await?;
        return super::record_membership(
            conn,
            &container.ty,
            &mut container.akt,
            caller.actor,
            reason.map(str::to_owned),
            membership,
            now,
        )
        .await;
    }
    let changes = super::leave_sections(&container.ty, &mut container.akt, member)?;
    super::record_membership_changing(
        conn,
        &container.ty,
        &mut container.akt,
        caller.actor,
        reason.map(str::to_owned),
        membership,
        changes,
        now,
    )
    .await
}
