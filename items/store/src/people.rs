//! The names and emails Kuria recorded for subjects at sign-in.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::schema::{person, tenant_person};

/// A person, as stored.
#[derive(Clone, Debug, PartialEq, Eq, Insertable, Queryable, Selectable)]
#[diesel(table_name = person)]
pub struct Person {
    pub subject_id: Uuid,
    pub name: Option<String>,
    pub email: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Records `person`, replacing what was recorded before.
pub async fn record(conn: &mut Connection, person: &Person) -> QueryResult<()> {
    diesel::insert_into(person::table)
        .values(person)
        .on_conflict(person::subject_id)
        .do_update()
        .set((
            person::name.eq(excluded(person::name)),
            person::email.eq(excluded(person::email)),
            person::updated_at.eq(excluded(person::updated_at)),
        ))
        .execute(conn)
        .await
        .map(drop)
}

/// The people recorded for `subjects`, in no particular order.
pub async fn find(conn: &mut Connection, subjects: &[Uuid]) -> QueryResult<Vec<Person>> {
    person::table
        .filter(person::subject_id.eq_any(subjects))
        .select(Person::as_select())
        .load(conn)
        .await
}

/// Records that `subject` signed in to `tenant` at `at`.
// r[impl person.tenant]
pub async fn signed_in_to(
    conn: &mut Connection,
    tenant: Uuid,
    subject: Uuid,
    at: DateTime<Utc>,
) -> QueryResult<()> {
    diesel::insert_into(tenant_person::table)
        .values((
            tenant_person::tenant_id.eq(tenant),
            tenant_person::subject_id.eq(subject),
            tenant_person::first_seen_at.eq(at),
            tenant_person::last_seen_at.eq(at),
        ))
        .on_conflict((tenant_person::tenant_id, tenant_person::subject_id))
        .do_update()
        .set(tenant_person::last_seen_at.eq(excluded(tenant_person::last_seen_at)))
        .execute(conn)
        .await
        .map(drop)
}

/// Whether `subject` has signed in to `tenant`.
pub async fn is_in(conn: &mut Connection, tenant: Uuid, subject: Uuid) -> QueryResult<bool> {
    diesel::select(diesel::dsl::exists(
        tenant_person::table.find((tenant, subject)),
    ))
    .get_result(conn)
    .await
}

/// The subjects who signed in to `tenant`, with what was recorded of them,
/// most recently seen first.
pub async fn in_tenant(
    conn: &mut Connection,
    tenant: Uuid,
) -> QueryResult<Vec<(Uuid, Option<Person>)>> {
    tenant_person::table
        .left_join(person::table.on(person::subject_id.eq(tenant_person::subject_id)))
        .filter(tenant_person::tenant_id.eq(tenant))
        .order(tenant_person::last_seen_at.desc())
        .select((tenant_person::subject_id, Option::<Person>::as_select()))
        .load(conn)
        .await
}
