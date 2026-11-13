//! What exports read and imports write (`bundle.*`): a tenant's rows in
//! bulk, the checks an import makes before writing, and subject aliases.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::akter::{AktRow, AktTypeVersion, RevisionRow};
use crate::attachments::AttachmentRow;
use crate::contacts::Contact;
use crate::containers::MembershipRow;
use crate::links::LinkRow;
use crate::people::Person;
use crate::schema::{
    akt, akt_attachment, akt_link, akt_membership, akt_revision, akt_type, akt_type_version,
    attachment, bundle_import, change_event, contact, person, subject_alias, tenant_counter,
    tenant_person,
};

/// That a subject signed in to a tenant, as stored.
#[derive(Clone, Debug, PartialEq, Eq, Insertable, Queryable, Selectable)]
#[diesel(table_name = tenant_person)]
pub struct TenantPerson {
    pub tenant_id: Uuid,
    pub subject_id: Uuid,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

/// That an akt's field holds an attachment, as stored.
#[derive(Clone, Debug, PartialEq, Eq, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_attachment)]
pub struct HeldAttachment {
    pub tenant_id: Uuid,
    pub akt_id: Uuid,
    pub field: String,
    pub attachment_id: Uuid,
}

/// A record of an import.
#[derive(Clone, Debug, Insertable)]
#[diesel(table_name = bundle_import)]
pub struct ImportRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub imported_by: Uuid,
    pub imported_at: DateTime<Utc>,
    pub scope: String,
    pub akt_types: i32,
    pub akter: i32,
}

/// The versions of `tenant`'s akt types `keys`, or of all of them, oldest
/// first per key.
pub async fn type_versions(
    conn: &mut Connection,
    tenant: Uuid,
    keys: Option<&[String]>,
) -> QueryResult<Vec<AktTypeVersion>> {
    let mut query = akt_type_version::table
        .filter(akt_type_version::tenant_id.eq(tenant))
        .order((akt_type_version::key, akt_type_version::version))
        .select(AktTypeVersion::as_select())
        .into_boxed();
    if let Some(keys) = keys {
        query = query.filter(akt_type_version::key.eq_any(keys));
    }
    query.load(conn).await
}

/// The akter of the akt type versions `versions`, in the tenant's order.
pub async fn akter_of(
    conn: &mut Connection,
    tenant: Uuid,
    versions: &[Uuid],
) -> QueryResult<Vec<AktRow>> {
    akt::table
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt::type_version_id.eq_any(versions))
        .order(akt::rank)
        .select(AktRow::as_select())
        .load(conn)
        .await
}

/// Every revision of `akter`, oldest first per akt.
pub async fn revisions_of(conn: &mut Connection, akter: &[Uuid]) -> QueryResult<Vec<RevisionRow>> {
    akt_revision::table
        .filter(akt_revision::akt_id.eq_any(akter))
        .order((akt_revision::akt_id, akt_revision::revision))
        .select(RevisionRow::as_select())
        .load(conn)
        .await
}

/// The memberships either side of which is one of `akter`.
pub async fn memberships_of(
    conn: &mut Connection,
    tenant: Uuid,
    akter: &[Uuid],
) -> QueryResult<Vec<MembershipRow>> {
    akt_membership::table
        .filter(akt_membership::tenant_id.eq(tenant))
        .filter(
            akt_membership::container_id
                .eq_any(akter)
                .or(akt_membership::member_id.eq_any(akter)),
        )
        .select(MembershipRow::as_select())
        .load(conn)
        .await
}

/// The links either end of which is one of `akter`.
pub async fn links_of(
    conn: &mut Connection,
    tenant: Uuid,
    akter: &[Uuid],
) -> QueryResult<Vec<LinkRow>> {
    akt_link::table
        .filter(akt_link::tenant_id.eq(tenant))
        .filter(
            akt_link::source_id
                .eq_any(akter)
                .or(akt_link::target_id.eq_any(akter)),
        )
        .select(LinkRow::as_select())
        .load(conn)
        .await
}

/// The akt type keys of the akter `ids`, by akt.
pub async fn keys_of(conn: &mut Connection, ids: &[Uuid]) -> QueryResult<Vec<(Uuid, String)>> {
    akt::table
        .inner_join(akt_type_version::table.on(akt_type_version::id.eq(akt::type_version_id)))
        .filter(akt::id.eq_any(ids))
        .select((akt::id, akt_type_version::key))
        .load(conn)
        .await
}

/// The tenant's counters, by name.
pub async fn counters(conn: &mut Connection, tenant: Uuid) -> QueryResult<Vec<(String, i64)>> {
    tenant_counter::table
        .filter(tenant_counter::tenant_id.eq(tenant))
        .order(tenant_counter::name)
        .select((tenant_counter::name, tenant_counter::value))
        .load(conn)
        .await
}

/// The tenant's contacts `ids`, or all of them.
pub async fn contacts(
    conn: &mut Connection,
    tenant: Uuid,
    ids: Option<&[Uuid]>,
) -> QueryResult<Vec<Contact>> {
    let mut query = contact::table
        .filter(contact::tenant_id.eq(tenant))
        .order(contact::created_at)
        .select(Contact::as_select())
        .into_boxed();
    if let Some(ids) = ids {
        query = query.filter(contact::id.eq_any(ids));
    }
    query.load(conn).await
}

/// Who signed in to the tenant.
pub async fn tenant_people(conn: &mut Connection, tenant: Uuid) -> QueryResult<Vec<TenantPerson>> {
    tenant_person::table
        .filter(tenant_person::tenant_id.eq(tenant))
        .order(tenant_person::first_seen_at)
        .select(TenantPerson::as_select())
        .load(conn)
        .await
}

/// The tenant's attachments `ids`, or all of them.
pub async fn attachments(
    conn: &mut Connection,
    tenant: Uuid,
    ids: Option<&[Uuid]>,
) -> QueryResult<Vec<AttachmentRow>> {
    let mut query = attachment::table
        .filter(attachment::tenant_id.eq(tenant))
        .order(attachment::uploaded_at)
        .select(AttachmentRow::as_select())
        .into_boxed();
    if let Some(ids) = ids {
        query = query.filter(attachment::id.eq_any(ids));
    }
    query.load(conn).await
}

/// Which attachments `akter`'s fields hold.
pub async fn held_by(conn: &mut Connection, akter: &[Uuid]) -> QueryResult<Vec<HeldAttachment>> {
    akt_attachment::table
        .filter(akt_attachment::akt_id.eq_any(akter))
        .select(HeldAttachment::as_select())
        .load(conn)
        .await
}

/// What the tenant already has of what an import would write: akt type
/// keys, akt numbers and contact emails in it, and contacts and
/// attachments anywhere in this Kuria.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Clashes {
    pub keys: Vec<String>,
    pub numbers: Vec<String>,
    pub emails: Vec<String>,
    pub contacts: Vec<Uuid>,
    pub attachments: Vec<Uuid>,
}

impl Clashes {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// What of `keys`, `numbers`, `emails` (compared ignoring case), contacts
/// and attachments clashes with what is stored.
pub async fn clashes(
    conn: &mut Connection,
    tenant: Uuid,
    keys: &[String],
    numbers: &[String],
    emails: &[String],
    contacts: &[Uuid],
    attachments: &[Uuid],
) -> QueryResult<Clashes> {
    let lower: Vec<String> = emails.iter().map(|e| e.to_lowercase()).collect();
    Ok(Clashes {
        keys: akt_type::table
            .filter(akt_type::tenant_id.eq(tenant))
            .filter(akt_type::key.eq_any(keys))
            .select(akt_type::key)
            .load(conn)
            .await?,
        numbers: akt::table
            .filter(akt::tenant_id.eq(tenant))
            .filter(akt::number.eq_any(numbers))
            .select(akt::number)
            .load(conn)
            .await?,
        emails: contact::table
            .filter(contact::tenant_id.eq(tenant))
            .filter(
                diesel::dsl::sql::<diesel::sql_types::Bool>("lower(email) = any(")
                    .bind::<diesel::sql_types::Array<diesel::sql_types::Text>, _>(&lower)
                    .sql(")"),
            )
            .select(contact::email.assume_not_null())
            .load(conn)
            .await?,
        contacts: contact::table
            .filter(contact::id.eq_any(contacts))
            .select(contact::id)
            .load(conn)
            .await?,
        attachments: attachment::table
            .filter(attachment::id.eq_any(attachments))
            .select(attachment::id)
            .load(conn)
            .await?,
    })
}

/// Whether the tenant holds no akt type, akt or contact.
pub async fn is_empty(conn: &mut Connection, tenant: Uuid) -> QueryResult<bool> {
    let any = |n: i64| n > 0;
    let types: i64 = akt_type::table
        .filter(akt_type::tenant_id.eq(tenant))
        .count()
        .get_result(conn)
        .await?;
    let akter: i64 = akt::table
        .filter(akt::tenant_id.eq(tenant))
        .count()
        .get_result(conn)
        .await?;
    let contacts: i64 = contact::table
        .filter(contact::tenant_id.eq(tenant))
        .count()
        .get_result(conn)
        .await?;
    Ok(!any(types) && !any(akter) && !any(contacts))
}

/// Stores an akt type with the versions `versions`, as they were.
pub async fn insert_type(conn: &mut Connection, versions: &[AktTypeVersion]) -> QueryResult<()> {
    let Some(latest) = versions.iter().max_by_key(|v| v.version) else {
        return Ok(());
    };
    diesel::insert_into(akt_type::table)
        .values((
            akt_type::tenant_id.eq(latest.tenant_id),
            akt_type::key.eq(&latest.key),
            akt_type::latest_version.eq(latest.version),
        ))
        .execute(conn)
        .await?;
    diesel::insert_into(akt_type_version::table)
        .values(versions)
        .execute(conn)
        .await
        .map(drop)
}

/// Stores akter, their revisions, memberships, links, and the attachments
/// they hold, as they were, in chunks Postgres takes.
pub async fn insert_akter(
    conn: &mut Connection,
    akter: &[AktRow],
    revisions: &[RevisionRow],
    memberships: &[MembershipRow],
    links: &[LinkRow],
    held: &[HeldAttachment],
) -> QueryResult<()> {
    // Postgres takes at most 65535 parameters to a statement.
    for chunk in akter.chunks(1000) {
        diesel::insert_into(akt::table)
            .values(chunk)
            .execute(conn)
            .await?;
    }
    for chunk in revisions.chunks(1000) {
        diesel::insert_into(akt_revision::table)
            .values(chunk)
            .execute(conn)
            .await?;
    }
    for chunk in memberships.chunks(2000) {
        diesel::insert_into(akt_membership::table)
            .values(chunk)
            .execute(conn)
            .await?;
    }
    for chunk in links.chunks(2000) {
        diesel::insert_into(akt_link::table)
            .values(chunk)
            .execute(conn)
            .await?;
    }
    for chunk in held.chunks(4000) {
        diesel::insert_into(akt_attachment::table)
            .values(chunk)
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// Stores contacts and attachments' metadata as they were.
pub async fn insert_contacts_and_attachments(
    conn: &mut Connection,
    contacts: &[Contact],
    attachments: &[AttachmentRow],
) -> QueryResult<()> {
    for chunk in contacts.chunks(1000) {
        diesel::insert_into(contact::table)
            .values(chunk)
            .execute(conn)
            .await?;
    }
    for chunk in attachments.chunks(1000) {
        diesel::insert_into(attachment::table)
            .values(chunk)
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// Records the names and emails of `people` whom Kuria holds nothing for.
pub async fn insert_people(conn: &mut Connection, people: &[Person]) -> QueryResult<()> {
    for chunk in people.chunks(2000) {
        diesel::insert_into(person::table)
            .values(chunk)
            .on_conflict_do_nothing()
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// Records who signed in to the tenant, keeping the earliest and latest
/// times if Kuria already has them.
pub async fn insert_tenant_people(conn: &mut Connection, rows: &[TenantPerson]) -> QueryResult<()> {
    for chunk in rows.chunks(2000) {
        diesel::insert_into(tenant_person::table)
            .values(chunk)
            .on_conflict((tenant_person::tenant_id, tenant_person::subject_id))
            .do_update()
            .set(tenant_person::last_seen_at.eq(diesel::dsl::sql(
                "greatest(tenant_person.last_seen_at, excluded.last_seen_at)",
            )))
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// Moves the tenant's counter `name` to at least `value`.
pub async fn raise_counter(
    conn: &mut Connection,
    tenant: Uuid,
    name: &str,
    value: i64,
) -> QueryResult<()> {
    diesel::insert_into(tenant_counter::table)
        .values((
            tenant_counter::tenant_id.eq(tenant),
            tenant_counter::name.eq(name),
            tenant_counter::value.eq(value),
        ))
        .on_conflict((tenant_counter::tenant_id, tenant_counter::name))
        .do_update()
        .set(tenant_counter::value.eq(diesel::dsl::sql(
            "greatest(tenant_counter.value, excluded.value)",
        )))
        .execute(conn)
        .await
        .map(drop)
}

/// Records that `subject` is shown as, and filters as, `alias_of` in the
/// tenant, replacing an alias it had.
pub async fn alias(
    conn: &mut Connection,
    tenant: Uuid,
    subject: Uuid,
    alias_of: Uuid,
    at: DateTime<Utc>,
) -> QueryResult<()> {
    diesel::insert_into(subject_alias::table)
        .values((
            subject_alias::tenant_id.eq(tenant),
            subject_alias::subject_id.eq(subject),
            subject_alias::alias_of.eq(alias_of),
            subject_alias::created_at.eq(at),
        ))
        .on_conflict((subject_alias::tenant_id, subject_alias::subject_id))
        .do_update()
        .set(subject_alias::alias_of.eq(excluded(subject_alias::alias_of)))
        .execute(conn)
        .await
        .map(drop)
}

/// The subject `subject` is shown as in the tenant: whom it aliases, or
/// itself.
// r[impl bundle.import.subjects]
pub async fn shown_as(conn: &mut Connection, tenant: Uuid, subject: Uuid) -> QueryResult<Uuid> {
    Ok(subject_alias::table
        .find((tenant, subject))
        .select(subject_alias::alias_of)
        .first(conn)
        .await
        .optional()?
        .unwrap_or(subject))
}

/// The subjects that alias `user` in the tenant.
// r[impl bundle.import.subjects]
pub async fn aliases_of(conn: &mut Connection, tenant: Uuid, user: Uuid) -> QueryResult<Vec<Uuid>> {
    subject_alias::table
        .filter(subject_alias::tenant_id.eq(tenant))
        .filter(subject_alias::alias_of.eq(user))
        .select(subject_alias::subject_id)
        .load(conn)
        .await
}

/// Records an import, with change events for the akter it wrote at their
/// latest revisions.
// r[impl bundle.import.recorded]
pub async fn record_import(
    conn: &mut Connection,
    record: &ImportRecord,
    akter: &[(Uuid, i32)],
) -> QueryResult<()> {
    diesel::insert_into(bundle_import::table)
        .values(record)
        .execute(conn)
        .await?;
    for (akt, revision) in akter {
        let seq = crate::akter::next_counter(conn, record.tenant_id, "change_event").await?;
        diesel::insert_into(change_event::table)
            .values((
                change_event::tenant_id.eq(record.tenant_id),
                change_event::seq.eq(seq),
                change_event::akt_id.eq(akt),
                change_event::revision.eq(revision),
                change_event::created_at.eq(record.imported_at),
                change_event::kind.eq("revision"),
            ))
            .execute(conn)
            .await?;
    }
    Ok(())
}
