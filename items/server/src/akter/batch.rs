//! Batches: several akt writes run in order in one transaction, all of them
//! or none (`akt.batch`).
//!
//! A batch takes its locks before its first operation, in the order every
//! write takes them, so that its operations only take again locks it holds
//! (`akt.batch.locks`). An operation names an akt an earlier `createAkt` of
//! the batch made as `@<n>` (`akt.batch.refer`).

use diesel_async::AsyncConnection;
use kuria_store::akter::AktRow;
use kuria_store::containers;
use serde_json::{Map, Value as Json};
use uuid::Uuid;

use super::{AktError, Akter, Caller, Place};

/// The most operations a batch holds (`akt.batch.limit`).
pub const MOST: usize = 200;

/// One akt write in a batch, with what its mutation takes.
#[derive(Clone, Debug)]
pub enum Operation {
    Create {
        key: String,
        fields: Map<String, Json>,
        reason: Option<String>,
        inside: Vec<String>,
    },
    Edit {
        number: String,
        revision: i32,
        fields: Map<String, Json>,
        reason: Option<String>,
    },
    Transition {
        number: String,
        revision: i32,
        transition: String,
        fields: Map<String, Json>,
        reason: Option<String>,
    },
    AddMember {
        container: String,
        member: String,
        reason: Option<String>,
    },
    RemoveMember {
        container: String,
        member: String,
        reason: Option<String>,
    },
    Transfer {
        member: String,
        from: String,
        to: String,
        reason: Option<String>,
    },
    AddLink {
        source: String,
        kind: String,
        target: String,
        pin: Option<i32>,
        reason: Option<String>,
    },
    RemoveLink {
        source: String,
        kind: String,
        target: String,
        reason: Option<String>,
    },
    Move {
        number: String,
        /// Before `next_to`, else after it.
        before: bool,
        next_to: String,
    },
    Draft {
        number: String,
        revision: i32,
        field: String,
        reason: Option<String>,
    },
}

impl Operation {
    /// Every akt number the operation names.
    fn numbers_mut(&mut self) -> Vec<&mut String> {
        match self {
            Self::Create { inside, .. } => inside.iter_mut().collect(),
            Self::Edit { number, .. }
            | Self::Transition { number, .. }
            | Self::Draft { number, .. } => vec![number],
            Self::AddMember {
                container, member, ..
            }
            | Self::RemoveMember {
                container, member, ..
            } => vec![container, member],
            Self::Transfer {
                member, from, to, ..
            } => vec![member, from, to],
            Self::AddLink { source, target, .. } | Self::RemoveLink { source, target, .. } => {
                vec![source, target]
            }
            Self::Move {
                number, next_to, ..
            } => vec![number, next_to],
        }
    }

    /// Makes `reason` its reason, as a push's message is each of its
    /// revisions' (`clone.server`); a move has none.
    pub(super) fn set_reason(&mut self, message: &str) {
        match self {
            Self::Create { reason, .. }
            | Self::Edit { reason, .. }
            | Self::Transition { reason, .. }
            | Self::AddMember { reason, .. }
            | Self::RemoveMember { reason, .. }
            | Self::Transfer { reason, .. }
            | Self::AddLink { reason, .. }
            | Self::RemoveLink { reason, .. }
            | Self::Draft { reason, .. } => *reason = Some(message.to_owned()),
            Self::Move { .. } => {}
        }
    }

    /// Whether it takes the tenant's order lock: it gives an akt a rank.
    pub(super) fn orders(&self) -> bool {
        matches!(self, Self::Create { .. } | Self::Move { .. })
    }
}

/// Why a batch was refused, and by which operation, counting from 1, if
/// one refused it.
#[derive(Debug)]
pub struct BatchError {
    pub operation: Option<usize>,
    pub error: AktError,
}

impl From<AktError> for BatchError {
    fn from(error: AktError) -> Self {
        Self {
            operation: None,
            error,
        }
    }
}

impl From<diesel::result::Error> for BatchError {
    fn from(error: diesel::result::Error) -> Self {
        AktError::from(error).into()
    }
}

/// The operation `@<n>` names, counting from 1, if `number` is one.
fn referred(number: &str) -> Option<Result<usize, ()>> {
    let n = number.strip_prefix('@')?;
    Some(n.parse::<usize>().ok().filter(|n| *n >= 1).ok_or(()))
}

/// Refuses a batch of no operations, too many, or an `@<n>` that names
/// anything but an earlier `createAkt`, before any operation runs.
// r[impl akt.batch.limit]
// r[impl akt.batch.refer]
fn check(operations: &mut [Operation]) -> Result<(), BatchError> {
    if operations.is_empty() || operations.len() > MOST {
        return Err(AktError::BadRequest(format!(
            "a batch holds from 1 to {MOST} operations, not {}",
            operations.len()
        ))
        .into());
    }
    let creates: Vec<bool> = operations
        .iter()
        .map(|o| matches!(o, Operation::Create { .. }))
        .collect();
    for (i, operation) in operations.iter_mut().enumerate() {
        for number in operation.numbers_mut() {
            let Some(n) = referred(number) else { continue };
            let earlier_create = n.is_ok_and(|n| n <= i && creates[n - 1]);
            if !earlier_create {
                return Err(BatchError {
                    operation: Some(i + 1),
                    error: AktError::BadRequest(format!(
                        "`{number}` names no `createAkt` before this operation"
                    )),
                });
            }
        }
    }
    Ok(())
}

impl Akter {
    /// Runs `operations` in order in one transaction, all of them or none,
    /// and answers the akt each one changed.
    // r[impl akt.batch]
    // r[impl akt.batch.atomic]
    // r[impl akt.batch.published]
    pub async fn batch(
        &self,
        caller: Caller<'_>,
        operations: Vec<Operation>,
    ) -> Result<Vec<AktRow>, BatchError> {
        let mut conn = self.pool.get().await.map_err(AktError::from)?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, BatchError, _>(async |conn| batch_in(conn, caller, operations).await)
            .await;
        // The relay is woken once, and only if it all committed.
        self.woken(result)
    }

    /// Wakes the relay if `result`, of a transaction that wrote akter, is
    /// committed.
    pub(crate) fn woken<T, E>(&self, result: Result<T, E>) -> Result<T, E> {
        if result.is_ok()
            && let Some(published) = &self.published
        {
            published.notify_one();
        }
        result
    }
}

/// Runs `operations` as [`Akter::batch`] does, in `conn`'s transaction.
pub(super) async fn batch_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    mut operations: Vec<Operation>,
) -> Result<Vec<AktRow>, BatchError> {
    check(&mut operations)?;
    let tenant: Uuid = caller.actor.tenant.into();
    lock(conn, tenant, &mut operations).await?;
    // Each operation's akt, by position, for `@<n>`.
    let mut created: Vec<String> = Vec::with_capacity(operations.len());
    let mut rows = Vec::with_capacity(operations.len());
    for (i, operation) in operations.iter().enumerate() {
        let mut operation = operation.clone();
        // r[impl akt.batch.refer]
        for number in operation.numbers_mut() {
            if let Some(Ok(n)) = referred(number) {
                let made = created.get(n - 1).ok_or_else(|| BatchError {
                    operation: Some(i + 1),
                    error: AktError::Internal(format!("`{number}` passed the check")),
                })?;
                number.clone_from(made);
            }
        }
        let row = run(conn, caller, operation)
            .await
            .map_err(|error| BatchError {
                operation: Some(i + 1),
                error,
            })?;
        created.push(row.number.clone());
        rows.push(row);
    }
    Ok(rows)
}

/// Takes the batch's locks before any operation: the tenant's order lock if
/// any operation gives an akt a rank, then every existing akt named, by id.
// r[impl akt.batch.locks]
async fn lock(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    operations: &mut [Operation],
) -> Result<(), BatchError> {
    if operations.iter().any(Operation::orders) {
        containers::lock_order(conn, tenant).await?;
    }
    let mut named = named(operations);
    named.sort();
    named.dedup();
    let refs: Vec<&str> = named.iter().map(String::as_str).collect();
    // Those missing are refused by the operation that names them.
    containers::lock_akter(conn, tenant, &refs).await?;
    Ok(())
}

/// Every existing akt `operations` name: not those `@<n>` names.
pub(super) fn named(operations: &mut [Operation]) -> Vec<String> {
    operations
        .iter_mut()
        .flat_map(Operation::numbers_mut)
        .filter(|n| referred(n).is_none())
        .map(|n| n.clone())
        .collect()
}

/// One operation, run as its mutation would run it, in `conn`'s
/// transaction.
// r[impl akt.batch]
async fn run(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    operation: Operation,
) -> Result<AktRow, AktError> {
    match operation {
        Operation::Create {
            key,
            fields,
            reason,
            inside,
        } => super::create::create_in(conn, caller, &key, &fields, reason, &inside).await,
        Operation::Edit {
            number,
            revision,
            fields,
            reason,
        } => super::edit_in(conn, caller, &number, revision, &fields, reason).await,
        Operation::Transition {
            number,
            revision,
            transition,
            fields,
            reason,
        } => {
            super::transition_in(
                conn,
                caller,
                &number,
                revision,
                &transition,
                &fields,
                reason,
            )
            .await
        }
        Operation::AddMember {
            container,
            member,
            reason,
        } => super::membership::add_member_in(conn, caller, &container, &member, reason).await,
        Operation::RemoveMember {
            container,
            member,
            reason,
        } => super::membership::remove_member_in(conn, caller, &container, &member, reason).await,
        Operation::Transfer {
            member,
            from,
            to,
            reason,
        } => super::membership::transfer_member_in(conn, caller, &member, &from, &to, reason).await,
        Operation::AddLink {
            source,
            kind,
            target,
            pin,
            reason,
        } => super::links::add_link_in(conn, caller, &source, &kind, &target, pin, reason).await,
        Operation::RemoveLink {
            source,
            kind,
            target,
            reason,
        } => super::links::remove_link_in(conn, caller, &source, &kind, &target, reason).await,
        Operation::Move {
            number,
            before,
            next_to,
        } => {
            let place = if before {
                Place::Before(&next_to)
            } else {
                Place::After(&next_to)
            };
            super::ordering::move_akt_in(conn, caller, &number, place).await
        }
        Operation::Draft {
            number,
            revision,
            field,
            reason,
        } => super::draft::draft_field_in(conn, caller, &number, revision, &field, reason).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(number: &str) -> Operation {
        Operation::Edit {
            number: number.to_owned(),
            revision: 1,
            fields: Map::new(),
            reason: None,
        }
    }

    fn create() -> Operation {
        Operation::Create {
            key: "task".to_owned(),
            fields: Map::new(),
            reason: None,
            inside: Vec::new(),
        }
    }

    // r[verify akt.batch.refer]
    // r[verify akt.batch.limit]
    #[test]
    fn refuses_references_to_anything_but_an_earlier_create() {
        assert!(check(&mut [create(), edit("@1")]).is_ok());
        assert!(check(&mut [create(), edit("T-1"), edit("@1")]).is_ok());
        for (mut batch, at) in [
            (vec![edit("@1")], 1),
            (vec![edit("@2"), create()], 1),
            (vec![edit("T-1"), edit("@1")], 2),
            (vec![create(), edit("@0")], 2),
            (vec![create(), edit("@x")], 2),
        ] {
            let refused = check(&mut batch).unwrap_err();
            assert_eq!(refused.operation, Some(at));
            assert!(matches!(refused.error, AktError::BadRequest(_)));
        }
        assert!(check(&mut []).is_err());
        assert!(check(&mut vec![edit("T-1"); MOST]).is_ok());
        assert!(check(&mut vec![edit("T-1"); MOST + 1]).is_err());
    }
}
