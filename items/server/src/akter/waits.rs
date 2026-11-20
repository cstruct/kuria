//! What a transition waits for (`akt.transition.members`,
//! `akt.transition.links`): the engine decides from the members and links
//! found here.

use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap};

use kuria_config::{AktType, TransitionDef};
use kuria_engine::waits::{self, Linked, Member, Waits};
use kuria_store::akter::{self as store, AktRow};
use kuria_store::{containers, links};
use uuid::Uuid;

use super::{AktError, StoredType};

/// Whether `akt` is in a final state of its own akt type version, the
/// versions read so far kept in `types`.
async fn is_final(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    types: &mut HashMap<Uuid, StoredType>,
    akt: &AktRow,
) -> Result<bool, AktError> {
    let ty = match types.entry(akt.type_version_id) {
        Entry::Occupied(known) => known.into_mut(),
        Entry::Vacant(new) => {
            let row = store::type_versions_by_id(conn, tenant, &[akt.type_version_id])
                .await?
                .pop()
                .ok_or_else(|| AktError::Internal(format!("akt {} has no akt type", akt.number)))?;
            new.insert(StoredType::read(row)?)
        }
    };
    Ok(ty.ty.state(&akt.state).is_some_and(|s| s.is_final))
}

/// The members and links of `akt` that `def` may wait for, as the engine
/// sees them, and the ids of the akter they name.
async fn found(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    def: &TransitionDef,
    akt: &AktRow,
) -> Result<(Vec<Member>, Vec<Linked>, BTreeSet<Uuid>), AktError> {
    let mut types = HashMap::new();
    let mut ids = BTreeSet::new();
    let mut members = Vec::new();
    if !def.waits_for.members.is_empty() {
        for (member, key) in containers::members_with_types(conn, tenant, akt.id).await? {
            if def.waits_for.members.contains(&key) {
                ids.insert(member.id);
                members.push(Member {
                    number: member.number.clone(),
                    is_final: is_final(conn, tenant, &mut types, &member).await?,
                    akt_type: key,
                });
            }
        }
    }
    let mut linked = Vec::new();
    if !def.waits_for.links.is_empty() || !def.requires_links.is_empty() {
        for (link, target) in links::from(conn, tenant, akt.id).await? {
            if def.waits_for.links.contains(&link.kind) || def.requires_links.contains(&link.kind) {
                ids.insert(target.id);
                linked.push(Linked {
                    is_final: is_final(conn, tenant, &mut types, &target).await?,
                    number: target.number,
                    kind: link.kind,
                });
            }
        }
    }
    Ok((members, linked, ids))
}

/// What `def` waits for, for `akt` as it is now; nothing if it waits for
/// nothing.
// r[impl akt.transition.waits]
pub async fn of(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    def: &TransitionDef,
    akt: &AktRow,
) -> Result<Waits, AktError> {
    if !waits::waits_at_all(def) {
        return Ok(Waits::default());
    }
    let (members, linked, _) = found(conn, tenant, def, akt).await?;
    Ok(waits::of(def, &members, &linked))
}

/// What the transition `transition` of `ty` waits for, for `akt`, which
/// the caller holds locked, so that its members and links stay as they
/// are. The members and targets it names are share-locked, in id order, so
/// that none of them changes state before the transaction ends.
// r[impl akt.transition.members]
// r[impl akt.transition.links]
pub async fn locked(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    ty: &AktType,
    transition: &str,
    akt: &AktRow,
) -> Result<Waits, AktError> {
    let Some(def) = ty.transition(transition) else {
        return Ok(Waits::default());
    };
    if !waits::waits_at_all(def) {
        return Ok(Waits::default());
    }
    let (_, _, ids) = found(conn, tenant, def, akt).await?;
    let ids: Vec<Uuid> = ids.into_iter().collect();
    containers::share_akter(conn, tenant, &ids).await?;
    // Read again, now that nothing it names can move.
    let (members, linked, _) = found(conn, tenant, def, akt).await?;
    Ok(waits::of(def, &members, &linked))
}
