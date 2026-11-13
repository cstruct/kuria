//! Pages of akter (`akt.page`): connections with opaque cursors that say
//! where a page ended, and which list it belongs to.

use async_graphql::{ErrorExtensions, SimpleObject};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::DateTime;
use kuria_store::akter::ChangedAfter;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::akter::Akt;

/// The most akter one page holds.
pub const MAX_PAGE: i32 = 200;
/// How many akter a page holds when not asked.
pub const DEFAULT_PAGE: i32 = 50;

/// How many akter a page asked for `first` holds.
pub fn page_size(first: Option<i32>) -> i64 {
    i64::from(first.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE))
}

/// A page of akter.
#[derive(SimpleObject)]
pub struct AktConnection {
    /// The page's akter, in the list's order.
    pub nodes: Vec<Akt>,
    pub page_info: PageInfo,
}

/// Where a page ends, and whether more follow.
#[derive(SimpleObject)]
pub struct PageInfo {
    /// A cursor after the page's last akt, to ask for the next page with
    /// `after`; `null` for an empty page.
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
}

/// What a cursor holds: the list it belongs to, by a digest of what the
/// list was asked for, and where its page ended.
#[derive(Serialize, Deserialize)]
struct Cursor {
    /// The list's fingerprint.
    l: String,
    /// The last akt's change time, in microseconds, for lists and search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    u: Option<i64>,
    /// The last akt's rank, for members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    r: Option<String>,
    i: Uuid,
}

/// Where a page of a list ended, as a cursor says it.
pub enum After {
    Changed(ChangedAfter),
    Ranked(String),
}

/// A list, as what it was asked for: its kind and arguments. Its
/// fingerprint ties cursors to it.
pub struct List(String);

impl List {
    /// A list of `kind` asked for with `arguments`, in a fixed order.
    pub fn new(kind: &str, arguments: &[Option<&str>]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(kind.as_bytes());
        for argument in arguments {
            // Absent and empty differ.
            match argument {
                Some(value) => {
                    hasher.update([1]);
                    hasher.update((value.len() as u64).to_be_bytes());
                    hasher.update(value.as_bytes());
                }
                None => hasher.update([0]),
            }
        }
        Self(URL_SAFE_NO_PAD.encode(&hasher.finalize()[..12]))
    }

    /// Where the page `after` names ended, or `BAD_REQUEST` if it isn't a
    /// cursor of this list.
    pub fn after(&self, after: Option<&str>) -> async_graphql::Result<Option<After>> {
        let Some(after) = after else {
            return Ok(None);
        };
        let refused = || {
            async_graphql::Error::new("that cursor isn't one of this list's")
                .extend_with(|_, e| e.set("code", "BAD_REQUEST"))
        };
        let cursor: Cursor = URL_SAFE_NO_PAD
            .decode(after)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or_else(refused)?;
        if cursor.l != self.0 {
            return Err(refused());
        }
        match (cursor.u, cursor.r) {
            (Some(micros), None) => DateTime::from_timestamp_micros(micros)
                .map(|updated_at| {
                    Some(After::Changed(ChangedAfter {
                        updated_at,
                        id: cursor.i,
                    }))
                })
                .ok_or_else(refused),
            (None, Some(rank)) => Ok(Some(After::Ranked(rank))),
            _ => Err(refused()),
        }
    }

    /// A page of `rows`, fetched one past its size `size` to learn whether
    /// more follow, ending where `end` says.
    pub fn page<R>(
        &self,
        mut rows: Vec<R>,
        size: i64,
        end: impl Fn(&R) -> After,
        id: impl Fn(&R) -> Uuid,
    ) -> (Vec<R>, PageInfo) {
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        let has_next_page = rows.len() > size;
        rows.truncate(size);
        let end_cursor = rows.last().map(|last| {
            let (u, r) = match end(last) {
                After::Changed(changed) => (Some(changed.updated_at.timestamp_micros()), None),
                After::Ranked(rank) => (None, Some(rank)),
            };
            let cursor = Cursor {
                l: self.0.clone(),
                u,
                r,
                i: id(last),
            };
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&cursor).unwrap_or_default())
        });
        (
            rows,
            PageInfo {
                end_cursor,
                has_next_page,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    // r[verify akt.page]
    #[test]
    fn cursors_belong_to_their_list() {
        let list = List::new("akter", &[Some("story"), None]);
        let id = Uuid::now_v7();
        let at = DateTime::from_timestamp_micros(Utc::now().timestamp_micros()).unwrap();
        let (_, info) = list.page(
            vec![0, 1, 2],
            2,
            |_| After::Changed(ChangedAfter { updated_at: at, id }),
            |_| id,
        );
        assert!(info.has_next_page);
        let cursor = info.end_cursor.unwrap();
        match list.after(Some(&cursor)).unwrap() {
            Some(After::Changed(after)) => assert_eq!(after, ChangedAfter { updated_at: at, id }),
            _ => panic!("not where the page ended"),
        }
        // Another list's, or no cursor at all, is refused.
        for other in [
            List::new("akter", &[Some("story"), Some("")]),
            List::new("akter", &[Some("bug"), None]),
            List::new("members", &[Some("story"), None]),
        ] {
            assert!(other.after(Some(&cursor)).is_err());
        }
        assert!(list.after(Some("not a cursor")).is_err());
        assert!(list.after(None).unwrap().is_none());
    }

    #[test]
    fn the_last_page_says_so() {
        let list = List::new("members", &[]);
        let (rows, info) = list.page(
            vec![1, 2],
            2,
            |_| After::Ranked("a".to_owned()),
            |_| Uuid::nil(),
        );
        assert_eq!(rows, [1, 2]);
        assert!(!info.has_next_page);
        let (rows, info) = list.page(
            Vec::<i32>::new(),
            2,
            |_| After::Ranked("a".to_owned()),
            |_| Uuid::nil(),
        );
        assert_eq!(rows, Vec::<i32>::new());
        assert_eq!(info.end_cursor, None);
    }
}
