//! Exporting a bundle: one consistent snapshot of the tenant
//! (`bundle.export.*`).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::Utc;
use kuria_config::AktType;
use kuria_engine::{Someone, Value};
use kuria_store::akter::AktTypeVersion;
use kuria_store::bundles as store;
use kuria_store::{Connection, Pool};
use serde_json::{Map, Value as Json};
use uuid::Uuid;

use super::BundleError;
use super::archive;
use super::format::{
    Bundle, BundleAkt, BundleAttachment, BundleContact, BundleCounter, BundleHeld, BundleLink,
    BundleMembership, BundlePerson, BundleRevision, BundleSignedIn, BundleType, BundleVersion,
    FORMAT, Scope,
};
use crate::akter::StoredType;
use crate::attachments::{Storage, hex};

/// What to export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// These akt types, and their akter if `akter`.
    Types { keys: Vec<String>, akter: bool },
    /// Everything Kuria holds for the tenant.
    Tenant,
}

/// The tenant's bundle for `selection`, as a tar archive.
// r[impl bundle.export.snapshot]
pub async fn export(
    pool: &Pool,
    storage: &Storage,
    tenant: Uuid,
    selection: &Selection,
) -> Result<Vec<u8>, BundleError> {
    let mut conn = pool.get().await?;
    let bundle = conn
        .build_transaction()
        .repeatable_read()
        .read_only()
        .run::<_, BundleError, _>(async |conn| gather(conn, tenant, selection).await)
        .await?;
    // Attachments never change, so their bytes are read after the snapshot.
    let mut files = Vec::new();
    for attachment in &bundle.attachments {
        let sha256 = unhex(&attachment.sha256).ok_or_else(|| {
            BundleError::Internal(format!("attachment {} has no hash", attachment.id))
        })?;
        let bytes = storage.get(tenant, &sha256).await.map_err(|e| {
            BundleError::Unavailable(format!("reading attachment {}: {e}", attachment.id))
        })?;
        files.push((attachment.sha256.clone(), bytes));
    }
    let json =
        serde_json::to_vec_pretty(&bundle).map_err(|e| BundleError::Internal(e.to_string()))?;
    archive::pack(&json, &files)
}

/// Bytes from hexadecimal.
pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    text.len()
        .is_multiple_of(2)
        .then(|| {
            (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
                .collect()
        })
        .flatten()
}

fn versions_of(rows: &[AktTypeVersion], latest_only: bool) -> Vec<BundleType> {
    let mut by_key: BTreeMap<&str, Vec<BundleVersion>> = BTreeMap::new();
    for row in rows {
        by_key.entry(&row.key).or_default().push(BundleVersion {
            id: row.id,
            version: row.version,
            definition: row.definition.clone(),
            created_by: row.created_by,
            created_at: row.created_at,
        });
    }
    by_key
        .into_iter()
        .map(|(key, mut versions)| {
            versions.sort_by_key(|v| v.version);
            if latest_only {
                versions.drain(..versions.len() - 1);
            }
            BundleType {
                key: key.to_owned(),
                versions,
            }
        })
        .collect()
}

/// Who and what values name: users, contacts and attachments.
#[derive(Default)]
struct Named {
    subjects: BTreeSet<Uuid>,
    contacts: BTreeSet<Uuid>,
    attachments: BTreeSet<Uuid>,
}

impl Named {
    fn value(&mut self, value: &Value) {
        for someone in value.named() {
            match someone {
                Someone::User(subject) => self.subjects.insert(subject),
                Someone::Contact(id) => self.contacts.insert(id),
            };
        }
        if let Value::Attachment(id) = value {
            self.attachments.insert(*id);
        }
    }

    /// The values of `fields`, read as `ty`'s; a key `ty` lacks or a value
    /// that doesn't read names no one.
    fn fields(&mut self, ty: &AktType, fields: &Map<String, Json>) {
        for (key, raw) in fields {
            if let Some(value) = ty
                .field(key)
                .filter(|_| !raw.is_null())
                .and_then(|field| Value::from_json(field, raw).ok())
            {
                self.value(&value);
            }
        }
    }
}

// r[impl bundle.contents]
// r[impl bundle.export.types]
// r[impl bundle.export.tenant]
async fn gather(
    conn: &mut Connection,
    tenant: Uuid,
    selection: &Selection,
) -> Result<Bundle, BundleError> {
    let (keys, with_akter, scope) = match selection {
        Selection::Types { keys, akter } => (Some(keys.as_slice()), *akter, Scope::Types),
        Selection::Tenant => (None, true, Scope::Tenant),
    };
    let versions = store::type_versions(conn, tenant, keys).await?;
    for key in keys.unwrap_or_default() {
        if !versions.iter().any(|v| v.key == *key) {
            return Err(BundleError::UnknownType(key.clone()));
        }
    }
    let mut bundle = Bundle {
        format: FORMAT,
        scope,
        types: keys.map(<[String]>::to_vec).unwrap_or_default(),
        holds_akter: with_akter,
        exported_at: Utc::now(),
        akt_types: versions_of(&versions, !with_akter),
        akter: Vec::new(),
        memberships: Vec::new(),
        links: Vec::new(),
        counters: Vec::new(),
        contacts: Vec::new(),
        people: Vec::new(),
        signed_in: Vec::new(),
        attachments: Vec::new(),
    };
    if !with_akter {
        return Ok(bundle);
    }

    let named = gather_akter(conn, tenant, &versions, &mut bundle).await?;
    gather_around(conn, tenant, named, &mut bundle).await?;
    Ok(bundle)
}

/// The origins of the copies among `ids` that the bundle holds too; one
/// outside it can't be named (`akt.copy.record`).
async fn origins_within(
    conn: &mut Connection,
    tenant: Uuid,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, Uuid>, BundleError> {
    let exported: HashSet<Uuid> = ids.iter().copied().collect();
    Ok(kuria_store::akter::origins_of(conn, tenant, ids)
        .await?
        .into_iter()
        .filter(|(_, origin)| exported.contains(origin))
        .collect())
}

/// The akter of `versions`, with their revisions, memberships, links and
/// attachments held, refusing a selection that leaves out an akt they name;
/// answers who and what they name.
async fn gather_akter(
    conn: &mut Connection,
    tenant: Uuid,
    versions: &[AktTypeVersion],
    bundle: &mut Bundle,
) -> Result<Named, BundleError> {
    let types: HashMap<Uuid, AktType> = versions
        .iter()
        .map(|row| StoredType::read(row.clone()).map(|t| (row.id, t.ty)))
        .collect::<Result<_, _>>()?;
    let version_ids: Vec<Uuid> = versions.iter().map(|v| v.id).collect();
    let akter = store::akter_of(conn, tenant, &version_ids).await?;
    let ids: Vec<Uuid> = akter.iter().map(|a| a.id).collect();
    let inside: HashSet<Uuid> = ids.iter().copied().collect();
    let memberships = store::memberships_of(conn, tenant, &ids).await?;
    let links = store::links_of(conn, tenant, &ids).await?;
    let outside: Vec<Uuid> = memberships
        .iter()
        .flat_map(|m| [m.container_id, m.member_id])
        .chain(links.iter().flat_map(|l| [l.source_id, l.target_id]))
        .filter(|id| !inside.contains(id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !outside.is_empty() {
        let missing: BTreeSet<String> = store::keys_of(conn, &outside)
            .await?
            .into_iter()
            .map(|(_, k)| k)
            .collect();
        return Err(BundleError::IncompleteSelection(
            missing.into_iter().collect(),
        ));
    }

    let mut named = Named::default();
    named.subjects.extend(versions.iter().map(|v| v.created_by));
    let mut revisions: HashMap<Uuid, Vec<BundleRevision>> = HashMap::new();
    for r in store::revisions_of(conn, &ids).await? {
        named.subjects.insert(r.author);
        revisions.entry(r.akt_id).or_default().push(BundleRevision {
            revision: r.revision,
            author: r.author,
            transition: r.transition,
            reason: r.reason,
            changes: r.changes,
            state: r.state,
            hash: hex(&r.hash),
            created_at: r.created_at,
            membership: r.membership,
        });
    }
    let mut held: HashMap<Uuid, Vec<BundleHeld>> = HashMap::new();
    for h in store::held_by(conn, &ids).await? {
        named.attachments.insert(h.attachment_id);
        held.entry(h.akt_id).or_default().push(BundleHeld {
            field: h.field,
            attachment: h.attachment_id,
        });
    }
    let origins = origins_within(conn, tenant, &ids).await?;
    for akt in akter {
        let ty = &types[&akt.type_version_id];
        let revisions = revisions.remove(&akt.id).unwrap_or_default();
        named.fields(ty, akt.fields.as_object().unwrap_or(&Map::new()));
        for r in &revisions {
            named.fields(ty, r.changes.as_object().unwrap_or(&Map::new()));
        }
        bundle.akter.push(BundleAkt {
            id: akt.id,
            number: akt.number,
            type_version: akt.type_version_id,
            state: akt.state,
            fields: akt.fields,
            title: akt.title,
            rank: akt.rank,
            created_at: akt.created_at,
            updated_at: akt.updated_at,
            revisions,
            attachments: held.remove(&akt.id).unwrap_or_default(),
            copied_from: origins.get(&akt.id).copied(),
        });
    }
    for m in memberships {
        named.subjects.insert(m.added_by);
        bundle.memberships.push(BundleMembership {
            container: m.container_id,
            member: m.member_id,
            added_at: m.added_at,
            added_by: m.added_by,
        });
    }
    for l in links {
        named.subjects.insert(l.added_by);
        bundle.links.push(BundleLink {
            source: l.source_id,
            kind: l.kind,
            target: l.target_id,
            pinned_revision: l.pinned_revision,
            added_at: l.added_at,
            added_by: l.added_by,
        });
    }

    Ok(named)
}

/// The contacts, attachments and people `named` names, or for a whole
/// tenant all of them, with who signed in to it and its counters.
async fn gather_around(
    conn: &mut Connection,
    tenant: Uuid,
    mut named: Named,
    bundle: &mut Bundle,
) -> Result<(), BundleError> {
    let scope = bundle.scope;
    let whole = scope == Scope::Tenant;
    let contact_ids: Vec<Uuid> = named.contacts.iter().copied().collect();
    for c in store::contacts(conn, tenant, (!whole).then_some(contact_ids.as_slice())).await? {
        named.subjects.extend(
            [Some(c.created_by), c.linked_by, c.linked_subject]
                .into_iter()
                .flatten(),
        );
        bundle.contacts.push(BundleContact {
            id: c.id,
            name: c.name,
            email: c.email,
            linked_subject: c.linked_subject,
            linked_at: c.linked_at,
            linked_by: c.linked_by,
            created_by: c.created_by,
            created_at: c.created_at,
        });
    }
    let attachment_ids: Vec<Uuid> = named.attachments.iter().copied().collect();
    for a in store::attachments(conn, tenant, (!whole).then_some(attachment_ids.as_slice())).await?
    {
        named.subjects.insert(a.uploaded_by);
        bundle.attachments.push(BundleAttachment {
            id: a.id,
            name: a.name,
            media_type: a.media_type,
            size: a.size,
            sha256: hex(&a.sha256),
            uploaded_by: a.uploaded_by,
            uploaded_at: a.uploaded_at,
            sbom: a.sbom,
        });
    }
    if whole {
        for p in store::tenant_people(conn, tenant).await? {
            named.subjects.insert(p.subject_id);
            bundle.signed_in.push(BundleSignedIn {
                subject: p.subject_id,
                first_seen_at: p.first_seen_at,
                last_seen_at: p.last_seen_at,
            });
        }
        bundle.counters = store::counters(conn, tenant)
            .await?
            .into_iter()
            .filter(|(name, _)| name.starts_with("number:"))
            .map(|(name, value)| BundleCounter { name, value })
            .collect();
    }
    let subjects: Vec<Uuid> = named.subjects.into_iter().collect();
    let mut people = kuria_store::people::find(conn, &subjects).await?;
    people.sort_by_key(|p| p.subject_id);
    bundle.people = people
        .into_iter()
        .map(|p| BundlePerson {
            subject: p.subject_id,
            name: p.name,
            email: p.email,
        })
        .collect();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hexadecimal() {
        assert_eq!(unhex("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(unhex("0"), None);
        assert_eq!(unhex("zz"), None);
    }
}
