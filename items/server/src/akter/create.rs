//! Creating akter, on their own or inside containers.

use chrono::Utc;
use diesel_async::AsyncConnection;
use kuria_engine::{Change, fields_to_json};
use kuria_store::akter::{self as store, AktRow};
use kuria_store::containers::{self, MembershipRow};
use serde_json::{Map, Value as Json, json};
use uuid::Uuid;

use super::{
    AktError, Akter, Caller, StoredType, membership_entry, next_number, rank_after_last,
    record_membership, require, require_read, revision, type_of,
};
use crate::authorize::Permission;

impl Akter {
    /// Creates an akt of the akt type `key`, as a member of the containers
    /// named in `inside`, the first of which is its parent.
    // r[impl akt.create]
    // r[impl akt.revision.atomic]
    // r[impl authz.create]
    // r[impl container.create-inside]
    // r[impl authz.create-inside]
    pub async fn create(
        &self,
        caller: Caller<'_>,
        key: &str,
        fields: &Map<String, Json>,
        reason: Option<String>,
        inside: &[String],
    ) -> Result<AktRow, AktError> {
        let Caller { actor, auth } = caller;
        let tenant: Uuid = actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                let row = store::latest_type_version(conn, tenant, key)
                    .await?
                    .ok_or_else(|| AktError::UnknownType(key.to_owned()))?;
                if !auth.sees_type(key).await? {
                    return Err(AktError::UnknownType(key.to_owned()));
                }
                require(
                    auth,
                    Permission::Create {
                        key: key.to_owned(),
                    },
                )
                .await?;
                let stored = StoredType::read(row)?;

                // The order lock first, then the containers' rows, as every
                // write path takes them.
                containers::lock_order(conn, tenant).await?;
                let mut parents = lock_containers(conn, tenant, inside).await?;
                let mut parent_types = Vec::with_capacity(parents.len());
                for parent in &parents {
                    let ty = type_of(conn, tenant, parent).await?;
                    require_read(auth, &ty, &parent.number).await?;
                    require(
                        auth,
                        Permission::Edit {
                            key: ty.ty.key().to_owned(),
                        },
                    )
                    .await?;
                    parent_types.push(ty);
                }
                check_created_in(&stored, &parent_types)?;
                check_containers(&stored, &parents, &parent_types)?;

                let now = Utc::now();
                let outcome = kuria_engine::create(
                    &stored.ty,
                    fields,
                    Change {
                        by: actor.subject.into(),
                        at: now,
                    },
                )?;
                let parent = parents.first().map(|p| p.number.as_str());
                let number = next_number(conn, tenant, &stored.ty, parent, now).await?;
                let akt = AktRow {
                    id: Uuid::now_v7(),
                    tenant_id: tenant,
                    number,
                    type_version_id: stored.row.id,
                    state: outcome.state.clone(),
                    fields: Json::Object(fields_to_json(&outcome.fields)),
                    revision: 1,
                    created_at: now,
                    updated_at: now,
                    rank: rank_after_last(conn, tenant).await?,
                };
                store::insert_akt(conn, &akt).await?;
                let mut first = revision(&akt, actor, None, reason.clone(), &outcome, now);
                if !parents.is_empty() {
                    first.membership = Some(json!({
                        "added": parents.iter().map(membership_entry).collect::<Vec<_>>(),
                    }));
                }
                store::append_revision(conn, &first).await?;

                for (parent, ty) in parents.iter_mut().zip(&parent_types) {
                    containers::insert_membership(
                        conn,
                        &MembershipRow {
                            tenant_id: tenant,
                            container_id: parent.id,
                            member_id: akt.id,
                            added_at: now,
                            added_by: actor.subject.into(),
                        },
                    )
                    .await?;
                    record_membership(
                        conn,
                        ty,
                        parent,
                        actor,
                        reason.clone(),
                        json!({ "added_member": [membership_entry(&akt)] }),
                        now,
                    )
                    .await?;
                }
                Ok(akt)
            })
            .await;
        self.committed(result)
    }
}

/// The containers `numbers` names, locked and in the order named.
async fn lock_containers(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    numbers: &[String],
) -> Result<Vec<AktRow>, AktError> {
    let mut seen = std::collections::BTreeSet::new();
    if let Some(twice) = numbers.iter().find(|n| !seen.insert(n.as_str())) {
        return Err(AktError::AlreadyMember {
            container: twice.clone(),
            member: "the new akt".to_owned(),
        });
    }
    let refs: Vec<&str> = numbers.iter().map(String::as_str).collect();
    let mut locked = containers::lock_akter(conn, tenant, &refs).await?;
    numbers
        .iter()
        .map(|number| {
            locked
                .iter()
                .position(|row| &row.number == number)
                .map(|i| locked.swap_remove(i))
                .ok_or_else(|| AktError::NotFound(number.clone()))
        })
        .collect()
}

/// Refuses an akt type with `created_in` created anywhere else.
// r[impl container.created-in]
fn check_created_in(ty: &StoredType, parents: &[StoredType]) -> Result<(), AktError> {
    let allowed = ty.ty.created_in();
    if allowed.is_empty() {
        return Ok(());
    }
    match parents.first() {
        Some(parent) if allowed.iter().any(|a| a == parent.ty.key()) => Ok(()),
        _ => Err(AktError::CreatedIn {
            akt_type: ty.ty.key().to_owned(),
            allowed: allowed.to_vec(),
        }),
    }
}

/// Refuses containers that can't take an akt of `ty`: of a type that doesn't
/// contain it, frozen, or two of one exclusive type.
fn check_containers(
    ty: &StoredType,
    parents: &[AktRow],
    parent_types: &[StoredType],
) -> Result<(), AktError> {
    let mut exclusive: Vec<(&str, &str)> = Vec::new();
    for (parent, parent_type) in parents.iter().zip(parent_types) {
        // r[impl container.contains]
        let Some(contains) = parent_type.ty.contains(ty.ty.key()) else {
            return Err(AktError::NotContainable {
                container: parent.number.clone(),
                member: ty.ty.key().to_owned(),
            });
        };
        // r[impl container.frozen]
        if super::is_final(parent_type, parent) {
            return Err(AktError::Frozen(parent.number.clone()));
        }
        // r[impl container.while]
        if !contains.accepts_in(&parent.state) {
            return Err(AktError::NotAccepting {
                container: parent.number.clone(),
                member_type: ty.ty.key().to_owned(),
                state: parent.state.clone(),
            });
        }
        // r[impl container.exclusive]
        if contains.exclusive {
            let key = parent_type.ty.key();
            if let Some((_, other)) = exclusive.iter().find(|(k, _)| *k == key) {
                return Err(AktError::Exclusive {
                    member: "the new akt".to_owned(),
                    other: (*other).to_owned(),
                    akt_type: key.to_owned(),
                });
            }
            exclusive.push((key, &parent.number));
        }
    }
    Ok(())
}
