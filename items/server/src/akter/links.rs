//! Linking akter by a kind their akt types declare, and unlinking them.

use chrono::{DateTime, Utc};
use diesel_async::AsyncConnection;
use kuria_config::LinkDef;
use kuria_store::akter::AktRow;
use kuria_store::containers;
use kuria_store::links::{self, LinkRow};
use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{
    AktError, Akter, Caller, StoredType, record_membership, require, require_read, type_of,
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
        let tenant: Uuid = caller.actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                let (mut s, mut t) = lock(conn, caller, tenant, source, target).await?;
                let def = kind_of(&s, kind)?.clone();
                if !def.to.iter().any(|key| key == t.ty.ty.key()) {
                    return Err(AktError::InvalidLink(format!(
                        "a `{kind}` link of `{}` goes to {}, not to `{}`",
                        s.ty.ty.key(),
                        def.to
                            .iter()
                            .map(|k| format!("`{k}`"))
                            .collect::<Vec<_>>()
                            .join(", "),
                        t.ty.ty.key()
                    )));
                }
                if let Some(pin) = pin
                    && !(1..=t.akt.revision).contains(&pin)
                {
                    return Err(AktError::InvalidLink(format!(
                        "`{target}` has no revision {pin}"
                    )));
                }
                let now = Utc::now();
                let new = links::insert(
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
                if !new {
                    return Err(AktError::InvalidLink(format!(
                        "`{source}` already has a `{kind}` link to `{target}`"
                    )));
                }
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
        let tenant: Uuid = caller.actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                let (mut s, mut t) = lock(conn, caller, tenant, source, target).await?;
                // A kind a newer version dropped can still be unlinked.
                let def = s.ty.ty.link(kind).cloned().unwrap_or_else(|| LinkDef {
                    key: kind.to_owned(),
                    name: kind.to_owned(),
                    inverse: kind.to_owned(),
                    to: Vec::new(),
                });
                if !links::delete(conn, tenant, s.akt.id, kind, t.akt.id).await? {
                    return Err(AktError::NotLinked {
                        source_number: source.to_owned(),
                        kind: kind.to_owned(),
                        target: target.to_owned(),
                    });
                }
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
            })
            .await;
        self.committed(result)
    }
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
    if source == target {
        return Err(AktError::InvalidLink(format!(
            "`{source}` can't link to itself"
        )));
    }
    let mut rows = containers::lock_akter(conn, tenant, &[source, target]).await?;
    let mut take = async |number: &str| -> Result<Locked, AktError> {
        let akt = rows
            .iter()
            .position(|row| row.number == number)
            .map(|i| rows.swap_remove(i))
            .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
        let ty = type_of(conn, tenant, &akt).await?;
        require_read(caller.auth, &ty, number).await?;
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

/// The kind `kind` the source's akt type version declares.
fn kind_of<'a>(s: &'a Locked, kind: &str) -> Result<&'a LinkDef, AktError> {
    s.ty.ty.link(kind).ok_or_else(|| {
        AktError::InvalidLink(format!("`{}` has no kind of link `{kind}`", s.ty.ty.key()))
    })
}

/// One side of a link, as a link revision records it: the other akt, the
/// kind, and its name as this akt reads it.
fn entry(other: &AktRow, def: &LinkDef, name: &str, pin: Option<i32>) -> Json {
    let mut entry = json!({
        "akt": other.id.to_string(),
        "number": other.number,
        "kind": def.key,
        "name": name,
    });
    if let Some(pin) = pin {
        entry["revision"] = json!(pin);
    }
    entry
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
