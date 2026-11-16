//! `add`, `remove`, `transfer`, `move`, `link` and `unlink`.

use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::output::Answer;
use crate::session::Session;

use super::akt;

/// Which way membership changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Membership {
    Add,
    Remove,
}

/// `kuria members add` and `members remove`: members in or out of a container, one
/// change each.
// r[impl cli.write]
pub async fn membership(
    session: &Session,
    change: Membership,
    container: &str,
    members: &[String],
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let (mutation, done) = match change {
        Membership::Add => ("addToContainer", "Added"),
        Membership::Remove => ("removeFromContainer", "Removed"),
    };
    let mut lines = Vec::new();
    for member in members {
        session
            .graphql(
                &format!(
                    "mutation($c: String!, $m: String!, $r: String) {{
                       {mutation}(input: {{ container: $c, member: $m, reason: $r }}) {{ number }} }}"
                ),
                json!({ "c": container, "m": member, "r": reason }),
            )
            .await?;
        lines.push(format!(
            "{done} {member} {} {container}.",
            if change == Membership::Add {
                "to"
            } else {
                "from"
            }
        ));
    }
    Ok(Answer {
        markdown: lines.join("\n"),
        json: json!({ "container": container, "members": members, "change": if change == Membership::Add { "added" } else { "removed" } }),
    })
}

/// `kuria members transfer <member> <from> <to>`: one change.
// r[impl cli.write]
pub async fn transfer(
    session: &Session,
    member: &str,
    from: &str,
    to: &str,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    session
        .graphql(
            "mutation($m: String!, $f: String!, $t: String!, $r: String) {
               transferMember(input: { member: $m, from: $f, to: $t, reason: $r }) { number } }",
            json!({ "m": member, "f": from, "t": to, "r": reason }),
        )
        .await?;
    Ok(Answer {
        markdown: format!("Transferred {member} from {from} to {to}."),
        json: json!({ "member": member, "from": from, "to": to }),
    })
}

/// `kuria members place <number> --before|--after <number>`: its place in the
/// tenant's order, on every board.
// r[impl cli.write]
pub async fn move_akt(
    session: &Session,
    number: &str,
    before: Option<&str>,
    after: Option<&str>,
) -> Result<Answer, CliError> {
    session
        .graphql(
            "mutation($n: String!, $b: String, $a: String) {
               moveAkt(input: { number: $n, before: $b, after: $a }) { number } }",
            json!({ "n": number, "b": before, "a": after }),
        )
        .await?;
    let place = match (before, after) {
        (Some(other), _) => format!("before {other}"),
        (_, Some(other)) => format!("after {other}"),
        _ => String::new(),
    };
    Ok(Answer {
        markdown: format!("Moved {number} {place}."),
        json: json!({ "number": number, "before": before, "after": after }),
    })
}

/// The key of the link kind `kind` names on `source`'s type, by key or
/// name.
async fn kind_key(session: &Session, source: &str, kind: &str) -> Result<String, CliError> {
    let akt = akt(session, source).await?;
    let links = akt.shape.document["links"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let named = |l: &&Json| {
        l["key"].as_str() == Some(kind)
            || l["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(kind))
    };
    match links.iter().find(named) {
        Some(link) => Ok(link["key"].as_str().unwrap_or(kind).to_owned()),
        None => Err(CliError::refused(
            "INVALID_LINK",
            format!(
                "{} links by {}, not `{kind}`",
                akt.shape.name,
                if links.is_empty() {
                    "no kind".to_owned()
                } else {
                    links
                        .iter()
                        .map(|l| format!("`{}`", l["key"].as_str().unwrap_or_default()))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
        )),
    }
}

/// `kuria link <source> <kind> <target>`, with `--pin` pinning the target
/// as it is now.
// r[impl cli.write]
pub async fn link(
    session: &Session,
    source: &str,
    kind: &str,
    target: &str,
    pin: bool,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let kind = kind_key(session, source, kind).await?;
    let pinned = if pin {
        Some(akt(session, target).await?.revision)
    } else {
        None
    };
    session
        .graphql(
            "mutation($s: String!, $k: String!, $t: String!, $p: Int, $r: String) {
               addLink(input: { source: $s, kind: $k, target: $t, pinRevision: $p, reason: $r }) { number } }",
            json!({ "s": source, "k": kind, "t": target, "p": pinned, "r": reason }),
        )
        .await?;
    let at = pinned
        .map(|r| format!(" at revision {r}"))
        .unwrap_or_default();
    Ok(Answer {
        markdown: format!("Linked {source} {kind} {target}{at}."),
        json: json!({ "source": source, "kind": kind, "target": target, "pinnedRevision": pinned }),
    })
}

/// `kuria unlink <source> <kind> <target>`.
// r[impl cli.write]
pub async fn unlink(
    session: &Session,
    source: &str,
    kind: &str,
    target: &str,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    // A kind a newer version of the type dropped still unlinks by its key.
    let kind = match kind_key(session, source, kind).await {
        Err(error) if error.code == "INVALID_LINK" => kind.to_owned(),
        other => other?,
    };
    session
        .graphql(
            "mutation($s: String!, $k: String!, $t: String!, $r: String) {
               removeLink(input: { source: $s, kind: $k, target: $t, reason: $r }) { number } }",
            json!({ "s": source, "k": kind, "t": target, "r": reason }),
        )
        .await?;
    Ok(Answer {
        markdown: format!("Unlinked {source} {kind} {target}."),
        json: json!({ "source": source, "kind": kind, "target": target }),
    })
}
