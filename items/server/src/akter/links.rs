//! Linking akter by a kind their akt types declare, and unlinking them.

use chrono::{DateTime, Utc};
use diesel_async::AsyncConnection;
use kuria_config::LinkDef;
use kuria_engine::rules::{links, records};
use kuria_store::akter::AktRow;
use kuria_store::containers;
use kuria_store::links::{self as stored, LinkRow};
use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{
    AktError, Akter, Caller, StoredType, akt_ref, record_membership, require, require_read, type_of,
};
use crate::authorize::Permission;

/// An akt, locked, with its akt type.
struct Locked {
    akt: AktRow,
    ty: StoredType,
}

impl Akter {
    /// Links `source` to `target` by `kind`, pinning the target's revision
    /// `pin` if given, and returns the source.
    // r[impl link.add]
    // r[impl link.pin]
    // r[impl link.revisions]
    pub async fn add_link(
        &self,
        caller: Caller<'_>,
        source: &str,
        kind: &str,
        target: &str,
        pin: Option<i32>,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                add_link_in(conn, caller, source, kind, target, pin, reason.clone()).await
            })
            .await;
        self.committed(result)
    }

    /// Ends the link of `kind` from `source` to `target`, and returns the
    /// source.
    // r[impl link.remove]
    // r[impl link.revisions]
    pub async fn remove_link(
        &self,
        caller: Caller<'_>,
        source: &str,
        kind: &str,
        target: &str,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                remove_link_in(conn, caller, source, kind, target, reason.clone()).await
            })
            .await;
        self.committed(result)
    }
}

/// The body of [`Akter::add_link`], in the transaction `conn` is in.
pub(super) async fn add_link_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    source: &str,
    kind: &str,
    target: &str,
    pin: Option<i32>,
    reason: Option<String>,
) -> Result<AktRow, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let (mut s, mut t) = lock(conn, caller, tenant, source, target).await?;
    let def = links::to_add(&akt_ref(&s.ty, &s.akt), kind, &akt_ref(&t.ty, &t.akt), pin)?.clone();
    let now = Utc::now();
    let new = stored::insert(
        conn,
        &LinkRow {
            tenant_id: tenant,
            source_id: s.akt.id,
            kind: kind.to_owned(),
            target_id: t.akt.id,
            pinned_revision: pin,
            added_at: now,
            added_by: caller.actor.subject.into(),
        },
    )
    .await?;
    links::check_new(new, source, kind, target)?;
    record(
        conn,
        caller,
        &mut s,
        reason.as_deref(),
        json!({ "linked": [entry(&t.akt, &def, &def.name, pin)] }),
        now,
    )
    .await?;
    record(
        conn,
        caller,
        &mut t,
        reason.as_deref(),
        json!({ "linked_from": [entry(&s.akt, &def, &def.inverse, pin)] }),
        now,
    )
    .await?;
    Ok(s.akt)
}

/// The body of [`Akter::remove_link`], in the transaction `conn` is in.
pub(super) async fn remove_link_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    source: &str,
    kind: &str,
    target: &str,
    reason: Option<String>,
) -> Result<AktRow, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let (mut s, mut t) = lock(conn, caller, tenant, source, target).await?;
    let def = links::to_remove(&akt_ref(&s.ty, &s.akt), kind)?;
    let existed = stored::delete(conn, tenant, s.akt.id, kind, t.akt.id).await?;
    links::check_existed(existed, source, kind, target)?;
    let now = Utc::now();
    record(
        conn,
        caller,
        &mut s,
        reason.as_deref(),
        json!({ "unlinked": [entry(&t.akt, &def, &def.name, None)] }),
        now,
    )
    .await?;
    record(
        conn,
        caller,
        &mut t,
        reason.as_deref(),
        json!({ "unlinked_from": [entry(&s.akt, &def, &def.inverse, None)] }),
        now,
    )
    .await?;
    Ok(s.akt)
}

/// Locks the two akter, refusing a missing or unreadable one as not found,
/// a link to itself, and a caller who may not edit the source.
// r[impl authz.link]
async fn lock(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    tenant: Uuid,
    source: &str,
    target: &str,
) -> Result<(Locked, Locked), AktError> {
    links::check_not_itself(source, target)?;
    let mut rows = containers::lock_akter(conn, tenant, &[source, target]).await?;
    let mut take = async |number: &str| -> Result<Locked, AktError> {
        let akt = rows
            .iter()
            .position(|row| row.number == number)
            .map(|i| rows.swap_remove(i))
            .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
        let ty = type_of(conn, tenant, &akt).await?;
        require_read(caller.auth, &ty, number).await?;
        super::archive::writable(&akt)?;
        Ok(Locked { akt, ty })
    };
    let s = take(source).await?;
    let t = take(target).await?;
    require(
        caller.auth,
        Permission::Edit {
            key: s.ty.ty.key().to_owned(),
        },
    )
    .await?;
    Ok((s, t))
}

/// One side of a link, as a link revision records it.
fn entry(other: &AktRow, def: &LinkDef, name: &str, pin: Option<i32>) -> Json {
    records::link_entry(&other.id.to_string(), &other.number, def, name, pin)
}

async fn record(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    akt: &mut Locked,
    reason: Option<&str>,
    change: Json,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    record_membership(
        conn,
        &akt.ty,
        &mut akt.akt,
        caller.actor,
        reason.map(str::to_owned),
        change,
        now,
    )
    .await
}
