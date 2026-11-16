//! Archive rules (`archive.rules`): applied on the changes that can make
//! them true (`archive.rules.on-change`), and by the sweep
//! (`archive.sweep`).

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use kuria_config::ArchiveRule;
use kuria_store::akter::{self as store, AktRow, Archived};
use kuria_store::containers;
use serde_json::json;
use uuid::Uuid;

use super::archive::{By, set};
use super::{AktError, StoredType, type_of};

/// The latest version of each akt type, by key, read once per pass: rules
/// added to a type apply to akter of its earlier versions too.
#[derive(Default)]
pub(crate) struct Types(HashMap<String, Option<StoredType>>);

impl Types {
    pub(crate) async fn latest(
        &mut self,
        conn: &mut kuria_store::Connection,
        tenant: Uuid,
        key: &str,
    ) -> Result<Option<&StoredType>, AktError> {
        if !self.0.contains_key(key) {
            let found = match store::latest_type_version(conn, tenant, key).await? {
                Some(row) => Some(StoredType::read(row)?),
                None => None,
            };
            self.0.insert(key.to_owned(), found);
        }
        Ok(self.0[key].as_ref())
    }
}

/// The rules an akt type declares.
pub(crate) fn rules_of(ty: &StoredType) -> Vec<ArchiveRule<'_>> {
    ty.ty
        .document()
        .archive
        .iter()
        .flat_map(|a| &a.rules)
        .filter_map(kuria_config::ArchiveRuleDef::rule)
        .collect()
}

/// `number` locked, as it now is.
async fn lock(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    number: &str,
) -> Result<AktRow, AktError> {
    containers::lock_akter(conn, tenant, &[number])
        .await?
        .pop()
        .ok_or_else(|| AktError::Internal(format!("akt {number} vanished")))
}

/// Archives `akt` by the rule `kind`, unless a person kept it, and follows
/// the rules down its members (`archive.rules.kept`).
async fn archive_by(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    akt: &AktRow,
    kind: &'static str,
    now: DateTime<Utc>,
) -> Result<bool, AktError> {
    let locked = lock(conn, tenant, &akt.number).await?;
    if locked.archived || locked.archive_kept {
        return Ok(false);
    }
    let ty = type_of(conn, tenant, &locked).await?;
    let by = By::Rule {
        kind,
        container: None,
    };
    let archived = set(conn, locked, &ty, true, by, None, now).await?;
    follow(conn, tenant, &archived, &ty, now).await?;
    Ok(true)
}

/// After `akt` was archived or unarchived, archives the members that go
/// with it by `with_container`, or unarchives those it took, and so on down
/// (`archive.rules.on-change`, `archive.rules.container`).
// r[impl archive.rules.on-change]
// r[impl archive.rules.container]
pub(crate) async fn follow(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    akt: &AktRow,
    ty: &StoredType,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    let mut types = Types::default();
    let mut queue = vec![(akt.id, ty.ty.key().to_owned(), akt.archived)];
    while let Some((container, key, archived)) = queue.pop() {
        let took = json!({ "rule": "with_container", "container": container });
        for (member, member_key) in containers::members_with_types(conn, tenant, container).await? {
            let goes = if archived {
                // r[impl archive.rules.kept]
                !member.archived
                    && !member.archive_kept
                    && types
                        .latest(conn, tenant, &member_key)
                        .await?
                        .is_some_and(|t| {
                            rules_of(t)
                                .iter()
                                .any(|r| *r == ArchiveRule::WithContainer(&key))
                        })
            } else {
                member.archived && member.archived_by.as_ref() == Some(&took)
            };
            if !goes {
                continue;
            }
            let locked = lock(conn, tenant, &member.number).await?;
            let own = type_of(conn, tenant, &locked).await?;
            let by = By::Rule {
                kind: "with_container",
                container: Some(container),
            };
            let moved = set(conn, locked, &own, archived, by, None, now).await?;
            queue.push((moved.id, member_key, archived));
        }
    }
    Ok(())
}

/// After `akt` of `ty` reached its state, archives the akter of its type
/// that its `newer` rules say are overtaken (`archive.rules.on-change`).
pub(crate) async fn after_transition(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    akt: &AktRow,
    ty: &StoredType,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    let key = ty.ty.key().to_owned();
    let mut types = Types::default();
    let Some(latest) = types.latest(conn, tenant, &key).await? else {
        return Ok(());
    };
    let newer: Vec<(u32, Vec<String>, Option<String>)> = rules_of(latest)
        .into_iter()
        .filter_map(|r| match r {
            ArchiveRule::Newer {
                count,
                states,
                within,
            } if states.contains(&akt.state) => {
                Some((count, states.to_vec(), within.map(str::to_owned)))
            }
            _ => None,
        })
        .collect();
    for (count, states, within) in newer {
        apply_newer(conn, tenant, &key, count, &states, within.as_deref(), now).await?;
    }
    Ok(())
}

/// Archives each live akt of the type `key` in one of `states` that at
/// least `count` others of its type, in one of `states` too and sharing a
/// container of the type `within` if given, reached after it did. Answers
/// how many it archived.
pub(crate) async fn apply_newer(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    key: &str,
    count: u32,
    states: &[String],
    within: Option<&str>,
    now: DateTime<Utc>,
) -> Result<usize, AktError> {
    let all = store::akter_in_states(conn, tenant, key, states, Archived::All).await?;
    let mut shared: HashMap<Uuid, HashSet<Uuid>> = HashMap::new();
    if let Some(within) = within {
        for a in &all {
            let held = containers::containers_of(conn, tenant, a.id)
                .await?
                .into_iter()
                .filter(|(_, k)| k == within)
                .map(|(c, _)| c.id)
                .collect();
            shared.insert(a.id, held);
        }
    }
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    let mut overtaken = Vec::new();
    for a in all.iter().filter(|a| !a.archived && !a.archive_kept) {
        let newer = all
            .iter()
            .filter(|b| b.id != a.id && b.state_since > a.state_since)
            .filter(|b| within.is_none() || !shared[&a.id].is_disjoint(&shared[&b.id]))
            .count();
        if newer >= count {
            overtaken.push(a.clone());
        }
    }
    let mut archived = 0;
    for a in &overtaken {
        if archive_by(conn, tenant, a, "newer", now).await? {
            archived += 1;
        }
    }
    Ok(archived)
}

/// Archives each live akt of the type `key` that has been in one of
/// `states` for at least `days` days. Answers how many it archived.
#[allow(dead_code, reason = "the sweep, PLAN-017-T05")]
pub(crate) async fn apply_days_in(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    key: &str,
    days: u32,
    states: &[String],
    now: DateTime<Utc>,
) -> Result<usize, AktError> {
    let since = now - Duration::days(i64::from(days));
    let mut archived = 0;
    for a in store::akter_in_states(conn, tenant, key, states, Archived::Live).await? {
        if a.state_since <= since
            && !a.archive_kept
            && archive_by(conn, tenant, &a, "days_in", now).await?
        {
            archived += 1;
        }
    }
    Ok(archived)
}
