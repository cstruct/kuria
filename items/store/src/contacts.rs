//! Contacts: people a tenant names before, or without, them being users.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::schema::contact;

/// A contact, as stored.
#[derive(Clone, Debug, PartialEq, Eq, Insertable, Queryable, Selectable)]
#[diesel(table_name = contact)]
pub struct Contact {
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `None` once erased.
    pub name: Option<String>,
    pub email: Option<String>,
    pub linked_subject: Option<Uuid>,
    pub linked_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Why a contact couldn't be written.
#[derive(Debug, thiserror::Error)]
pub enum ContactError {
    #[error("another contact of the tenant has that email")]
    DuplicateEmail,
    #[error(transparent)]
    Database(#[from] Error),
}

fn written(result: QueryResult<usize>) -> Result<usize, ContactError> {
    result.map_err(|e| match e {
        Error::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => ContactError::DuplicateEmail,
        other => ContactError::Database(other),
    })
}

/// Adds `contact`.
pub async fn insert(conn: &mut Connection, contact: &Contact) -> Result<(), ContactError> {
    written(
        diesel::insert_into(contact::table)
            .values(contact)
            .execute(conn)
            .await,
    )
    .map(drop)
}

/// The tenant's contacts `ids`, in no particular order.
pub async fn find(conn: &mut Connection, tenant: Uuid, ids: &[Uuid]) -> QueryResult<Vec<Contact>> {
    contact::table
        .filter(contact::tenant_id.eq(tenant))
        .filter(contact::id.eq_any(ids))
        .select(Contact::as_select())
        .load(conn)
        .await
}

/// Every contact of the tenant, by name, then id.
pub async fn list(conn: &mut Connection, tenant: Uuid) -> QueryResult<Vec<Contact>> {
    contact::table
        .filter(contact::tenant_id.eq(tenant))
        .order((contact::name.asc().nulls_last(), contact::id.asc()))
        .select(Contact::as_select())
        .load(conn)
        .await
}

/// Changes an unlinked contact's name and email. Answers whether it did:
/// not if there is no such contact, or it is linked.
pub async fn edit_unlinked(
    conn: &mut Connection,
    tenant: Uuid,
    id: Uuid,
    name: &str,
    email: Option<&str>,
) -> Result<bool, ContactError> {
    let changed = written(
        diesel::update(
            contact::table
                .filter(contact::tenant_id.eq(tenant))
                .filter(contact::id.eq(id))
                .filter(contact::linked_subject.is_null()),
        )
        .set((contact::name.eq(name), contact::email.eq(email)))
        .execute(conn)
        .await,
    )?;
    Ok(changed == 1)
}

/// Deletes a contact's name and email. Answers whether there was one.
pub async fn erase(conn: &mut Connection, tenant: Uuid, id: Uuid) -> QueryResult<bool> {
    let changed = diesel::update(
        contact::table
            .filter(contact::tenant_id.eq(tenant))
            .filter(contact::id.eq(id)),
    )
    .set((
        contact::name.eq(None::<String>),
        contact::email.eq(None::<String>),
    ))
    .execute(conn)
    .await?;
    Ok(changed == 1)
}
