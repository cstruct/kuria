//! `copy`: an akt and its members, or part of them, as new akter of the
//! same types (`akt.copy`).

use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::output::Answer;
use crate::session::Session;

/// How many members a page of the copy's members holds.
const PAGE: u32 = 200;

/// What `copy` answers: the copy's number, and each copied member's number
/// with its original's, parents first.
// r[impl cli.write]
#[must_use]
pub fn answer(from: &str, copy: &str, members: &[(String, String)]) -> (String, Json) {
    let mut markdown = format!("Copied {from} to {copy}");
    match members.len() {
        0 => markdown.push('.'),
        1 => markdown.push_str(", with 1 member:\n"),
        n => markdown.push_str(&format!(", with {n} members:\n")),
    }
    for (original, copied) in members {
        markdown.push_str(&format!("\n- {original} → {copied}"));
    }
    (
        markdown,
        json!({
            "number": copy,
            "from": from,
            "members": members
                .iter()
                .map(|(original, copied)| json!({ "from": original, "number": copied }))
                .collect::<Vec<_>>(),
        }),
    )
}

/// `kuria copy <number> [--only <number>…] [--in <container>…]`.
// r[impl cli.write]
pub async fn copy(
    session: &Session,
    number: &str,
    only: &[String],
    inside: &[String],
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let data = session
        .graphql(
            "mutation($n: String!, $o: [String!], $i: [String!], $r: String) {
               copyAkt(input: { number: $n, only: $o, in: $i, reason: $r }) { number } }",
            json!({
                "n": number,
                "o": (!only.is_empty()).then_some(only),
                "i": (!inside.is_empty()).then_some(inside),
                "r": reason,
            }),
        )
        .await?;
    let copy = data["copyAkt"]["number"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    // A new akt's members are its copies' alone: each container's, parents
    // first, in their order.
    let mut members: Vec<(String, String)> = Vec::new();
    let mut containers = vec![copy.clone()];
    let mut at = 0;
    while at < containers.len() {
        let mut after: Option<String> = None;
        loop {
            let data = session
                .graphql(
                    &format!(
                        "query($n: String!, $after: String) {{ akt(number: $n) {{
                           members(first: {PAGE}, after: $after) {{
                             nodes {{ number copiedFrom {{ number }} }}
                             pageInfo {{ hasNextPage endCursor }} }} }} }}"
                    ),
                    json!({ "n": containers[at], "after": after }),
                )
                .await?;
            let page = &data["akt"]["members"];
            for node in page["nodes"].as_array().into_iter().flatten() {
                let copied = node["number"].as_str().unwrap_or_default().to_owned();
                let original = node["copiedFrom"]["number"]
                    .as_str()
                    .unwrap_or("?")
                    .to_owned();
                containers.push(copied.clone());
                members.push((original, copied));
            }
            if page["pageInfo"]["hasNextPage"] != true {
                break;
            }
            after = page["pageInfo"]["endCursor"].as_str().map(str::to_owned);
        }
        at += 1;
    }
    let (markdown, json) = answer(number, &copy, &members);
    Ok(Answer { markdown, json })
}
