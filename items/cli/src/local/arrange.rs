//! `add`, `remove`, `transfer`, `move`, `link` and `unlink` in local mode,
//! through the engine's rules (`cli.local.rules`). Containment is kept in
//! each member's `in`, and a link in both akter's `links`.

use kuria_engine::placement;
use kuria_engine::rules::{Broken, containment, links};
use serde_json::json;

use super::dir::{AktDir, LocalAkt};
use super::write::{Writer, answer};
use crate::error::CliError;
use crate::output::Answer;
use crate::writes::Membership;

/// What the directory knows of `member` joining `container`, leaving out
/// `leaving`.
fn joining<'a>(
    dir: &'a AktDir,
    container: &str,
    member: &LocalAkt,
    leaving: Option<&str>,
) -> containment::Joining<'a> {
    let key = &dir.akt(container).expect("found").type_key;
    containment::Joining {
        already: member.containers.iter().any(|c| c == container),
        other_of_type: dir
            .akter
            .iter()
            .find(|c| {
                c.number != container
                    && Some(c.number.as_str()) != leaving
                    && c.type_key == *key
                    && member.containers.contains(&c.number)
            })
            .map(|c| c.number.as_str()),
        contains_container: containment::contains(&member.number, container, |akt| {
            dir.members_of(akt).map(|m| m.number.clone()).collect()
        }),
    }
}

fn check_joins(
    w: &Writer,
    container: &str,
    member: &str,
    leaving: Option<&str>,
) -> Result<(), CliError> {
    let m = w.akt(member)?;
    let known = joining(&w.dir, container, m, leaving);
    containment::check_joining(&w.akt_ref(container)?, &w.akt_ref(member)?, known)?;
    Ok(())
}

/// `kuria members add` and `members remove` in local mode.
// r[impl cli.local.rules]
// r[impl cli.local.revision]
pub fn membership(
    w: &mut Writer,
    change: Membership,
    container: &str,
    members: &[String],
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let mut lines = Vec::new();
    for member in members {
        containment::check_not_itself(&[container], member)?;
        w.akt(member)?;
        containment::check_unfrozen(&w.akt_ref(container)?)?;
        match change {
            Membership::Add => {
                check_joins(w, container, member, None)?;
                w.akt_mut(member).containers.push(container.to_owned());
                lines.push(format!("Added {member} to {container}."));
            }
            Membership::Remove => {
                let m = w.akt_mut(member);
                let Some(i) = m.containers.iter().position(|c| c == container) else {
                    return Err(Broken::NotMember {
                        container: container.to_owned(),
                        member: member.clone(),
                    }
                    .into());
                };
                m.containers.remove(i);
                w.leave_sections(container, member)?;
                lines.push(format!("Removed {member} from {container}."));
            }
        }
        w.bump(member);
        w.bump(container);
    }
    let said = if change == Membership::Add {
        "added"
    } else {
        "removed"
    };
    Ok(answer(
        lines.join("\n"),
        json!({ "container": container, "members": members, "change": said }),
        reason,
    ))
}

/// `kuria members transfer <member> <from> <to>` in local mode: one change.
pub fn transfer(
    w: &mut Writer,
    member: &str,
    from: &str,
    to: &str,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    if from == to {
        return Err(Broken::AlreadyMember {
            container: to.to_owned(),
            member: member.to_owned(),
        }
        .into());
    }
    containment::check_not_itself(&[from, to], member)?;
    w.akt(member)?;
    containment::check_unfrozen(&w.akt_ref(from)?)?;
    containment::check_unfrozen(&w.akt_ref(to)?)?;
    if !w.akt(member)?.containers.iter().any(|c| c == from) {
        return Err(Broken::NotMember {
            container: from.to_owned(),
            member: member.to_owned(),
        }
        .into());
    }
    // The membership being ended doesn't count against exclusivity.
    check_joins(w, to, member, Some(from))?;
    let m = w.akt_mut(member);
    m.containers.retain(|c| c != from);
    m.containers.push(to.to_owned());
    w.leave_sections(from, member)?;
    for number in [member, from, to] {
        w.bump(number);
    }
    Ok(answer(
        format!("Transferred {member} from {from} to {to}."),
        json!({ "member": member, "from": from, "to": to }),
        reason,
    ))
}

/// `kuria members place <number> --before|--after <number>` in local mode: its
/// place in the directory's order, which no revision records.
pub fn move_akt(
    w: &mut Writer,
    number: &str,
    before: Option<&str>,
    after: Option<&str>,
) -> Result<Answer, CliError> {
    let neighbour = before.or(after).unwrap_or_default();
    if neighbour == number {
        return Err(Broken::SelfMove.into());
    }
    let own = w.akt(number)?.rank.clone();
    let next_to = w.akt(neighbour)?.rank.clone();
    let mut ranks: Vec<&str> = w.dir.akter.iter().map(|a| a.rank.as_str()).collect();
    ranks.sort_unstable();
    let (low, high) = if before.is_some() {
        (
            ranks.iter().rev().find(|r| **r < next_to.as_str()).copied(),
            Some(next_to.as_str()),
        )
    } else {
        (
            Some(next_to.as_str()),
            ranks.iter().find(|r| **r > next_to.as_str()).copied(),
        )
    };
    let moved =
        placement(&own, low, high).map_err(|e| CliError::refused("INTERNAL", e.to_string()))?;
    if let Some(rank) = moved {
        w.akt_mut(number).rank = rank;
    }
    let place = match (before, after) {
        (Some(other), _) => format!("before {other}"),
        (_, Some(other)) => format!("after {other}"),
        _ => String::new(),
    };
    Ok(answer(
        format!("Moved {number} {place}."),
        json!({ "number": number, "before": before, "after": after }),
        None,
    ))
}

/// The key of the kind `kind` names on `source`'s type, by key or name.
fn kind_key(w: &Writer, source: &str, kind: &str) -> Result<Option<String>, CliError> {
    let akt = w.akt(source)?;
    let links = &w.dir.type_of(akt).ty.document().links;
    Ok(links
        .iter()
        .find(|l| l.key == kind || l.name.eq_ignore_ascii_case(kind))
        .map(|l| l.key.clone()))
}

/// Whether `said`, as a file lists a link, is `name` to `number`.
fn is_link(said: &str, name: &str, number: &str) -> bool {
    said.rsplit_once(' ').is_some_and(|(n, target)| {
        n == name && target.split_once('@').map_or(target, |(t, _)| t) == number
    })
}

/// `kuria link <source> <kind> <target> [--pin]` in local mode.
// r[impl cli.local.rules]
// r[impl cli.local.revision]
pub fn link(
    w: &mut Writer,
    source: &str,
    kind: &str,
    target: &str,
    pin: bool,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let pinned = if pin {
        Some(w.akt(target)?.revision)
    } else {
        None
    };
    let key = add_link(w, source, kind, target, pinned)?;
    let said = pinned
        .map(|r| format!(" at revision {r}"))
        .unwrap_or_default();
    Ok(answer(
        format!("Linked {source} {key} {target}{said}."),
        json!({ "source": source, "kind": key, "target": target, "pinnedRevision": pinned }),
        reason,
    ))
}

/// Links `source` to `target` by `kind`, pinned at `pinned` if given, on
/// both akter's files, and answers the kind's key.
pub(crate) fn add_link(
    w: &mut Writer,
    source: &str,
    kind: &str,
    target: &str,
    pinned: Option<i32>,
) -> Result<String, CliError> {
    links::check_not_itself(source, target)?;
    let key = kind_key(w, source, kind)?.unwrap_or_else(|| kind.to_owned());
    let def = links::to_add(&w.akt_ref(source)?, &key, &w.akt_ref(target)?, pinned)?.clone();
    let new = !w
        .akt(source)?
        .links
        .iter()
        .any(|l| is_link(l, &def.name, target));
    links::check_new(new, source, &key, target)?;
    let at = pinned.map(|r| format!("@{r}")).unwrap_or_default();
    w.akt_mut(source)
        .links
        .push(format!("{} {target}{at}", def.name));
    w.akt_mut(target)
        .links
        .push(format!("{} {source}{at}", def.inverse));
    w.bump(source);
    w.bump(target);
    Ok(key)
}

/// `kuria unlink <source> <kind> <target>` in local mode.
pub fn unlink(
    w: &mut Writer,
    source: &str,
    kind: &str,
    target: &str,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    w.akt(target)?;
    let key = kind_key(w, source, kind)?.unwrap_or_else(|| kind.to_owned());
    let def = links::to_remove(&w.akt_ref(source)?, &key)?;
    let existed = w
        .akt(source)?
        .links
        .iter()
        .any(|l| is_link(l, &def.name, target));
    links::check_existed(existed, source, &key, target)?;
    w.akt_mut(source)
        .links
        .retain(|l| !is_link(l, &def.name, target));
    w.akt_mut(target)
        .links
        .retain(|l| !is_link(l, &def.inverse, source));
    w.bump(source);
    w.bump(target);
    Ok(answer(
        format!("Unlinked {source} {key} {target}."),
        json!({ "source": source, "kind": key, "target": target }),
        reason,
    ))
}
