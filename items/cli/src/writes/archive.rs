//! `archive` and `unarchive` (`cli.archive`): `archiveAkt` and
//! `unarchiveAkt` for each akt, in order, stopping at the first refused.

use serde_json::json;

use crate::error::CliError;
use crate::output::Answer;
use crate::session::Session;

/// What archiving or unarchiving `done`, each akt with the revision it
/// reached, answers; `archive` says which it was.
#[must_use]
pub fn answer(done: &[(String, i64)], archive: bool) -> Answer {
    let key = if archive { "archived" } else { "unarchived" };
    Answer {
        markdown: said(done, archive),
        json: json!({
            key: done
                .iter()
                .map(|(number, revision)| json!({ "number": number, "revision": revision }))
                .collect::<Vec<_>>(),
        }),
    }
}

fn said(done: &[(String, i64)], archive: bool) -> String {
    let verb = if archive { "Archived" } else { "Unarchived" };
    done.iter()
        .map(|(number, revision)| format!("{verb} {number} (revision {revision})."))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `refused`, after `done` were archived or unarchived, saying so first.
#[must_use]
pub fn after(done: &[(String, i64)], archive: bool, refused: CliError) -> CliError {
    if done.is_empty() {
        return refused;
    }
    CliError {
        message: format!("{}\n{}", said(done, archive), refused.message),
        ..refused
    }
}

/// `kuria archive <number>…` and, with `archive` false, `kuria unarchive
/// <number>…`, against a server.
// r[impl cli.archive]
pub async fn archive(
    session: &Session,
    numbers: &[String],
    reason: Option<&str>,
    archive: bool,
) -> Result<Answer, CliError> {
    let mutation = if archive {
        "archiveAkt"
    } else {
        "unarchiveAkt"
    };
    let mut done = Vec::new();
    for number in numbers {
        let answered = session
            .graphql(
                &format!(
                    "mutation($n: String!, $r: String) {{ {mutation}(input: {{ number: $n, reason: $r }}) {{ number revision }} }}"
                ),
                json!({ "n": number, "r": reason }),
            )
            .await;
        match answered {
            Ok(data) => done.push((
                number.clone(),
                data[mutation]["revision"].as_i64().unwrap_or_default(),
            )),
            Err(refused) => return Err(after(&done, archive, refused)),
        }
    }
    Ok(answer(&done, archive))
}

/// A reason given, as Kuria takes it: one line of at most 200
/// characters (`archive.act`).
pub fn reason_of(reason: Option<&str>) -> Result<Option<String>, CliError> {
    let reason = reason.map(str::trim).filter(|r| !r.is_empty());
    if reason.is_some_and(|r| r.chars().count() > 200 || r.contains(['\n', '\r'])) {
        return Err(CliError::refused(
            "BAD_REQUEST",
            "a reason is one line of at most 200 characters",
        ));
    }
    Ok(reason.map(str::to_owned))
}
