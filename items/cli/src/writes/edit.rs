//! `new`, `apply`, `set`, `do` and `draft`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value as Json, json};

use crate::error::{CliError, Kind};
use crate::file::{self, Akt, Parsed};
use crate::output::Answer;
use crate::profile::io_error;
use crate::session::Session;
use crate::values::{current_input, input_of, same_input};

use super::{akt, inputs, known_field, people_for, split_sets};

/// What `new` was given.
pub struct Created<'a> {
    pub akt_type: &'a str,
    pub inside: &'a [String],
    pub sets: &'a [String],
    pub file: Option<&'a Path>,
    pub reason: Option<&'a str>,
}

fn read_file(path: &Path) -> Result<Parsed, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    file::parse(&text).map_err(|e| CliError {
        message: format!("{}: {}", path.display(), e.message),
        ..e
    })
}

/// `kuria new <type>`: creates an akt from `--set`s, a file, or both, and
/// answers its number.
// r[impl cli.write]
// r[impl cli.apply]
pub async fn create(session: &Session, asked: &Created<'_>) -> Result<Answer, CliError> {
    let shape = crate::reads::shape(crate::reads::Source::Server(session), asked.akt_type).await?;
    let mut given = Map::new();
    let mut inside: Vec<String> = Vec::new();
    if let Some(path) = asked.file {
        let parsed = read_file(path)?;
        if let Some(key) = parsed.text("type").filter(|k| *k != shape.key) {
            return Err(CliError::refused(
                "BAD_FILE",
                format!("{} is a `{key}`, not a `{}`", path.display(), shape.key),
            ));
        }
        // A file's number, revision, state and links are another akt's.
        if let Some(Json::Array(containers)) = parsed.front.get("in") {
            inside.extend(
                containers
                    .iter()
                    .filter_map(Json::as_str)
                    .map(str::to_owned),
            );
        }
        given = parsed.values()?;
    }
    inside.extend(asked.inside.iter().cloned());
    given.extend(split_sets(&shape, asked.sets)?);
    let people = people_for(session, &shape, given.keys()).await?;
    let mut fields = inputs(&shape, &given, &people)?;
    fields.retain(|_, v| !v.is_null());
    let data = session
        .graphql(
            "mutation($t: String!, $f: JSON, $in: [String!]!, $r: String) {
               createAkt(input: { type: $t, fields: $f, in: $in, reason: $r }) { number revision } }",
            json!({ "t": shape.key, "f": fields, "in": inside, "r": asked.reason }),
        )
        .await?;
    let number = data["createAkt"]["number"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    Ok(Answer {
        markdown: format!("Created {number}."),
        json: json!({ "number": number, "revision": data["createAkt"]["revision"] }),
    })
}

/// What a file changes of `akt`: each field whose value differs from the
/// akt's, by its input; a field the file leaves out is cleared.
async fn changes(
    session: &Session,
    akt: &Akt,
    parsed: &Parsed,
) -> Result<Map<String, Json>, CliError> {
    let shape = &akt.shape;
    let given = parsed.values()?;
    for key in given.keys() {
        known_field(shape, key)?;
    }
    let people = people_for(session, shape, given.keys()).await?;
    let mut changed = Map::new();
    for field in &shape.fields {
        let wanted = input_of(field, given.get(&field.key).unwrap_or(&Json::Null), &people)?;
        let now = akt
            .fields
            .iter()
            .find(|v| v.field() == field.key)
            .map_or(Json::Null, current_input);
        if !same_input(&wanted, &now) {
            changed.insert(field.key.clone(), wanted);
        }
    }
    Ok(changed)
}

/// Refuses a file whose read-only keys differ from the akt's.
fn check_read_only(akt: &Akt, parsed: &Parsed) -> Result<(), CliError> {
    let read_only = |key: &str, how: &str| CliError {
        field: Some(key.to_owned()),
        ..CliError::refused(
            "READ_ONLY",
            format!(
                "`{key}` can't be changed in a file of {}: {how}",
                akt.number
            ),
        )
    };
    if parsed.text("type").is_some_and(|t| t != akt.shape.key) {
        return Err(read_only("type", "an akt keeps its type"));
    }
    if parsed.text("state").is_some_and(|s| s != akt.state.key) {
        return Err(read_only("state", "perform a transition with `kuria do`"));
    }
    let listed = |key: &str| -> BTreeSet<String> {
        parsed.front[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
            .map(str::to_owned)
            .collect()
    };
    let containers: BTreeSet<String> = akt.containers.iter().map(|c| c.number.clone()).collect();
    if parsed.front.contains_key("in") && listed("in") != containers {
        return Err(read_only("in", "use `kuria add`, `remove` or `transfer`"));
    }
    let links: BTreeSet<String> = akt.links.iter().map(file::Link::said).collect();
    if parsed.front.contains_key("links") && listed("links") != links {
        return Err(read_only("links", "use `kuria link` or `unlink`"));
    }
    Ok(())
}

/// A `STALE` refusal saying what changed since `revision`.
async fn stale(session: &Session, akt: &Akt, revision: i64) -> CliError {
    let since = session
        .graphql(
            "query($n: String!) { akt(number: $n) { revisions { revision transition changes membership } } }",
            json!({ "n": akt.number }),
        )
        .await;
    let mut changed: Vec<String> = Vec::new();
    let mut note = |what: String| {
        if !changed.contains(&what) {
            changed.push(what);
        }
    };
    if let Ok(data) = since {
        for r in data["akt"]["revisions"].as_array().into_iter().flatten() {
            if r["revision"].as_i64().unwrap_or_default() <= revision {
                continue;
            }
            for key in r["changes"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, _)| k)
            {
                note(akt.shape.field_name(key).to_owned());
            }
            if r["transition"].is_string() {
                note("its state".to_owned());
            }
            if r["membership"].is_object() {
                note("its containers, members or links".to_owned());
            }
        }
    }
    CliError::new(
        Kind::Stale,
        "STALE",
        format!(
            "{} is at revision {}, the file at {revision}; since then {} changed. Merge the file with `kuria cat {}` and apply it again",
            akt.number,
            akt.revision,
            if changed.is_empty() {
                "something".to_owned()
            } else {
                changed.join(", ")
            },
            akt.number
        ),
    )
}

/// Every akt the files at `paths` hold, each with its file, its number and
/// the revision it was read at; an akt given twice is refused.
// r[impl cli.apply]
// r[impl cli.file.multi]
pub(crate) fn read_akter(
    paths: &[PathBuf],
) -> Result<Vec<(PathBuf, String, i64, Parsed)>, CliError> {
    let mut akter: Vec<(PathBuf, String, i64, Parsed)> = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
        let in_file = |e: CliError| CliError {
            message: format!("{}: {}", path.display(), e.message),
            ..e
        };
        for (number, parsed) in file::parse_all(&text).map_err(in_file)? {
            let revision = parsed
                .front
                .get("revision")
                .and_then(Json::as_i64)
                .ok_or_else(|| {
                    CliError::refused(
                        "BAD_FILE",
                        format!(
                            "{}: {number} names no `revision` it was read at",
                            path.display()
                        ),
                    )
                })?;
            if let Some((other, ..)) = akter.iter().find(|(_, n, ..)| *n == number) {
                return Err(CliError::refused(
                    "BAD_FILE",
                    format!(
                        "{number} is given twice, in {} and {}",
                        other.display(),
                        path.display()
                    ),
                ));
            }
            akter.push((path.clone(), number, revision, parsed));
        }
    }
    Ok(akter)
}

/// `kuria apply <file>…`: edits each akt its files hold with the fields
/// the file changed, after checking every one, refusing read-only keys and
/// stale akter before changing any.
// r[impl cli.apply]
pub async fn apply(
    session: &Session,
    paths: &[PathBuf],
    dry_run: bool,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    // Every akt checked first, so that a refusal changes nothing.
    let mut checked = Vec::new();
    for (path, number, revision, parsed) in read_akter(paths)? {
        let akt = akt(session, &number).await?;
        if akt.revision != revision {
            return Err(stale(session, &akt, revision).await);
        }
        check_read_only(&akt, &parsed)?;
        let changed = changes(session, &akt, &parsed).await?;
        checked.push((path, number, revision, akt, changed));
    }
    let mut lines = Vec::new();
    let mut applied = Vec::new();
    for (path, number, revision, akt, changed) in checked {
        let names: Vec<&str> = changed.keys().map(|k| akt.shape.field_name(k)).collect();
        if changed.is_empty() {
            lines.push(format!("{number}: nothing changed."));
            applied.push(
                json!({ "file": path, "number": number, "revision": akt.revision, "changed": [] }),
            );
            continue;
        }
        if dry_run {
            lines.push(format!("{number}: would change {}.", names.join(", ")));
            applied.push(json!({ "file": path, "number": number, "revision": akt.revision, "changed": changed.keys().collect::<Vec<_>>(), "dryRun": true }));
            continue;
        }
        let data = session
            .graphql(
                "mutation($n: String!, $v: Int!, $f: JSON!, $r: String) {
                   editAkt(input: { number: $n, revision: $v, fields: $f, reason: $r }) { revision } }",
                json!({ "n": number, "v": revision, "f": changed, "r": reason }),
            )
            .await?;
        let now = data["editAkt"]["revision"].as_i64().unwrap_or_default();
        lines.push(format!(
            "{number}: changed {} (revision {now}).",
            names.join(", ")
        ));
        applied.push(json!({ "file": path, "number": number, "revision": now, "changed": changed.keys().collect::<Vec<_>>() }));
    }
    Ok(Answer {
        markdown: lines.join("\n"),
        json: json!({ "applied": applied }),
    })
}

/// `kuria set <number> <field>=<value>…`: an edit without a file.
// r[impl cli.write]
pub async fn set(
    session: &Session,
    number: &str,
    sets: &[String],
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let akt = akt(session, number).await?;
    let given = split_sets(&akt.shape, sets)?;
    let people = people_for(session, &akt.shape, given.keys()).await?;
    let fields = inputs(&akt.shape, &given, &people)?;
    let data = session
        .graphql(
            "mutation($n: String!, $v: Int!, $f: JSON!, $r: String) {
               editAkt(input: { number: $n, revision: $v, fields: $f, reason: $r }) { revision } }",
            json!({ "n": number, "v": akt.revision, "f": fields, "r": reason }),
        )
        .await?;
    let names: Vec<&str> = fields.keys().map(|k| akt.shape.field_name(k)).collect();
    let revision = &data["editAkt"]["revision"];
    Ok(Answer {
        markdown: format!("{number}: set {} (revision {revision}).", names.join(", ")),
        json: json!({ "number": number, "revision": revision, "changed": fields.keys().collect::<Vec<_>>() }),
    })
}

/// `kuria do <number> <transition>`: a transition, by key or name, with the
/// fields it requires.
// r[impl cli.write]
pub async fn perform(
    session: &Session,
    number: &str,
    transition: &str,
    sets: &[String],
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let akt = akt(session, number).await?;
    let shape = &akt.shape;
    let chosen = shape
        .transitions
        .iter()
        .find(|t| t.key == transition)
        .or_else(|| {
            shape
                .transitions
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(transition))
        })
        .ok_or_else(|| {
            let allowed: Vec<String> = shape
                .transitions
                .iter()
                .filter(|t| t.from.contains(&akt.state.key))
                .map(|t| format!("`{}` ({})", t.key, t.name))
                .collect();
            CliError::refused(
                "UNKNOWN_TRANSITION",
                format!(
                    "{} has no transition `{transition}`; from {} it can take {}",
                    shape.name,
                    akt.state.name,
                    if allowed.is_empty() {
                        "none".to_owned()
                    } else {
                        allowed.join(", ")
                    }
                ),
            )
        })?;
    let given = split_sets(shape, sets)?;
    let people = people_for(session, shape, given.keys()).await?;
    let fields = inputs(shape, &given, &people)?;
    let data = session
        .graphql(
            "mutation($n: String!, $v: Int!, $t: String!, $f: JSON, $r: String) {
               performTransition(input: { number: $n, revision: $v, transition: $t, fields: $f, reason: $r }) {
                 revision state { key name } } }",
            json!({ "n": number, "v": akt.revision, "t": chosen.key, "f": fields, "r": reason }),
        )
        .await?;
    let done = &data["performTransition"];
    Ok(Answer {
        markdown: format!(
            "{number}: {} → {} (revision {}).",
            chosen.name,
            done["state"]["name"].as_str().unwrap_or_default(),
            done["revision"]
        ),
        json: json!({ "number": number, "revision": done["revision"], "state": done["state"]["key"], "transition": chosen.key }),
    })
}

/// `kuria draft <number> <field>`: fills a field from its recipe.
// r[impl cli.write]
pub async fn draft(
    session: &Session,
    number: &str,
    field: &str,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let akt = akt(session, number).await?;
    known_field(&akt.shape, field)?;
    let data = session
        .graphql(
            "mutation($n: String!, $v: Int!, $f: String!, $r: String) {
               draftField(input: { number: $n, revision: $v, field: $f, reason: $r }) { revision } }",
            json!({ "n": number, "v": akt.revision, "f": field, "r": reason }),
        )
        .await?;
    let revision = &data["draftField"]["revision"];
    Ok(Answer {
        markdown: format!(
            "{number}: drafted {} (revision {revision}); see it with `kuria show {number}`.",
            akt.shape.field_name(field)
        ),
        json: json!({ "number": number, "revision": revision, "field": field }),
    })
}
