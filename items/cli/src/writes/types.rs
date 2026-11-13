//! `types export` and `types define`: akt type documents out and in.

use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use crate::error::CliError;
use crate::output::Answer;
use crate::profile::io_error;
use crate::session::Session;

/// `kuria types export [<key>…] [-o <dir>]`: the documents the akt types'
/// latest versions were defined by, all of them unless keys are named.
// r[impl cli.write]
// r[impl cli.local.types]
pub async fn export(
    source: crate::reads::Source<'_>,
    keys: &[String],
    output: Option<&Path>,
) -> Result<Answer, CliError> {
    let session = match source {
        crate::reads::Source::Server(session) => session,
        crate::reads::Source::Local(l) => {
            let mut documents = Vec::new();
            for key in keys {
                if !l.dir.types.contains_key(key) {
                    return Err(CliError::from_server(
                        "UNKNOWN_AKT_TYPE",
                        format!("No akt type `{key}`; see `kuria types`"),
                        None,
                    ));
                }
            }
            for (key, t) in &l.dir.types {
                if keys.is_empty() || keys.contains(key) {
                    documents.push(t.document.clone());
                }
            }
            return answer(&documents, output);
        }
    };
    let documents: Vec<Json> = if keys.is_empty() {
        let data = session
            .graphql("{ aktTypes { document } }", json!({}))
            .await?;
        data["aktTypes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| t["document"].clone())
            .collect()
    } else {
        let mut documents = Vec::new();
        for key in keys {
            let data = session
                .graphql(
                    "query($k: String!) { aktType(key: $k) { document } }",
                    json!({ "k": key }),
                )
                .await?;
            if data["aktType"].is_null() {
                return Err(CliError::from_server(
                    "UNKNOWN_AKT_TYPE",
                    format!("No akt type `{key}`; see `kuria types`"),
                    None,
                ));
            }
            documents.push(data["aktType"]["document"].clone());
        }
        documents
    };
    answer(&documents, output)
}

/// The documents, written to `output` or answered.
fn answer(documents: &[Json], output: Option<&Path>) -> Result<Answer, CliError> {
    let pretty = |d: &Json| serde_json::to_string_pretty(d).expect("serializable");
    let key = |d: &Json| d["key"].as_str().unwrap_or("type").to_owned();
    if let Some(dir) = output {
        std::fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
        let mut written = Vec::new();
        for document in documents {
            let path = dir.join(format!("{}.json", key(document)));
            std::fs::write(&path, pretty(document) + "\n").map_err(|e| io_error(&path, &e))?;
            written.push(path.display().to_string());
        }
        return Ok(Answer {
            markdown: written
                .iter()
                .map(|p| format!("Wrote {p}"))
                .collect::<Vec<_>>()
                .join("\n"),
            json: json!({ "written": written }),
        });
    }
    Ok(Answer {
        markdown: documents
            .iter()
            .map(|d| format!("## `{}`\n\n```json\n{}\n```\n", key(d), pretty(d)))
            .collect::<Vec<_>>()
            .join("\n"),
        json: json!({ "types": documents }),
    })
}

/// The documents in `path`: one, or a list of them.
fn documents(path: &Path) -> Result<Vec<Json>, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    match serde_json::from_str(&text) {
        Ok(Json::Array(documents)) => Ok(documents),
        Ok(document) => Ok(vec![document]),
        Err(e) => Err(CliError::refused(
            "INVALID_AKT_TYPE",
            format!("{} isn't JSON: {e}", path.display()),
        )),
    }
}

/// What's wrong with `document` on its own, as `kuria-config` checks it.
fn problems(document: &Json) -> Vec<String> {
    match serde_json::from_value::<kuria_config::AktTypeDocument>(document.clone()) {
        Err(e) => vec![e.to_string()],
        Ok(parsed) => match kuria_config::validate(parsed) {
            Ok(_) => Vec::new(),
            Err(problems) => problems.iter().map(ToString::to_string).collect(),
        },
    }
}

/// `kuria types define <file>…`: defines each document as its type's next
/// version; with `check`, only validates them, offline.
// r[impl cli.write]
pub async fn define(
    session: Option<&Session>,
    paths: &[PathBuf],
    check: bool,
) -> Result<Answer, CliError> {
    let mut found = Vec::new();
    for path in paths {
        for document in documents(path)? {
            found.push((path, document));
        }
    }
    let wrong: Vec<String> = found
        .iter()
        .flat_map(|(path, document)| {
            let key = document["key"].as_str().unwrap_or("?").to_owned();
            problems(document)
                .into_iter()
                .map(move |p| format!("{} (`{key}`): {p}", path.display()))
        })
        .collect();
    if !wrong.is_empty() {
        return Err(CliError::refused(
            "INVALID_AKT_TYPE",
            format!("Not valid:\n- {}", wrong.join("\n- ")),
        ));
    }
    let Some(session) = session.filter(|_| !check) else {
        let keys: Vec<&str> = found
            .iter()
            .filter_map(|(_, d)| d["key"].as_str())
            .collect();
        return Ok(Answer {
            markdown: format!(
                "Valid on their own: {}. Kuria also checks what they name of other akt types when they are defined.",
                keys.join(", ")
            ),
            json: json!({ "valid": keys }),
        });
    };
    let mut defined = Vec::new();
    for (_, document) in &found {
        let data = session
            .graphql(
                "mutation($d: JSON!) { defineAktType(document: $d) { key version } }",
                json!({ "d": document }),
            )
            .await?;
        defined.push(data["defineAktType"].clone());
    }
    Ok(Answer {
        markdown: defined
            .iter()
            .map(|d| {
                format!(
                    "Defined `{}`, version {}.",
                    d["key"].as_str().unwrap_or_default(),
                    d["version"]
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        json: json!({ "defined": defined }),
    })
}
