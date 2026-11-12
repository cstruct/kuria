//! The names and emails Kuria recorded for subjects at sign-in.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::schema::person;

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
