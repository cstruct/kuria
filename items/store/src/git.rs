//! Repositories' refs (`git.repo.storage`): each branch and tag of the git
//! repository an akt is, and the object it names. A ref moves only by a
//! compare-and-swap, so two pushes racing to one ref can't both succeed.

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::Connection;
use crate::schema::git_ref;

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
