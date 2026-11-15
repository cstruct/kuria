//! Pushes from clones (`clone.server`): a message, each live draft's update
//! made against the base the clone read, and edits of other akter at the
//! revisions it read, all in one transaction, kept together or not at all.

use diesel_async::AsyncConnection;
use kuria_store::akter::AktRow;
use kuria_store::containers;
use uuid::Uuid;

use super::batch::{self, BatchError, Operation};
use super::{AktError, Akter, Caller};

/// The most characters a push's message holds.
pub const MESSAGE_MOST: usize = 200;

/// One live draft's part of a push: an update made against `base`.
#[derive(Clone, Debug)]
pub struct DraftPush {
    pub number: String,
    /// The draft as the clone read it, as its base: a Yjs v1 snapshot
    /// (`LiveDraft::base`).
    pub base: Vec<u8>,
    /// What the clone changed, a Yjs v1 update made against `base`.
    pub update: Vec<u8>,
}

/// What a push answers: each akt it wrote, its drafts' first, in order.
#[derive(Debug)]
pub struct Pushed {
    pub drafts: Vec<AktRow>,
    pub operations: Vec<AktRow>,
}

/// Which part of a push refused it, counting from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Draft(usize),
    Operation(usize),
}

/// Why a push was refused, and by which part, if one refused it.
#[derive(Debug)]
pub struct PushError {
    pub part: Option<Part>,
    pub error: AktError,
}

impl From<AktError> for PushError {
    fn from(error: AktError) -> Self {
        Self { part: None, error }
    }
}

impl From<diesel::result::Error> for PushError {
    fn from(error: diesel::result::Error) -> Self {
        AktError::from(error).into()
    }
}

impl From<BatchError> for PushError {
    fn from(refused: BatchError) -> Self {
        Self {
            part: refused.operation.map(Part::Operation),
            error: refused.error,
        }
    }
}

/// Refuses a message that isn't one line of 1 to [`MESSAGE_MOST`]
/// characters, and a push of nothing, too much, or a draft twice.
fn check(message: &str, drafts: &[DraftPush], operations: usize) -> Result<(), AktError> {
    let length = message.chars().count();
    if message.trim().is_empty() || length > MESSAGE_MOST || message.contains(['\n', '\r']) {
        return Err(AktError::BadRequest(format!(
            "a push's message is one line of 1 to {MESSAGE_MOST} characters"
        )));
    }
    if drafts.is_empty() && operations == 0 {
        return Err(AktError::BadRequest(
            "a push holds at least one draft or operation".to_owned(),
        ));
    }
    if drafts.len() > batch::MOST {
        return Err(AktError::BadRequest(format!(
            "a push holds at most {} drafts, not {}",
            batch::MOST,
            drafts.len()
        )));
    }
    let mut numbers: Vec<&str> = drafts.iter().map(|d| d.number.as_str()).collect();
    numbers.sort_unstable();
    if let Some([twice, ..]) = numbers.windows(2).find(|w| w[0] == w[1]) {
        return Err(AktError::BadRequest(format!(
            "a push holds {twice} as one draft, not more"
        )));
    }
    Ok(())
}

impl Akter {
    /// Pushes from a clone, as `caller`, with `message`: each of `drafts`,
    /// then `operations` as a batch runs them, each with the message as
    /// its reason, in one transaction, all of them or none (`clone.server`).
    // r[impl clone.server]
    pub async fn push_clone(
        &self,
        caller: Caller<'_>,
        message: &str,
        drafts: Vec<DraftPush>,
        mut operations: Vec<Operation>,
    ) -> Result<Pushed, PushError> {
        check(message, &drafts, operations.len())?;
        for operation in &mut operations {
            operation.set_reason(message);
        }
        let mut conn = self.pool.get().await.map_err(AktError::from)?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, PushError, _>(async |conn| {
                push_in(conn, caller, message, &drafts, operations).await
            })
            .await;
        self.woken(result)
    }
}

/// The body of [`Akter::push_clone`], in `conn`'s transaction.
async fn push_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    message: &str,
    drafts: &[DraftPush],
    mut operations: Vec<Operation>,
) -> Result<Pushed, PushError> {
    let tenant: Uuid = caller.actor.tenant.into();
    // Every lock up front, in the order every write takes them, as a batch
    // does (`akt.batch.locks`), drafts' akter with the operations'.
    let orders = operations.iter().any(Operation::orders);
    if orders {
        containers::lock_order(conn, tenant).await?;
    }
    let mut named: Vec<String> = drafts.iter().map(|d| d.number.clone()).collect();
    named.extend(batch::named(&mut operations));
    named.sort();
    named.dedup();
    let refs: Vec<&str> = named.iter().map(String::as_str).collect();
    containers::lock_akter(conn, tenant, &refs).await?;
    let mut written = Vec::with_capacity(drafts.len());
    for (i, draft) in drafts.iter().enumerate() {
        let row = super::live::push_in(
            conn,
            caller,
            &draft.number,
            &draft.base,
            &draft.update,
            message,
        )
        .await
        .map_err(|error| PushError {
            part: Some(Part::Draft(i + 1)),
            error,
        })?;
        written.push(row);
    }
    let rows = if operations.is_empty() {
        Vec::new()
    } else {
        batch::batch_in(conn, caller, operations).await?
    };
    Ok(Pushed {
        drafts: written,
        operations: rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(number: &str) -> DraftPush {
        DraftPush {
            number: number.to_owned(),
            base: Vec::new(),
            update: Vec::new(),
        }
    }

    // r[verify clone.server]
    #[test]
    fn a_push_holds_one_line_and_something_to_push() {
        assert!(check("Fix the intro", &[draft("D-1")], 0).is_ok());
        assert!(check("Fix the intro", &[], 1).is_ok());
        assert!(check(&"x".repeat(MESSAGE_MOST), &[], 1).is_ok());
        for message in ["", "  ", "two\nlines", "a\r", &"x".repeat(MESSAGE_MOST + 1)] {
            assert!(check(message, &[draft("D-1")], 0).is_err(), "{message:?}");
        }
        assert!(check("Nothing", &[], 0).is_err());
        assert!(check("Twice", &[draft("D-1"), draft("D-2"), draft("D-1")], 0).is_err());
    }
}
