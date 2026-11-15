//! Repositories' refs (`git.repo.storage`): each branch and tag of the git
//! repository an akt is, and the object it names. A ref moves only by a
//! compare-and-swap, so two pushes racing to one ref can't both succeed.
//! And what `main` holds of Kuria (`git.repo.commits`): its marks.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::schema::{
    akt, akt_commit, akt_draft, akt_draft_update, akt_revision, git_mark, git_ref,
};

/// The refs of the repository `akt`, as `(name, target)`, by name.
pub async fn refs(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
) -> QueryResult<Vec<(String, String)>> {
    git_ref::table
        .filter(git_ref::tenant_id.eq(tenant))
        .filter(git_ref::akt_id.eq(akt))
        .order(git_ref::name)
        .select((git_ref::name, git_ref::target))
        .load(conn)
        .await
}

/// Moves the ref `name` of the repository `akt` from `old` to `new`, `None`
/// for a ref that isn't there: creating, moving or deleting it, only if it
/// is still at `old`. Answers whether it moved. Run it in the transaction
/// that writes what the push changed, so the ref stays where it was if
/// anything else is refused.
// r[impl git.repo.storage]
pub async fn move_ref(
    conn: &mut Connection,
    tenant: Uuid,
    akt: Uuid,
    name: &str,
    old: Option<&str>,
    new: Option<&str>,
) -> QueryResult<bool> {
    let at = git_ref::table
        .filter(git_ref::tenant_id.eq(tenant))
        .filter(git_ref::akt_id.eq(akt))
        .filter(git_ref::name.eq(name));
    let moved = match (old, new) {
        (None, None) => return Ok(at.count().get_result::<i64>(conn).await? == 0),
        (None, Some(new)) => {
            diesel::insert_into(git_ref::table)
                .values((
                    git_ref::tenant_id.eq(tenant),
                    git_ref::akt_id.eq(akt),
                    git_ref::name.eq(name),
                    git_ref::target.eq(new),
                ))
                .on_conflict_do_nothing()
                .execute(conn)
                .await?
        }
        (Some(old), Some(new)) => {
            diesel::update(at.filter(git_ref::target.eq(old)))
                .set(git_ref::target.eq(new))
                .execute(conn)
                .await?
        }
        (Some(old), None) => {
            diesel::delete(at.filter(git_ref::target.eq(old)))
                .execute(conn)
                .await?
        }
    };
    Ok(moved == 1)
}

/// Every repository with a `main`, as `(tenant, akt, number)`.
pub async fn repositories(conn: &mut Connection) -> QueryResult<Vec<(Uuid, Uuid, String)>> {
    git_ref::table
        .inner_join(akt::table.on(akt::id.eq(git_ref::akt_id)))
        .filter(git_ref::name.eq("refs/heads/main"))
        .select((git_ref::tenant_id, git_ref::akt_id, akt::number))
        .order((git_ref::tenant_id, git_ref::akt_id))
        .load(conn)
        .await
}

/// What a repository's `main` holds of one akt (`git.repo.commits`): its
/// file there is as of this revision and this update of its live draft.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Queryable, Selectable)]
#[diesel(table_name = git_mark)]
pub struct Mark {
    pub akt_id: Uuid,
    pub revision: i32,
    pub draft_seq: i64,
}

/// The marks of the repository `repository`.
pub async fn marks(
    conn: &mut Connection,
    tenant: Uuid,
    repository: Uuid,
) -> QueryResult<Vec<Mark>> {
    git_mark::table
        .filter(git_mark::tenant_id.eq(tenant))
        .filter(git_mark::repository_id.eq(repository))
        .select(Mark::as_select())
        .load(conn)
        .await
}

/// Makes `marks` the repository's marks of their akter, and drops those of
/// any akt not among `akter`, which it no longer holds.
pub async fn set_marks(
    conn: &mut Connection,
    tenant: Uuid,
    repository: Uuid,
    marks: &[Mark],
    akter: &[Uuid],
) -> QueryResult<()> {
    diesel::delete(
        git_mark::table
            .filter(git_mark::tenant_id.eq(tenant))
            .filter(git_mark::repository_id.eq(repository))
            .filter(git_mark::akt_id.ne_all(akter)),
    )
    .execute(conn)
    .await?;
    for mark in marks {
        diesel::insert_into(git_mark::table)
            .values((
                git_mark::tenant_id.eq(tenant),
                git_mark::repository_id.eq(repository),
                git_mark::akt_id.eq(mark.akt_id),
                git_mark::revision.eq(mark.revision),
                git_mark::draft_seq.eq(mark.draft_seq),
            ))
            .on_conflict((
                git_mark::tenant_id,
                git_mark::repository_id,
                git_mark::akt_id,
            ))
            .do_update()
            .set((
                git_mark::revision.eq(mark.revision),
                git_mark::draft_seq.eq(mark.draft_seq),
            ))
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// Where an akt's live draft is: its last update, the last a release took,
/// and when it last changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DraftHead {
    pub akt_id: Uuid,
    pub last_seq: i64,
    pub released_seq: i64,
    pub updated_at: DateTime<Utc>,
}

/// The heads of the live drafts of `akter`, for those that have one stored.
pub async fn draft_heads(conn: &mut Connection, akter: &[Uuid]) -> QueryResult<Vec<DraftHead>> {
    if akter.is_empty() {
        return Ok(Vec::new());
    }
    let drafts: Vec<(Uuid, i64, i64, DateTime<Utc>)> = akt_draft::table
        .filter(akt_draft::akt_id.eq_any(akter))
        .select((
            akt_draft::akt_id,
            akt_draft::compacted_seq,
            akt_draft::released_seq,
            akt_draft::updated_at,
        ))
        .load(conn)
        .await?;
    let last: Vec<(Uuid, Option<i64>)> = akt_draft_update::table
        .filter(akt_draft_update::akt_id.eq_any(akter))
        .group_by(akt_draft_update::akt_id)
        .select((
            akt_draft_update::akt_id,
            diesel::dsl::max(akt_draft_update::seq),
        ))
        .load(conn)
        .await?;
    Ok(drafts
        .into_iter()
        .map(|(akt_id, compacted, released_seq, updated_at)| {
            let updated = last
                .iter()
                .find(|(id, _)| *id == akt_id)
                .and_then(|(_, seq)| *seq);
            DraftHead {
                akt_id,
                last_seq: updated.unwrap_or(0).max(compacted),
                released_seq,
                updated_at,
            }
        })
        .collect())
}

/// Who changed an akt in Kuria, and when, oldest first: by its revisions
/// after `after.revision` up to `through.revision`, other than those a push
/// made, and by its live draft's updates after `after.draft_seq` up to
/// `through.draft_seq`.
pub async fn changed_by(
    conn: &mut Connection,
    tenant: Uuid,
    after: Mark,
    through: Mark,
) -> QueryResult<Vec<(Uuid, DateTime<Utc>)>> {
    let pushed = akt_commit::table
        .filter(akt_commit::tenant_id.eq(tenant))
        .filter(akt_commit::akt_id.eq(after.akt_id))
        .select(akt_commit::revision);
    let mut changes: Vec<(Uuid, DateTime<Utc>)> = akt_revision::table
        .filter(akt_revision::tenant_id.eq(tenant))
        .filter(akt_revision::akt_id.eq(after.akt_id))
        .filter(akt_revision::revision.gt(after.revision))
        .filter(akt_revision::revision.le(through.revision))
        .filter(akt_revision::revision.ne_all(pushed))
        .select((akt_revision::author, akt_revision::created_at))
        .load(conn)
        .await?;
    changes.extend(
        akt_draft_update::table
            .filter(akt_draft_update::tenant_id.eq(tenant))
            .filter(akt_draft_update::akt_id.eq(after.akt_id))
            .filter(akt_draft_update::seq.gt(after.draft_seq))
            .filter(akt_draft_update::seq.le(through.draft_seq))
            .select((akt_draft_update::by, akt_draft_update::at))
            .load::<(Uuid, DateTime<Utc>)>(conn)
            .await?,
    );
    changes.sort_by_key(|(_, at)| *at);
    Ok(changes)
}
