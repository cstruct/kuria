//! Versions (`version.*`): revisions of an akt named by releasing them.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde_json::Value as Json;
use uuid::Uuid;

use crate::Connection;
use crate::akter::next_counter;
use crate::schema::{akt_commit, akt_version, change_event};

/// One version, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_version)]
pub struct VersionRow {
    pub tenant_id: Uuid,
    pub akt_id: Uuid,
    pub number: i32,
    pub revision: i32,
    pub label: Option<String>,
    pub released_by: Uuid,
    pub released_at: DateTime<Utc>,
    pub pins: Json,
}

/// Stores `row`, and its change event with the tenant's next change
/// sequence number, which it returns (`version.history`).
// r[impl version.history]
pub async fn insert(conn: &mut Connection, row: &VersionRow) -> QueryResult<i64> {
    diesel::insert_into(akt_version::table)
        .values(row)
        .execute(conn)
        .await?;
    let seq = next_counter(conn, row.tenant_id, "change_event").await?;
    diesel::insert_into(change_event::table)
        .values((
            change_event::tenant_id.eq(row.tenant_id),
            change_event::seq.eq(seq),
            change_event::akt_id.eq(row.akt_id),
            change_event::revision.eq(row.revision),
            change_event::created_at.eq(row.released_at),
            change_event::kind.eq("version"),
        ))
        .execute(conn)
        .await?;
    Ok(seq)
}

/// The versions of `akt`, latest first.
// r[impl version.list]
pub async fn of(conn: &mut Connection, tenant: Uuid, akt: Uuid) -> QueryResult<Vec<VersionRow>> {
    akt_version::table
        .filter(akt_version::tenant_id.eq(tenant))
        .filter(akt_version::akt_id.eq(akt))
        .order(akt_version::number.desc())
        .select(VersionRow::as_select())
        .load(conn)
        .await
}

/// The version `number` of `akt`, if it has one.
pub async fn one(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    number: i32,
) -> QueryResult<Option<VersionRow>> {
    akt_version::table
        .filter(akt_version::tenant_id.eq(tenant))
        .filter(akt_version::akt_id.eq(akt))
        .filter(akt_version::number.eq(number))
        .select(VersionRow::as_select())
        .first(conn)
        .await
        .optional()
}

/// Keeps `commit`, a git commit, with each revision made since: those whose
/// change events came after `after`.
pub async fn keep_commit(
    conn: &mut Connection,
    tenant: Uuid,
    after: i64,
    commit: &Json,
) -> QueryResult<()> {
    let made: Vec<(Uuid, Option<i32>)> = change_event::table
        .filter(change_event::tenant_id.eq(tenant))
        .filter(change_event::seq.gt(after))
        .filter(change_event::kind.eq("revision"))
        .select((change_event::akt_id, change_event::revision))
        .load(conn)
        .await?;
    for (akt_id, revision) in made {
        let Some(revision) = revision else { continue };
        diesel::insert_into(akt_commit::table)
            .values((
                akt_commit::tenant_id.eq(tenant),
                akt_commit::akt_id.eq(akt_id),
                akt_commit::revision.eq(revision),
                akt_commit::commit.eq(commit),
            ))
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// The commits kept with `akt`'s revisions, by revision.
pub async fn commits_of(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
) -> QueryResult<Vec<(i32, Json)>> {
    akt_commit::table
        .filter(akt_commit::tenant_id.eq(tenant))
        .filter(akt_commit::akt_id.eq(akt))
        .select((akt_commit::revision, akt_commit::commit))
        .load(conn)
        .await
}

/// Every version of `akter`, oldest first per akt (`bundle.contents`).
pub async fn of_akter(conn: &mut Connection, akter: &[Uuid]) -> QueryResult<Vec<VersionRow>> {
    akt_version::table
        .filter(akt_version::akt_id.eq_any(akter))
        .order((akt_version::akt_id, akt_version::number))
        .select(VersionRow::as_select())
        .load(conn)
        .await
}

/// The commits kept with the revisions of `akter`, as (akt, revision,
/// commit), oldest first per akt (`bundle.contents`).
pub async fn commits_of_akter(
    conn: &mut Connection,
    akter: &[Uuid],
) -> QueryResult<Vec<(Uuid, i32, Json)>> {
    akt_commit::table
        .filter(akt_commit::akt_id.eq_any(akter))
        .order((akt_commit::akt_id, akt_commit::revision))
        .select((akt_commit::akt_id, akt_commit::revision, akt_commit::commit))
        .load(conn)
        .await
}

/// Stores imported versions and commits as they were, without change
/// events of their own: the import records one per akt
/// (`bundle.import.history`).
pub async fn insert_imported(
    conn: &mut Connection,
    versions: &[VersionRow],
    commits: &[(Uuid, Uuid, i32, Json)],
) -> QueryResult<()> {
    // Postgres takes at most 65535 parameters to a statement.
    for chunk in versions.chunks(4000) {
        diesel::insert_into(akt_version::table)
            .values(chunk)
            .execute(conn)
            .await?;
    }
    for chunk in commits.chunks(8000) {
        let rows: Vec<_> = chunk
            .iter()
            .map(|(tenant, akt, revision, commit)| {
                (
                    akt_commit::tenant_id.eq(*tenant),
                    akt_commit::akt_id.eq(*akt),
                    akt_commit::revision.eq(*revision),
                    akt_commit::commit.eq(commit),
                )
            })
            .collect();
        diesel::insert_into(akt_commit::table)
            .values(&rows)
            .execute(conn)
            .await?;
    }
    Ok(())
}
