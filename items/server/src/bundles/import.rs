//! Importing a bundle into the caller's tenant, all or nothing
//! (`bundle.import.*`).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::Utc;
use diesel_async::AsyncConnection as _;
use kuria_config::{AktType, AktTypeDocument, validate};
use kuria_engine::{Value, fields_from_json, rank_between, state_hash};
use kuria_store::akter::{AktRow, AktTypeVersion, RevisionRow};
use kuria_store::attachments::AttachmentRow;
use kuria_store::bundles::{self as store, HeldAttachment, ImportRecord, TenantPerson};
use kuria_store::contacts::Contact;
use kuria_store::containers::MembershipRow;
use kuria_store::links::LinkRow;
use kuria_store::people::Person;
use kuria_store::versions::VersionRow;
use kuria_store::{Connection, Pool};
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::BundleError;
use super::archive::{self, Unpacked};
use super::export::unhex;
use super::format::{Bundle, READS, Scope};
use crate::akter::{define_in, read_document};
use crate::attachments::{Storage, hex};

/// What an import wrote.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub scope: Scope,
    pub akt_types: usize,
    pub akter: usize,
    /// The bundle's subjects shown as the tenant's users.
    pub aliases: usize,
}

/// Imports the bundle in `archive` into `tenant`, as `importer`.
pub async fn import(
    pool: &Pool,
    storage: &Storage,
    tenant: Uuid,
    importer: Uuid,
    archive: &[u8],
) -> Result<Imported, BundleError> {
    let unpacked = archive::unpack(archive)?;
    let bundle = read_bundle(&unpacked.bundle)?;
    let mapping = read_mapping(unpacked.subjects.as_deref())?;
    let types = check(&bundle, &unpacked)?;
    // Content-addressed, so storing bytes an import then refuses harms
    // nothing; doing it first keeps the transaction short.
    for attachment in &bundle.attachments {
        let sha256 = unhex(&attachment.sha256).unwrap_or_default();
        let bytes = unpacked.attachments[&attachment.sha256.to_ascii_lowercase()].clone();
        storage.put(tenant, &sha256, bytes).await.map_err(|e| {
            BundleError::Unavailable(format!("storing attachment {}: {e}", attachment.id))
        })?;
    }
    let mut conn = pool.get().await?;
    let conn: &mut Connection = &mut conn;
    // r[impl bundle.import.atomic]
    conn.transaction::<_, BundleError, _>(async |conn| {
        write(conn, tenant, importer, &bundle, &types, &mapping).await
    })
    .await
}

/// `bundle.json`, refusing a format this server doesn't read.
// r[impl bundle.import.format]
fn read_bundle(bytes: &[u8]) -> Result<Bundle, BundleError> {
    let invalid = |e: serde_json::Error| BundleError::Invalid(format!("`bundle.json`: {e}"));
    let raw: Json = serde_json::from_slice(bytes).map_err(invalid)?;
    match raw["format"].as_u64() {
        Some(format) if READS.iter().any(|r| u64::from(*r) == format) => {}
        Some(format) => {
            return Err(BundleError::Invalid(format!(
                "the bundle's format is {format}; this Kuria reads formats {}",
                READS.map(|r| r.to_string()).join(" and ")
            )));
        }
        None => {
            return Err(BundleError::Invalid(
                "`bundle.json` names no format".to_owned(),
            ));
        }
    }
    serde_json::from_value(raw).map_err(invalid)
}

/// `subjects.json`: the bundle's subjects to the tenant's.
fn read_mapping(bytes: Option<&[u8]>) -> Result<HashMap<Uuid, Uuid>, BundleError> {
    let Some(bytes) = bytes else {
        return Ok(HashMap::new());
    };
    serde_json::from_slice(bytes).map_err(|e| {
        BundleError::Invalid(format!(
            "`subjects.json` maps subject identifiers to subject identifiers: {e}"
        ))
    })
}

/// Checks everything the bundle says before anything is written, and
/// answers its akt type versions, by id.
// r[impl bundle.import.format]
// r[impl bundle.import.hashes]
fn check(bundle: &Bundle, unpacked: &Unpacked) -> Result<HashMap<Uuid, AktType>, BundleError> {
    let invalid = |message: String| Err(BundleError::Invalid(message));
    let keys: BTreeSet<&str> = bundle.akt_types.iter().map(|t| t.key.as_str()).collect();
    match bundle.scope {
        Scope::Types => {
            let selected: BTreeSet<&str> = bundle.types.iter().map(String::as_str).collect();
            if selected.is_empty() || selected != keys {
                return invalid(
                    "a `types` bundle holds exactly the akt types it selected".to_owned(),
                );
            }
        }
        Scope::Tenant if !bundle.holds_akter => {
            return invalid("a `tenant` bundle holds the tenant's akter".to_owned());
        }
        Scope::Tenant => {}
    }
    if !bundle.holds_akter {
        let only_documents = bundle.akter.is_empty()
            && bundle.contacts.is_empty()
            && bundle.attachments.is_empty()
            && bundle.akt_types.iter().all(|t| t.versions.len() == 1);
        if !only_documents {
            return invalid(
                "a bundle without akter holds one document per akt type, and nothing else"
                    .to_owned(),
            );
        }
    }

    let types = check_types(bundle)?;
    let mut ids = HashSet::new();
    let mut numbers = HashSet::new();
    let attachments: HashSet<Uuid> = bundle.attachments.iter().map(|a| a.id).collect();
    for akt in &bundle.akter {
        if !ids.insert(akt.id) || !numbers.insert(akt.number.as_str()) {
            return invalid(format!("the akt {} is there twice", akt.number));
        }
        let Some(ty) = types.get(&akt.type_version) else {
            return invalid(format!(
                "{} names an akt type version the bundle lacks",
                akt.number
            ));
        };
        replay(akt, ty)?;
        check_versions(akt)?;
        check_held(akt, &attachments)?;
    }
    if let Some(akt) = bundle
        .akter
        .iter()
        .find(|a| a.copied_from.is_some_and(|o| !ids.contains(&o)))
    {
        return invalid(format!(
            "{} was copied from an akt the bundle lacks",
            akt.number
        ));
    }
    for m in &bundle.memberships {
        if !ids.contains(&m.container) || !ids.contains(&m.member) {
            return invalid("a membership names an akt the bundle lacks".to_owned());
        }
    }
    for l in &bundle.links {
        if !ids.contains(&l.source) || !ids.contains(&l.target) {
            return invalid("a link names an akt the bundle lacks".to_owned());
        }
    }
    let mut emails = HashSet::new();
    let mut contacts = HashSet::new();
    for c in &bundle.contacts {
        if !contacts.insert(c.id)
            || c.email
                .as_ref()
                .is_some_and(|e| !emails.insert(e.to_lowercase()))
        {
            return invalid(format!("the contact {} is there twice", c.id));
        }
    }
    for a in &bundle.attachments {
        let bytes = unpacked.attachments.get(&a.sha256.to_ascii_lowercase());
        let matches = bytes.is_some_and(|b| {
            hex(&Sha256::digest(b)) == a.sha256.to_ascii_lowercase()
                && i64::try_from(b.len()).ok() == Some(a.size)
        });
        if !matches {
            return invalid(format!(
                "the bytes of attachment {} are missing or aren't its",
                a.id
            ));
        }
    }
    Ok(types)
}

/// The bundle's akt type versions, each valid and numbered as it says.
fn check_types(bundle: &Bundle) -> Result<HashMap<Uuid, AktType>, BundleError> {
    let invalid = |message: String| Err(BundleError::Invalid(message));
    let mut types = HashMap::new();
    for t in &bundle.akt_types {
        for (i, v) in t.versions.iter().enumerate() {
            if bundle.holds_akter && usize::try_from(v.version).ok() != Some(i + 1) {
                return invalid(format!(
                    "`{}` holds its versions from 1 on, in order",
                    t.key
                ));
            }
            let document: AktTypeDocument =
                serde_json::from_value(v.definition.clone()).map_err(|e| {
                    BundleError::Invalid(format!("`{}` version {}: {e}", t.key, v.version))
                })?;
            let ty = validate(document).map_err(|problems| {
                BundleError::Invalid(format!(
                    "`{}` version {} isn't valid: {}",
                    t.key,
                    v.version,
                    problems
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ))
            })?;
            if ty.key() != t.key {
                return invalid(format!("a version of `{}` defines `{}`", t.key, ty.key()));
            }
            if types.insert(v.id, ty).is_some() {
                return invalid(format!("the akt type version {} is there twice", v.id));
            }
        }
    }

    Ok(types)
}

/// Archives the akter `bundle` holds archived, as it records them, and
/// marks those it holds kept, once they and their history are written:
/// each is written live and moved, as archiving moves it
/// (`archive.storage`). A `with_container` archive names its container
/// by the identifier it got.
// r[impl bundle.import.archived]
async fn archive_akter(
    conn: &mut Connection,
    tenant: Uuid,
    bundle: &Bundle,
    akt_ids: &HashMap<Uuid, Uuid>,
) -> Result<(), BundleError> {
    for a in &bundle.akter {
        let id = akt_ids[&a.id];
        if let Some(archived) = &a.archived {
            let mut by = archived.by.clone();
            if let Some(container) = by["container"]
                .as_str()
                .and_then(|c| c.parse::<Uuid>().ok())
                .and_then(|c| akt_ids.get(&c))
            {
                by["container"] = json!(container);
            }
            let archive = kuria_store::akter::Archive {
                at: archived.at,
                by,
                reason: archived.reason.clone(),
            };
            kuria_store::akter::set_archived(conn, tenant, id, Some(&archive), false).await?;
        } else if a.archive_kept {
            kuria_store::akter::set_archived(conn, tenant, id, None, true).await?;
        }
    }
    Ok(())
}

/// Replays `akt`'s revisions, checking each one's hash against the state
/// and field values it results in, and that they end where the akt is.
// r[impl bundle.import.hashes]
fn replay(akt: &super::format::BundleAkt, ty: &AktType) -> Result<(), BundleError> {
    let invalid = |message: String| BundleError::Invalid(format!("{}: {message}", akt.number));
    let mut fields: BTreeMap<String, Value> = BTreeMap::new();
    for (i, r) in akt.revisions.iter().enumerate() {
        if usize::try_from(r.revision).ok() != Some(i + 1) {
            return Err(invalid(
                "its revisions aren't numbered from 1 on, in order".to_owned(),
            ));
        }
        for (key, raw) in r.changes.as_object().into_iter().flatten() {
            if raw.is_null() {
                fields.remove(key);
                continue;
            }
            let field = ty.field(key).ok_or_else(|| {
                invalid(format!(
                    "revision {} sets `{key}`, which its type lacks",
                    r.revision
                ))
            })?;
            let value = Value::from_json(field, raw)
                .map_err(|e| invalid(format!("revision {} sets `{key}`: {e}", r.revision)))?;
            fields.insert(key.clone(), value);
        }
        if hex(&state_hash(&r.state, &fields)) != r.hash.to_ascii_lowercase() {
            return Err(invalid(format!(
                "revision {}'s hash doesn't match its state and fields",
                r.revision
            )));
        }
    }
    let last = akt
        .revisions
        .last()
        .ok_or_else(|| invalid("it has no revisions".to_owned()))?;
    let now = fields_from_json(ty, akt.fields.as_object().unwrap_or(&Map::new()))
        .map_err(|e| invalid(format!("its fields: {e}")))?;
    if last.state != akt.state || now != fields {
        return Err(invalid("its revisions don't end where it is".to_owned()));
    }
    Ok(())
}

/// Every attachment `akt` holds, in a field or as a diagram, is one the
/// bundle holds.
fn check_held(
    akt: &super::format::BundleAkt,
    attachments: &HashSet<Uuid>,
) -> Result<(), BundleError> {
    match akt
        .attachments
        .iter()
        .map(|h| h.attachment)
        .chain(akt.diagrams.iter().map(|d| d.attachment))
        .find(|a| !attachments.contains(a))
    {
        Some(held) => Err(BundleError::Invalid(format!(
            "{} holds the attachment {held}, which the bundle lacks",
            akt.number
        ))),
        None => Ok(()),
    }
}

/// `akt`'s versions: numbered from 1 on, in order, each naming a revision
/// of its own that the akt has, with its pins as `version.pins` keeps them.
// r[impl bundle.import.history]
fn check_versions(akt: &super::format::BundleAkt) -> Result<(), BundleError> {
    let invalid = |message: String| Err(BundleError::Invalid(format!("{}: {message}", akt.number)));
    let last = akt.revisions.last().map_or(0, |r| r.revision);
    let mut named = HashSet::new();
    for (i, v) in akt.versions.iter().enumerate() {
        if usize::try_from(v.number).ok() != Some(i + 1) {
            return invalid("its versions aren't numbered from 1 on, in order".to_owned());
        }
        if !(1..=last).contains(&v.revision) || !named.insert(v.revision) {
            return invalid(format!(
                "version {} names revision {}, which it lacks or another version names",
                v.number, v.revision
            ));
        }
        if serde_json::from_value::<Vec<kuria_engine::versions::Pin>>(v.pins.clone()).is_err() {
            return invalid(format!("version {}'s pins don't read", v.number));
        }
    }
    Ok(())
}

/// Who each subject the bundle names is in the tenant: by the mapping,
/// else by email to a user who signed in to the tenant.
// r[impl bundle.import.subjects]
async fn subjects(
    conn: &mut Connection,
    tenant: Uuid,
    bundle: &Bundle,
    given: &HashMap<Uuid, Uuid>,
) -> Result<HashMap<Uuid, Uuid>, BundleError> {
    let by_email: HashMap<String, Uuid> = kuria_store::people::in_tenant(conn, tenant)
        .await?
        .into_iter()
        .filter_map(|(subject, person)| Some((person?.email?.to_lowercase(), subject)))
        .collect();
    let mut mapped: HashMap<Uuid, Uuid> = given.clone();
    for person in &bundle.people {
        if mapped.contains_key(&person.subject) {
            continue;
        }
        if let Some(user) = person
            .email
            .as_ref()
            .and_then(|e| by_email.get(&e.to_lowercase()))
        {
            mapped.insert(person.subject, *user);
        }
    }
    mapped.retain(|from, to| from != to);
    Ok(mapped)
}

/// Every attachment each akt held, now or at any revision, recorded as
/// held for good (`attachment.download`).
async fn keep_held(
    conn: &mut Connection,
    tenant: Uuid,
    bundle: &Bundle,
    types: &HashMap<Uuid, AktType>,
    akt_ids: &HashMap<Uuid, Uuid>,
) -> Result<(), BundleError> {
    for a in &bundle.akter {
        let ty = &types[&a.type_version];
        let mut ids: Vec<Uuid> = a.attachments.iter().map(|h| h.attachment).collect();
        for r in &a.revisions {
            for (key, raw) in r.changes.as_object().into_iter().flatten() {
                if let Some(Value::Attachment(id)) = ty
                    .field(key)
                    .filter(|_| !raw.is_null())
                    .and_then(|f| Value::from_json(f, raw).ok())
                    && !ids.contains(&id)
                {
                    ids.push(id);
                }
            }
        }
        kuria_store::attachments::keep_held(conn, tenant, akt_ids[&a.id], &ids).await?;
        let diagrams: Vec<kuria_store::attachments::DiagramRow> = a
            .diagrams
            .iter()
            .map(|d| kuria_store::attachments::DiagramRow {
                tenant_id: tenant,
                akt_id: akt_ids[&a.id],
                field: d.field.clone(),
                source_sha256: unhex(&d.source_sha256).unwrap_or_default(),
                attachment_id: d.attachment,
            })
            .collect();
        kuria_store::attachments::insert_diagrams(conn, &diagrams).await?;
    }
    Ok(())
}

/// The bundle's versions and commits, with the akter's new identifiers
/// (`bundle.import.history`).
async fn insert_versions(
    conn: &mut Connection,
    tenant: Uuid,
    bundle: &Bundle,
    akt_ids: &HashMap<Uuid, Uuid>,
) -> Result<(), BundleError> {
    let mut versions = Vec::new();
    let mut commits = Vec::new();
    for a in &bundle.akter {
        let id = akt_ids[&a.id];
        versions.extend(a.versions.iter().map(|v| VersionRow {
            tenant_id: tenant,
            akt_id: id,
            number: v.number,
            revision: v.revision,
            label: v.label.clone(),
            released_by: v.released_by,
            released_at: v.released_at,
            pins: v.pins.clone(),
        }));
        commits.extend(
            a.revisions
                .iter()
                .filter_map(|r| Some((tenant, id, r.revision, r.commit.clone()?))),
        );
    }
    kuria_store::versions::insert_imported(conn, &versions, &commits).await?;
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "one transaction's writes, in the order the rows depend on each other"
)]
async fn write(
    conn: &mut Connection,
    tenant: Uuid,
    importer: Uuid,
    bundle: &Bundle,
    types: &HashMap<Uuid, AktType>,
    given: &HashMap<Uuid, Uuid>,
) -> Result<Imported, BundleError> {
    let now = Utc::now();
    let mapping = subjects(conn, tenant, bundle, given).await?;
    for (from, to) in &mapping {
        store::alias(conn, tenant, *from, *to, now).await?;
    }

    // r[impl bundle.import.clashes]
    if !bundle.holds_akter {
        for t in &bundle.akt_types {
            let (ty, definition) = read_document(t.versions[0].definition.clone())?;
            define_in(conn, tenant, importer, ty, &definition).await?;
        }
        store::record_import(
            conn,
            &ImportRecord {
                id: Uuid::now_v7(),
                tenant_id: tenant,
                imported_by: importer,
                imported_at: now,
                scope: bundle.scope.as_str().to_owned(),
                akt_types: i32::try_from(bundle.akt_types.len()).unwrap_or(i32::MAX),
                akter: 0,
            },
            &[],
        )
        .await?;
        return Ok(Imported {
            scope: bundle.scope,
            akt_types: bundle.akt_types.len(),
            akter: 0,
            aliases: mapping.len(),
        });
    }
    // r[impl bundle.import.tenant]
    if bundle.scope == Scope::Tenant && !store::is_empty(conn, tenant).await? {
        return Err(BundleError::Conflict(
            "the tenant already holds akt types, akter or contacts; a whole tenant goes only into an empty one".to_owned(),
        ));
    }
    let keys: Vec<String> = bundle.akt_types.iter().map(|t| t.key.clone()).collect();
    let numbers: Vec<String> = bundle.akter.iter().map(|a| a.number.clone()).collect();
    let emails: Vec<String> = bundle
        .contacts
        .iter()
        .filter_map(|c| c.email.clone())
        .collect();
    let contact_ids: Vec<Uuid> = bundle.contacts.iter().map(|c| c.id).collect();
    let attachment_ids: Vec<Uuid> = bundle.attachments.iter().map(|a| a.id).collect();
    let clashes = store::clashes(
        conn,
        tenant,
        &keys,
        &numbers,
        &emails,
        &contact_ids,
        &attachment_ids,
    )
    .await?;
    if !clashes.is_empty() {
        let mut said = Vec::new();
        let list = |what: &str, items: Vec<String>, said: &mut Vec<String>| {
            if !items.is_empty() {
                said.push(format!("{what} {}", items.join(", ")));
            }
        };
        list("the akt types", clashes.keys, &mut said);
        list("the akter", clashes.numbers, &mut said);
        list("contacts with the emails", clashes.emails, &mut said);
        list(
            "the contacts",
            clashes.contacts.iter().map(Uuid::to_string).collect(),
            &mut said,
        );
        list(
            "the attachments",
            clashes.attachments.iter().map(Uuid::to_string).collect(),
            &mut said,
        );
        return Err(BundleError::Conflict(format!(
            "Kuria already holds {}",
            said.join("; ")
        )));
    }

    // r[impl bundle.import.ids]
    let version_ids: HashMap<Uuid, Uuid> = types.keys().map(|old| (*old, Uuid::now_v7())).collect();
    for t in &bundle.akt_types {
        let rows: Vec<AktTypeVersion> = t
            .versions
            .iter()
            .map(|v| AktTypeVersion {
                id: version_ids[&v.id],
                tenant_id: tenant,
                key: t.key.clone(),
                version: v.version,
                definition: v.definition.clone(),
                created_by: v.created_by,
                created_at: v.created_at,
            })
            .collect();
        store::insert_type(conn, &rows).await?;
    }
    store::insert_contacts_and_attachments(
        conn,
        &bundle
            .contacts
            .iter()
            .map(|c| Contact {
                id: c.id,
                tenant_id: tenant,
                name: c.name.clone(),
                email: c.email.clone(),
                linked_subject: c
                    .linked_subject
                    .map(|s| mapping.get(&s).copied().unwrap_or(s)),
                linked_at: c.linked_at,
                created_by: c.created_by,
                created_at: c.created_at,
                linked_by: c.linked_by,
            })
            .collect::<Vec<_>>(),
        &bundle
            .attachments
            .iter()
            .map(|a| AttachmentRow {
                id: a.id,
                tenant_id: tenant,
                name: a.name.clone(),
                media_type: a.media_type.clone(),
                size: a.size,
                sha256: unhex(&a.sha256).unwrap_or_default(),
                uploaded_by: a.uploaded_by,
                uploaded_at: a.uploaded_at,
                sbom: a.sbom.clone(),
            })
            .collect::<Vec<_>>(),
    )
    .await?;

    let akt_ids: HashMap<Uuid, Uuid> = bundle
        .akter
        .iter()
        .map(|a| (a.id, Uuid::now_v7()))
        .collect();
    // r[impl bundle.import.order]
    let mut previous = match bundle.scope {
        Scope::Types => kuria_store::containers::last_rank(conn, tenant).await?,
        Scope::Tenant => None,
    };
    let mut akter = Vec::new();
    let mut revisions = Vec::new();
    let mut held = Vec::new();
    for a in &bundle.akter {
        let rank = match bundle.scope {
            Scope::Tenant => a.rank.clone(),
            Scope::Types => {
                let rank = rank_between(previous.as_deref(), None)
                    .map_err(|e| BundleError::Internal(e.to_string()))?;
                previous = Some(rank.clone());
                rank
            }
        };
        let id = akt_ids[&a.id];
        // r[impl bundle.import.history]
        akter.push(AktRow {
            id,
            tenant_id: tenant,
            number: a.number.clone(),
            type_version_id: version_ids[&a.type_version],
            state: a.state.clone(),
            fields: a.fields.clone(),
            revision: a.revisions.last().map_or(1, |r| r.revision),
            created_at: a.created_at,
            updated_at: a.updated_at,
            rank,
            title: a.title.clone(),
            state_since: a
                .revisions
                .iter()
                .rev()
                .find(|r| r.transition.is_some())
                .map_or(a.created_at, |r| r.created_at),
            archived: false,
            archived_at: None,
            archived_by: None,
            archive_reason: None,
            archive_kept: false,
        });
        revisions.extend(a.revisions.iter().map(|r| RevisionRow {
            akt_id: id,
            revision: r.revision,
            tenant_id: tenant,
            author: r.author,
            transition: r.transition.clone(),
            reason: r.reason.clone(),
            changes: r.changes.clone(),
            state: r.state.clone(),
            hash: unhex(&r.hash).unwrap_or_default(),
            created_at: r.created_at,
            membership: r.membership.clone(),
        }));
        held.extend(a.attachments.iter().map(|h| HeldAttachment {
            tenant_id: tenant,
            akt_id: id,
            field: h.field.clone(),
            attachment_id: h.attachment,
        }));
    }
    let memberships: Vec<MembershipRow> = bundle
        .memberships
        .iter()
        .map(|m| MembershipRow {
            tenant_id: tenant,
            container_id: akt_ids[&m.container],
            member_id: akt_ids[&m.member],
            added_at: m.added_at,
            added_by: m.added_by,
        })
        .collect();
    let links: Vec<LinkRow> = bundle
        .links
        .iter()
        .map(|l| LinkRow {
            tenant_id: tenant,
            source_id: akt_ids[&l.source],
            kind: l.kind.clone(),
            target_id: akt_ids[&l.target],
            pinned_revision: l.pinned_revision,
            added_at: l.added_at,
            added_by: l.added_by,
        })
        .collect();
    store::insert_akter(conn, &akter, &revisions, &memberships, &links, &held).await?;
    insert_versions(conn, tenant, bundle, &akt_ids).await?;
    keep_held(conn, tenant, bundle, types, &akt_ids).await?;
    for a in &bundle.akter {
        if let Some(origin) = a.copied_from.and_then(|o| akt_ids.get(&o)) {
            kuria_store::akter::insert_origin(conn, tenant, akt_ids[&a.id], *origin).await?;
        }
    }

    // r[impl bundle.import.subjects]
    let people: Vec<Person> = bundle
        .people
        .iter()
        .map(|p| Person {
            subject_id: p.subject,
            name: p.name.clone(),
            email: p.email.clone(),
            updated_at: now,
        })
        .collect();
    store::insert_people(conn, &people).await?;
    let signed_in: Vec<TenantPerson> = bundle
        .signed_in
        .iter()
        .map(|p| TenantPerson {
            tenant_id: tenant,
            subject_id: p.subject,
            first_seen_at: p.first_seen_at,
            last_seen_at: p.last_seen_at,
        })
        .collect();
    store::insert_tenant_people(conn, &signed_in).await?;

    // r[impl bundle.import.counters]
    let mut counters: BTreeMap<String, i64> = bundle
        .counters
        .iter()
        .filter(|c| c.name.starts_with("number:"))
        .map(|c| (c.name.clone(), c.value))
        .collect();
    for a in &bundle.akter {
        if let Some((scope, sequence)) = types[&a.type_version].number().read(&a.number) {
            let value = i64::try_from(sequence).unwrap_or(i64::MAX);
            let counter = counters.entry(format!("number:{scope}")).or_default();
            *counter = (*counter).max(value);
        }
    }
    for (name, value) in &counters {
        store::raise_counter(conn, tenant, name, *value).await?;
    }

    // r[impl bundle.import.recorded]
    let written: Vec<(Uuid, i32)> = akter.iter().map(|a| (a.id, a.revision)).collect();
    store::record_import(
        conn,
        &ImportRecord {
            id: Uuid::now_v7(),
            tenant_id: tenant,
            imported_by: importer,
            imported_at: now,
            scope: bundle.scope.as_str().to_owned(),
            akt_types: i32::try_from(bundle.akt_types.len()).unwrap_or(i32::MAX),
            akter: i32::try_from(akter.len()).unwrap_or(i32::MAX),
        },
        &written,
    )
    .await?;
    // Moved once everything naming them, change events too, is written.
    archive_akter(conn, tenant, bundle, &akt_ids).await?;
    Ok(Imported {
        scope: bundle.scope,
        akt_types: bundle.akt_types.len(),
        akter: akter.len(),
        aliases: mapping.len(),
    })
}
