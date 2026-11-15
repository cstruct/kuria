//! `release`, `versions` and `diff` (`version.*`): against a server, and
//! the reads in local mode too.

use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::file::{Akt, Released};
use crate::model::read;
use crate::output::Answer;
use crate::reads::Source;
use crate::session::Session;
use crate::values::file_value;

/// `kuria pdf <number> [--field] [--version] -o <file>` (`pdf.route`).
// r[impl pdf.route]
pub async fn pdf(
    session: &Session,
    number: &str,
    field: Option<&str>,
    version: Option<i64>,
    output: &std::path::Path,
) -> Result<Answer, CliError> {
    use crate::http::Body;
    let mut query = Vec::new();
    if let Some(field) = field {
        // Field keys are lowercase letters, digits and `_`.
        query.push(format!("field={field}"));
    }
    if let Some(version) = version {
        query.push(format!("version={version}"));
    }
    let url = format!("{}/akter/{number}/pdf?{}", session.url, query.join("&"));
    let response = session
        .http
        .send(
            reqwest::Method::GET,
            &url,
            Some(session.access_token()),
            &Body::Empty,
        )
        .await?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(CliError::from_server(
            "NOT_FOUND",
            format!("No PDF of {number}: no such akt, version or markdown field"),
            None,
        ));
    }
    if !status.is_success() {
        return Err(CliError::unavailable(format!("Kuria failed ({status})")));
    }
    let bytes = response.bytes().await?;
    std::fs::write(output, &bytes).map_err(|e| crate::profile::io_error(output, &e))?;
    Ok(Answer {
        markdown: format!("Wrote {}: {} bytes.", output.display(), bytes.len()),
        json: json!({ "number": number, "version": version, "path": output, "bytes": bytes.len() }),
    })
}

/// `kuria release <number> [--label]` against a server.
// r[impl version.release]
pub async fn release(
    session: &Session,
    number: &str,
    label: Option<&str>,
) -> Result<Answer, CliError> {
    let akt = crate::writes::akt(session, number).await?;
    let data = session
        .graphql(
            "mutation($n: String!, $v: Int!, $l: String) {
               releaseVersion(input: { number: $n, revision: $v, label: $l }) { number revision label } }",
            json!({ "n": number, "v": akt.revision, "l": label }),
        )
        .await?;
    let version = &data["releaseVersion"];
    let n = version["number"].as_i64().unwrap_or_default();
    let label = version["label"].as_str();
    Ok(Answer {
        markdown: format!(
            "{number}: released version {n}{} (revision {}).",
            label.map(|l| format!(", {l}")).unwrap_or_default(),
            akt.revision
        ),
        json: json!({ "number": number, "version": n, "revision": akt.revision, "label": label }),
    })
}

/// `kuria versions <number>`: its versions, latest first.
// r[impl version.list]
pub async fn list(source: Source<'_>, number: &str) -> Result<Answer, CliError> {
    let akt: Akt = read(
        crate::reads::akt_data(source, number, "", None).await?,
        "the akt",
    )?;
    let rows: Vec<Vec<String>> = akt.versions.iter().map(row).collect();
    let markdown = if rows.is_empty() {
        format!("{number} has no versions.")
    } else {
        crate::md::table(
            &["Version", "Revision", "Label", "Released by", "Released"],
            &rows,
        )
    };
    Ok(Answer {
        markdown,
        json: json!({
            "number": number,
            "versions": akt.versions.iter().map(|v| json!({
                "number": v.number,
                "revision": v.revision,
                "label": v.label,
                "releasedBy": v.released_by.as_ref().map(|u| u.email.clone().unwrap_or_else(|| u.name.clone())),
                "releasedAt": v.released_at,
            })).collect::<Vec<_>>(),
        }),
    })
}

fn row(v: &Released) -> Vec<String> {
    vec![
        v.number.to_string(),
        v.revision.to_string(),
        v.label.clone().unwrap_or_default(),
        v.released_by
            .as_ref()
            .map(|u| u.email.clone().unwrap_or_else(|| u.name.clone()))
            .unwrap_or_default(),
        v.released_at
            .as_deref()
            .map(crate::history::when)
            .unwrap_or_default(),
    ]
}

/// A field's value as `diff` compares it: its text in the akt file.
fn text(value: Option<&Json>) -> String {
    match value {
        None | Some(Json::Null) => String::new(),
        Some(Json::String(text)) => text.clone(),
        Some(other) => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// `kuria diff <number> <from> [<to>]`: what changed between two
/// versions, or a version and the living version, field by field, a
/// `markdown` field line by line.
// r[impl version.diff]
pub async fn diff(
    source: Source<'_>,
    number: &str,
    from: i64,
    to: Option<i64>,
) -> Result<Answer, CliError> {
    let read_at = async |version| -> Result<Akt, CliError> {
        read(
            crate::reads::akt_data(source, number, "", version).await?,
            "the akt",
        )
    };
    let before = read_at(Some(from)).await?;
    let after = read_at(to).await?;
    let values = |akt: &Akt| -> std::collections::BTreeMap<String, Json> {
        akt.fields
            .iter()
            .map(|v| (v.field().to_owned(), file_value(v)))
            .collect()
    };
    let (old, new) = (values(&before), values(&after));
    let mut markdown = format!(
        "# {number}: version {from} → {}\n",
        to.map_or_else(|| "now".to_owned(), |t| format!("version {t}"))
    );
    let mut changes = Vec::new();
    for field in &after.shape.fields {
        let (a, b) = (text(old.get(&field.key)), text(new.get(&field.key)));
        if a == b {
            continue;
        }
        let mut lines = String::new();
        for c in similar::TextDiff::from_lines(&a, &b).iter_all_changes() {
            lines.push(match c.tag() {
                similar::ChangeTag::Delete => '-',
                similar::ChangeTag::Insert => '+',
                similar::ChangeTag::Equal => ' ',
            });
            lines.push_str(c.value().trim_end_matches('\n'));
            lines.push('\n');
        }
        markdown.push_str(&format!("\n## {}\n\n```diff\n{lines}```\n", field.name));
        changes.push(json!({ "field": field.key, "from": a, "to": b }));
    }
    if changes.is_empty() {
        markdown.push_str("\nNothing changed.\n");
    }
    Ok(Answer {
        markdown,
        json: json!({ "number": number, "from": from, "to": to, "changes": changes }),
    })
}
