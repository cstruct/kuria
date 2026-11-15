//! Akt types, akter, their revisions, and the change-event outbox.
//!
//! Every function takes the tenant, and every query filters on it. Writes
//! that belong together are meant to run in one transaction, which the
//! caller holds.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde_json::Value as Json;
use uuid::Uuid;

use crate::Connection;
use crate::schema::{
    akt, akt_origin, akt_revision, akt_type, akt_type_version, change_event, tenant_counter,
};

/// Takes the next value of a tenant's counter `name`, counting from 1. The
/// counter's row stays locked until the transaction ends.
pub async fn next_counter(conn: &mut Connection, tenant: Uuid, name: &str) -> QueryResult<i64> {
    diesel::insert_into(tenant_counter::table)
        .values((
            tenant_counter::tenant_id.eq(tenant),
            tenant_counter::name.eq(name),
            tenant_counter::value.eq(1_i64),
        ))
        .on_conflict((tenant_counter::tenant_id, tenant_counter::name))
        .do_update()
        .set(tenant_counter::value.eq(tenant_counter::value + 1))
        .returning(tenant_counter::value)
        .get_result(conn)
        .await
}

/// An akt type version, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_type_version)]
pub struct AktTypeVersion {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub key: String,
    pub version: i32,
    pub definition: Json,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Stores `definition` as the next version of the akt type `key`, and
/// returns it.
pub async fn insert_type_version(
    conn: &mut Connection,
    tenant: Uuid,
    key: &str,
    definition: &Json,
    created_by: Uuid,
    now: DateTime<Utc>,
) -> QueryResult<AktTypeVersion> {
    let version: i32 = diesel::insert_into(akt_type::table)
        .values((
            akt_type::tenant_id.eq(tenant),
            akt_type::key.eq(key),
            akt_type::latest_version.eq(1),
        ))
        .on_conflict((akt_type::tenant_id, akt_type::key))
        .do_update()
        .set(akt_type::latest_version.eq(akt_type::latest_version + 1))
        .returning(akt_type::latest_version)
        .get_result(conn)
        .await?;
    diesel::insert_into(akt_type_version::table)
        .values((
            akt_type_version::id.eq(Uuid::now_v7()),
            akt_type_version::tenant_id.eq(tenant),
            akt_type_version::key.eq(key),
            akt_type_version::version.eq(version),
            akt_type_version::definition.eq(definition),
            akt_type_version::created_by.eq(created_by),
            akt_type_version::created_at.eq(now),
        ))
        .returning(AktTypeVersion::as_returning())
        .get_result(conn)
        .await
}

/// The latest version of the akt type `key`.
pub async fn latest_type_version(
    conn: &mut Connection,
    tenant: Uuid,
    key: &str,
) -> QueryResult<Option<AktTypeVersion>> {
    akt_type_version::table
        .inner_join(
            akt_type::table.on(akt_type::tenant_id
                .eq(akt_type_version::tenant_id)
                .and(akt_type::key.eq(akt_type_version::key))
                .and(akt_type::latest_version.eq(akt_type_version::version))),
        )
        .filter(akt_type_version::tenant_id.eq(tenant))
        .filter(akt_type_version::key.eq(key))
        .select(AktTypeVersion::as_select())
        .first(conn)
        .await
        .optional()
}

/// A version of the akt type `key`.
pub async fn type_version(
    conn: &mut Connection,
    tenant: Uuid,
    key: &str,
    version: i32,
) -> QueryResult<Option<AktTypeVersion>> {
    akt_type_version::table
        .filter(akt_type_version::tenant_id.eq(tenant))
        .filter(akt_type_version::key.eq(key))
        .filter(akt_type_version::version.eq(version))
        .select(AktTypeVersion::as_select())
        .first(conn)
        .await
        .optional()
}

/// Akt type versions by id, in the tenant.
pub async fn type_versions_by_id(
    conn: &mut Connection,
    tenant: Uuid,
    ids: &[Uuid],
) -> QueryResult<Vec<AktTypeVersion>> {
    akt_type_version::table
        .filter(akt_type_version::tenant_id.eq(tenant))
        .filter(akt_type_version::id.eq_any(ids))
        .select(AktTypeVersion::as_select())
        .load(conn)
        .await
}

/// The latest version of every akt type in the tenant, by key.
pub async fn latest_type_versions(
    conn: &mut Connection,
    tenant: Uuid,
) -> QueryResult<Vec<AktTypeVersion>> {
    akt_type_version::table
        .inner_join(
            akt_type::table.on(akt_type::tenant_id
                .eq(akt_type_version::tenant_id)
                .and(akt_type::key.eq(akt_type_version::key))
                .and(akt_type::latest_version.eq(akt_type_version::version))),
        )
        .filter(akt_type_version::tenant_id.eq(tenant))
        .order(akt_type_version::key)
        .select(AktTypeVersion::as_select())
        .load(conn)
        .await
}

/// An akt's current state, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt)]
pub struct AktRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub number: String,
    pub type_version_id: Uuid,
    pub state: String,
    pub fields: Json,
    pub revision: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The akt's place in its tenant's priority order.
    pub rank: String,
    /// The value of its akt type's title field, kept by the write path.
    pub title: Option<String>,
}

/// An akt revision, as stored.
#[derive(Clone, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = akt_revision)]
pub struct RevisionRow {
    pub akt_id: Uuid,
    pub revision: i32,
    pub tenant_id: Uuid,
    pub author: Uuid,
    pub transition: Option<String>,
    pub reason: Option<String>,
    pub changes: Json,
    pub state: String,
    pub hash: Vec<u8>,
    pub created_at: DateTime<Utc>,
    /// For a membership revision, the membership it added or removed.
    pub membership: Option<Json>,
}

/// Stores a new akt.
pub async fn insert_akt(conn: &mut Connection, row: &AktRow) -> QueryResult<()> {
    diesel::insert_into(akt::table)
        .values(row)
        .execute(conn)
        .await
        .map(drop)
}

/// Replaces an akt's state, fields and revision, if it is still at
/// `expected_revision`. Returns whether it was.
pub async fn update_akt(
    conn: &mut Connection,
    row: &AktRow,
    expected_revision: i32,
) -> QueryResult<bool> {
    let updated = diesel::update(
        akt::table
            .filter(akt::tenant_id.eq(row.tenant_id))
            .filter(akt::id.eq(row.id))
            .filter(akt::revision.eq(expected_revision)),
    )
    .set((
        akt::state.eq(&row.state),
        akt::fields.eq(&row.fields),
        akt::revision.eq(row.revision),
        akt::updated_at.eq(row.updated_at),
        akt::title.eq(&row.title),
    ))
    .execute(conn)
    .await?;
    Ok(updated == 1)
}

/// Appends a revision, and its change event with the tenant's next change
/// sequence number, which it returns.
pub async fn append_revision(conn: &mut Connection, row: &RevisionRow) -> QueryResult<i64> {
    diesel::insert_into(akt_revision::table)
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
            change_event::created_at.eq(row.created_at),
            change_event::kind.eq("revision"),
        ))
        .execute(conn)
        .await?;
    Ok(seq)
}

/// Appends the change event that `akt`'s live draft changed, at its latest
/// revision `revision`, and returns its sequence number.
pub async fn append_draft_change(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    revision: i32,
    at: DateTime<Utc>,
) -> QueryResult<i64> {
    let seq = next_counter(conn, tenant, "change_event").await?;
    diesel::insert_into(change_event::table)
        .values((
            change_event::tenant_id.eq(tenant),
            change_event::seq.eq(seq),
            change_event::akt_id.eq(akt),
            change_event::revision.eq(revision),
            change_event::created_at.eq(at),
            change_event::kind.eq("draft"),
        ))
        .execute(conn)
        .await?;
    Ok(seq)
}

/// The akt numbered `number`.
pub async fn akt_by_number(
    conn: &mut Connection,
    tenant: Uuid,
    number: &str,
) -> QueryResult<Option<AktRow>> {
    akt::table
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt::number.eq(number))
        .select(AktRow::as_select())
        .first(conn)
        .await
        .optional()
}

/// The akt numbered `number`, locked until the transaction ends.
pub async fn lock_akt_by_number(
    conn: &mut Connection,
    tenant: Uuid,
    number: &str,
) -> QueryResult<Option<AktRow>> {
    akt::table
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt::number.eq(number))
        .select(AktRow::as_select())
        .for_update()
        .first(conn)
        .await
        .optional()
}

/// One condition on an akt's field values (`akt.filter`): an akt meets it
/// when any of its matches holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Condition(pub Vec<Match>);

/// One way an akt's fields can meet a condition.
#[derive(Clone, Debug, PartialEq)]
pub enum Match {
    /// The fields contain this JSON object (`@>`), as a value stores it.
    Contains(serde_json::Value),
    /// The checkbox `field` was checked by `subject` for themself: its
    /// `checked_by` is the subject and it says no `for`.
    CheckedForSelf { field: String, subject: String },
    /// The outcome of an akt with a `verdict` field of its own is
    /// `outcome` (`pass`, `fail`, `skip` or `pending`), from its verdicts in
    /// `verdict` and its assignees in `assignees` as
    /// `kuria_engine::outcomes::of_verdicts` works it out.
    Outcome {
        verdict: String,
        assignees: Option<String>,
        outcome: String,
    },
}

type AktFilter =
    Box<dyn BoxableExpression<akt::table, diesel::pg::Pg, SqlType = diesel::sql_types::Bool>>;

fn matched(m: &Match) -> AktFilter {
    use diesel::dsl::sql;
    use diesel::sql_types::Text;
    match m {
        Match::Contains(json) => Box::new(akt::fields.contains(json.clone())),
        Match::CheckedForSelf { field, subject } => Box::new(
            sql::<diesel::sql_types::Bool>("((akt.fields -> ")
                .bind::<Text, _>(field.clone())
                .sql(" ->> 'checked_by') = ")
                .bind::<Text, _>(subject.clone())
                .sql(" AND NOT ((akt.fields -> ")
                .bind::<Text, _>(field.clone())
                .sql(") ? 'for'))"),
        ),
        // r[impl akt.filter.outcome]
        // Per assignee, the verdict given for them, if any; then `fail` if
        // any failed, `pending` if none are assigned or one hasn't
        // answered, `skip` if all skipped, else `pass`. An array in place
        // of a missing or other value, as the engine reads one.
        Match::Outcome {
            verdict,
            assignees,
            outcome,
        } => {
            let assignees = assignees.clone().unwrap_or_default();
            Box::new(
                sql::<diesel::sql_types::Bool>(
                    "((SELECT CASE WHEN bool_or(v = 'fail') THEN 'fail' \
                       WHEN count(*) = 0 OR bool_or(v IS NULL) THEN 'pending' \
                       WHEN bool_and(v = 'skip') THEN 'skip' ELSE 'pass' END \
                     FROM (SELECT (SELECT g ->> 'verdict' FROM jsonb_array_elements(\
                         CASE WHEN jsonb_typeof(akt.fields -> ",
                )
                .bind::<Text, _>(verdict.clone())
                .sql(") = 'array' THEN akt.fields -> ")
                .bind::<Text, _>(verdict.clone())
                .sql(
                    " ELSE '[]'::jsonb END) g WHERE g -> 'assignee' = a LIMIT 1) AS v \
                     FROM jsonb_array_elements(CASE WHEN jsonb_typeof(akt.fields -> ",
                )
                .bind::<Text, _>(assignees.clone())
                .sql(") = 'array' THEN akt.fields -> ")
                .bind::<Text, _>(assignees)
                .sql(" ELSE '[]'::jsonb END) a) t) = ")
                .bind::<Text, _>(outcome.clone())
                .sql(")"),
            )
        }
    }
}

/// The ids of the tenant's akter that meet every condition, as a subquery.
// r[impl akt.filter]
pub(crate) fn meeting(
    tenant: Uuid,
    conditions: &[Condition],
) -> Option<akt::BoxedQuery<'static, diesel::pg::Pg, diesel::sql_types::Uuid>> {
    if conditions.is_empty() {
        return None;
    }
    let mut ids = akt::table
        .select(akt::id)
        .filter(akt::tenant_id.eq(tenant))
        .into_boxed();
    for condition in conditions {
        let any = condition
            .0
            .iter()
            .map(matched)
            .reduce(|a, b| Box::new(a.or(b)) as AktFilter);
        ids = match any {
            Some(any) => ids.filter(any),
            // A condition nothing can meet.
            None => ids.filter(diesel::dsl::sql::<diesel::sql_types::Bool>("false")),
        };
    }
    Some(ids)
}

/// Where a page of akter, most recently changed first, ends: the last akt's
/// change time and id. The next page starts after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChangedAfter {
    pub updated_at: DateTime<Utc>,
    pub id: Uuid,
}

impl ChangedAfter {
    /// Where a page ending with `row` ends.
    #[must_use]
    pub fn of(row: &AktRow) -> Self {
        Self {
            updated_at: row.updated_at,
            id: row.id,
        }
    }
}

/// Which akter a list or search answers, and which page of them.
#[derive(Clone, Copy, Debug)]
pub struct Listing<'a> {
    /// The akt types listed, by key.
    pub keys: &'a [String],
    /// Only akter in this state, if given.
    pub state: Option<&'a str>,
    /// Conditions every akt must meet.
    pub conditions: &'a [Condition],
    /// Where the previous page ended.
    pub after: Option<ChangedAfter>,
    pub limit: i64,
}

/// Up to `limit` akter of the listing's types, most recently changed first
/// (ties by id, descending), optionally only in its state, meeting every
/// condition, and after where the previous page ended.
// r[impl akt.page]
pub async fn list_akter(
    conn: &mut Connection,
    tenant: Uuid,
    listing: &Listing<'_>,
) -> QueryResult<Vec<AktRow>> {
    let Listing {
        keys,
        state,
        conditions,
        after,
        limit,
    } = *listing;
    let mut query = akt::table
        .inner_join(akt_type_version::table)
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt_type_version::key.eq_any(keys))
        .select(AktRow::as_select())
        .order((akt::updated_at.desc(), akt::id.desc()))
        .limit(limit)
        .into_boxed();
    if let Some(state) = state {
        query = query.filter(akt::state.eq(state.to_owned()));
    }
    if let Some(ids) = meeting(tenant, conditions) {
        query = query.filter(akt::id.eq_any(ids));
    }
    if let Some(after) = after {
        query = query.filter(
            akt::updated_at.lt(after.updated_at).or(akt::updated_at
                .eq(after.updated_at)
                .and(akt::id.lt(after.id))),
        );
    }
    query.load(conn).await
}

/// How many akter of the akt type `key` the tenant has, in any version.
pub async fn count_akter(conn: &mut Connection, tenant: Uuid, key: &str) -> QueryResult<i64> {
    akt::table
        .inner_join(akt_type_version::table)
        .filter(akt::tenant_id.eq(tenant))
        .filter(akt_type_version::key.eq(key))
        .count()
        .get_result(conn)
        .await
}

/// Escapes `%`, `_` and `\` for a `like` pattern.
fn like_escaped(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// The tenant's akter of the listing's types that match `text`: whose
/// number starts with it, or whose title contains each of its words,
/// ignoring case, and in `state` if given; the most recently changed first
/// (ties by id, descending), after `after`, at most `limit`.
// r[impl akt.search]
// r[impl akt.page]
pub async fn search_akter(
    conn: &mut Connection,
    tenant: Uuid,
    text: &str,
    listing: &Listing<'_>,
) -> QueryResult<Vec<AktRow>> {
    use diesel::dsl::sql;
    use diesel::sql_types::{Bool, Nullable, Text};

    let Listing {
        keys,
        state,
        conditions,
        after,
        limit,
    } = *listing;
    let text = text.trim().to_lowercase();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    diesel::define_sql_function!(fn lower(value: Nullable<Text>) -> Nullable<Text>);
    let number = lower(akt::number.nullable()).like(format!("{}%", like_escaped(&text)));
    let mut title: Box<
        dyn BoxableExpression<akt::table, diesel::pg::Pg, SqlType = Nullable<Bool>>,
    > = Box::new(sql::<Nullable<Bool>>("true"));
    for word in text.split_whitespace() {
        title = Box::new(title.and(lower(akt::title).like(format!("%{}%", like_escaped(word)))));
    }
    let mut matching = akt::table
        .filter(akt::tenant_id.eq(tenant))
        .filter(number.or(title))
        .filter(
            akt::type_version_id.eq_any(
                akt_type_version::table
                    .filter(akt_type_version::tenant_id.eq(tenant))
                    .filter(akt_type_version::key.eq_any(keys))
                    .select(akt_type_version::id),
            ),
        )
        .select(AktRow::as_select())
        .order((akt::updated_at.desc(), akt::id.desc()))
        .limit(limit)
        .into_boxed();
    if let Some(state) = state {
        matching = matching.filter(akt::state.eq(state.to_owned()));
    }
    if let Some(ids) = meeting(tenant, conditions) {
        matching = matching.filter(akt::id.eq_any(ids));
    }
    if let Some(after) = after {
        matching = matching.filter(
            akt::updated_at.lt(after.updated_at).or(akt::updated_at
                .eq(after.updated_at)
                .and(akt::id.lt(after.id))),
        );
    }
    matching.load(conn).await
}

/// An akt's revisions, oldest first.
pub async fn revisions(
    conn: &mut Connection,
    tenant: Uuid,
    akt_id: Uuid,
) -> QueryResult<Vec<RevisionRow>> {
    akt_revision::table
        .filter(akt_revision::tenant_id.eq(tenant))
        .filter(akt_revision::akt_id.eq(akt_id))
        .order(akt_revision::revision)
        .select(RevisionRow::as_select())
        .load(conn)
        .await
}

/// A change event, as stored.
#[derive(Clone, Debug, Queryable, Selectable)]
#[diesel(table_name = change_event)]
pub struct ChangeEventRow {
    pub tenant_id: Uuid,
    pub seq: i64,
    pub akt_id: Uuid,
    /// The new revision, for a `revision` event; `None` for a `rank` event.
    pub revision: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    /// `revision` or `rank`.
    pub kind: String,
}

/// The tenant's change events after `after`, in order, up to `limit`.
pub async fn change_events_after(
    conn: &mut Connection,
    tenant: Uuid,
    after: i64,
    limit: i64,
) -> QueryResult<Vec<ChangeEventRow>> {
    change_event::table
        .filter(change_event::tenant_id.eq(tenant))
        .filter(change_event::seq.gt(after))
        .order(change_event::seq)
        .limit(limit)
        .select(ChangeEventRow::as_select())
        .load(conn)
        .await
}

/// Up to `limit` unpublished change events of any tenant, oldest first,
/// locked until the transaction ends and skipping rows another transaction
/// holds, so instances relaying at once never take the same row.
pub async fn take_unpublished(conn: &mut Connection, limit: i64) -> QueryResult<Vec<(Uuid, i64)>> {
    change_event::table
        .filter(change_event::published_at.is_null())
        .order((change_event::created_at, change_event::seq))
        .limit(limit)
        .select((change_event::tenant_id, change_event::seq))
        .for_update()
        .skip_locked()
        .load(conn)
        .await
}

/// Marks the change events `events` published at `now`.
pub async fn mark_published(
    conn: &mut Connection,
    events: &[(Uuid, i64)],
    now: DateTime<Utc>,
) -> QueryResult<()> {
    for (tenant, seq) in events {
        diesel::update(change_event::table.find((tenant, seq)))
            .set(change_event::published_at.eq(now))
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// The tenant's last change event's sequence number, or 0.
pub async fn last_change_seq(conn: &mut Connection, tenant: Uuid) -> QueryResult<i64> {
    change_event::table
        .filter(change_event::tenant_id.eq(tenant))
        .select(diesel::dsl::max(change_event::seq))
        .first::<Option<i64>>(conn)
        .await
        .map(Option::unwrap_or_default)
}

/// A change event with what a subscriber needs to see it: the akt's
/// number and akt type key.
#[derive(Clone, Debug, PartialEq, Eq, Queryable)]
pub struct Change {
    pub seq: i64,
    pub akt_id: Uuid,
    /// `revision` or `rank`.
    pub kind: String,
    pub revision: Option<i32>,
    pub number: String,
    pub akt_type: String,
}

/// The tenant's change events after `after`, in sequence order, up to
/// `limit`, each with its akt's number and akt type key.
pub async fn changes_after(
    conn: &mut Connection,
    tenant: Uuid,
    after: i64,
    limit: i64,
) -> QueryResult<Vec<Change>> {
    change_event::table
        .inner_join(akt::table.on(akt::id.eq(change_event::akt_id)))
        .inner_join(akt_type_version::table.on(akt_type_version::id.eq(akt::type_version_id)))
        .filter(change_event::tenant_id.eq(tenant))
        .filter(change_event::seq.gt(after))
        .order(change_event::seq)
        .limit(limit)
        .select((
            change_event::seq,
            change_event::akt_id,
            change_event::kind,
            change_event::revision,
            akt::number,
            akt_type_version::key,
        ))
        .load(conn)
        .await
}

/// Records that `akt` was copied from `origin` (`akt.copy.record`).
pub async fn insert_origin(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    origin: Uuid,
) -> QueryResult<()> {
    diesel::insert_into(akt_origin::table)
        .values((
            akt_origin::tenant_id.eq(tenant),
            akt_origin::akt_id.eq(akt),
            akt_origin::origin_id.eq(origin),
        ))
        .execute(conn)
        .await
        .map(drop)
}

/// The akt `akt` was copied from, if it is a copy.
pub async fn origin_of(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
) -> QueryResult<Option<AktRow>> {
    akt_origin::table
        .inner_join(akt::table.on(akt::id.eq(akt_origin::origin_id)))
        .filter(akt_origin::tenant_id.eq(tenant))
        .filter(akt_origin::akt_id.eq(akt))
        .select(AktRow::as_select())
        .first(conn)
        .await
        .optional()
}

/// The origins of the copies among `akter`, as `(copy, origin)`.
pub async fn origins_of(
    conn: &mut Connection,
    tenant: Uuid,
    akter: &[Uuid],
) -> QueryResult<Vec<(Uuid, Uuid)>> {
    akt_origin::table
        .filter(akt_origin::tenant_id.eq(tenant))
        .filter(akt_origin::akt_id.eq_any(akter))
        .select((akt_origin::akt_id, akt_origin::origin_id))
        .load(conn)
        .await
}
