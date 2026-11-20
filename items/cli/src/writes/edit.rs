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
#[derive(Default)]
pub struct Created<'a> {
    pub akt_type: &'a str,
    pub inside: &'a [String],
    pub sets: &'a [String],
    pub file: Option<&'a Path>,
    /// Its number, if it is given one (`akt.number.given`).
    pub number: Option<&'a str>,
    /// The links it is created with (`link.create`).
    pub links: &'a [NewLink],
    /// Whether it starts with the values of the akter it is linked with
    /// (`link.create.carry`).
    pub carry: bool,
    pub reason: Option<&'a str>,
}

/// A link a new akt is created with: of `kind`, with the akt `other`, from
/// the new akt if `outgoing`, else to it (`link.create`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewLink {
    pub kind: String,
    pub other: String,
    pub outgoing: bool,
}

/// The links `--link` and `--linked-from` ask for, each `<kind>:<number>`.
// r[impl link.create]
pub fn linked(links: &[String], linked_from: &[String]) -> Result<Vec<NewLink>, CliError> {
    let read = |text: &String, outgoing| {
        let (kind, other) = text.split_once(':').ok_or_else(|| {
            CliError::refused("BAD_REQUEST", format!("`{text}` isn't `<kind>:<number>`"))
        })?;
        Ok(NewLink {
            kind: kind.to_owned(),
            other: other.to_owned(),
            outgoing,
        })
    };
    links
        .iter()
        .map(|t| read(t, true))
        .chain(linked_from.iter().map(|t| read(t, false)))
        .collect()
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
    // r[impl link.create.carry]
    if asked.carry {
        for link in asked.links {
            let other = super::akt(session, &link.other).await?;
            for field in &shape.fields {
                let carried = other
                    .shape
                    .fields
                    .iter()
                    .any(|f| f.key == field.key && f.kind == field.kind)
                    && field.kind != "VERDICT"
                    && field.kind != "SECTIONS";
                if !carried || fields.contains_key(&field.key) {
                    continue;
                }
                if let Some(value) = other.fields.iter().find(|v| v.field() == field.key) {
                    fields.insert(field.key.clone(), crate::values::current_input(value));
                }
            }
        }
    }
    fields.retain(|_, v| !v.is_null());
    // r[impl link.create]
    let links: Vec<Json> = asked
        .links
        .iter()
        .map(|l| {
            if l.outgoing {
                json!({ "kind": l.kind, "to": l.other })
            } else {
                json!({ "kind": l.kind, "from": l.other })
            }
        })
        .collect();
    let data = session
        .graphql(
            "mutation($t: String!, $f: JSON, $in: [String!]!, $r: String, $n: String, $l: [NewLinkInput!]) {
               createAkt(input: { type: $t, fields: $f, in: $in, reason: $r, number: $n, links: $l }) { number revision } }",
            json!({ "t": shape.key, "f": fields, "in": inside, "r": asked.reason, "n": asked.number, "l": links }),
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
pub(crate) async fn changes(
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
        let now = akt.fields.iter().find(|v| v.field() == field.key);
        if field.kind == "VERDICT" {
            let now = now.map_or_else(|| json!({}), crate::values::file_value);
            verdicts_unchanged(&akt.number, field, given.get(&field.key), &now)?;
            continue;
        }
        let wanted = input_of(field, given.get(&field.key).unwrap_or(&Json::Null), &people)?;
        let now = now.map_or(Json::Null, current_input);
        if !same_input(&wanted, &now) {
            changed.insert(field.key.clone(), wanted);
        }
    }
    Ok(changed)
}

/// Refuses a file whose verdicts, `given` if it gives them, differ from
/// those the akt has, `now`, in the file's form: they are given with
/// `kuria verdict` (`cli.file.kinds`).
// r[impl cli.file.kinds]
pub(crate) fn verdicts_unchanged(
    number: &str,
    field: &crate::model::FieldDef,
    given: Option<&Json>,
    now: &Json,
) -> Result<(), CliError> {
    match given {
        Some(given) if !(given == now || given.is_null() && now == &json!({})) => Err(CliError {
            field: Some(field.key.clone()),
            ..CliError::refused(
                "READ_ONLY",
                format!(
                    "`{}` can't be changed in a file of {number}: give a verdict with `kuria verdict`",
                    field.key
                ),
            )
        }),
        _ => Ok(()),
    }
}

/// Refuses a file whose `outcome` differs from `now`'s: it follows from
/// verdicts (`akt.outcome.derived`).
pub(crate) fn outcome_unchanged(
    number: &str,
    parsed: &Parsed,
    now: Option<&str>,
) -> Result<(), CliError> {
    let given = parsed.text("outcome");
    if given.is_some() && given.as_deref() != now {
        return Err(CliError {
            field: Some("outcome".to_owned()),
            ..CliError::refused(
                "READ_ONLY",
                format!(
                    "`outcome` can't be changed in a file of {number}: it follows from verdicts"
                ),
            )
        });
    }
    Ok(())
}

/// Refuses a file whose read-only keys differ from the akt's.
pub(crate) fn check_read_only(akt: &Akt, parsed: &Parsed) -> Result<(), CliError> {
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
    // r[impl cli.file.kinds]
    if parsed.front.contains_key("version") {
        return Err(read_only(
            "version",
            "a version never changes; edit the living version",
        ));
    }
    if parsed.text("state").is_some_and(|s| s != akt.state.key) {
        return Err(read_only("state", "perform a transition with `kuria do`"));
    }
    let outcome = akt.outcome.as_ref().map(|o| o.to_lowercase());
    outcome_unchanged(&akt.number, parsed, outcome.as_deref())?;
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
        return Err(read_only(
            "in",
            "use `kuria members add`, `remove` or `transfer`",
        ));
    }
    let links: BTreeSet<String> = file::filed(&akt.links).map(file::Link::said).collect();
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
            // r[impl cli.file.kinds]
            // A version never changes: refused whatever revision it names.
            if parsed.front.contains_key("version") {
                return Err(CliError {
                    field: Some("version".to_owned()),
                    ..CliError::refused(
                        "READ_ONLY",
                        format!(
                            "{}: {number} is read at a version, which never changes; edit the living version",
                            path.display()
                        ),
                    )
                });
            }
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

/// A batch's refusal of its operation `n`, said of the akt it edited, and
/// that nothing changed.
fn in_batch(error: CliError, edited: &[&String]) -> CliError {
    let said = (1..=edited.len()).find_map(|n| {
        error
            .message
            .strip_prefix(&format!("operation {n}: "))
            .map(|rest| format!("{}: {rest}; nothing was changed", edited[n - 1]))
    });
    match said {
        Some(message) => CliError { message, ..error },
        None => error,
    }
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
    // Every edit in one batch: all of them, or none (`akt.batch`).
    let edits: Vec<Json> = checked
        .iter()
        .filter(|(.., changed)| !changed.is_empty() && !dry_run)
        .map(|(_, number, revision, _, changed)| {
            json!({ "editAkt": { "number": number, "revision": revision, "fields": changed, "reason": reason } })
        })
        .collect();
    let edited: Vec<&String> = checked
        .iter()
        .filter(|(.., changed)| !changed.is_empty() && !dry_run)
        .map(|(_, number, ..)| number)
        .collect();
    let mut now = Vec::new();
    if !edits.is_empty() {
        let data = session
            .graphql(
                "mutation($o: [OperationInput!]!) { batch(input: { operations: $o }) { revision } }",
                json!({ "o": edits }),
            )
            .await
            .map_err(|e| in_batch(e, &edited))?;
        now = data["batch"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|r| r["revision"].as_i64().unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default();
    }
    let mut now = now.into_iter();
    let mut lines = Vec::new();
    let mut applied = Vec::new();
    for (path, number, _, akt, changed) in checked {
        let names: Vec<&str> = changed.keys().map(|k| akt.shape.field_name(k)).collect();
        if changed.is_empty() {
            lines.push(format!("{number}: nothing changed."));
            applied.push(
                json!({ "file": path, "number": number, "revision": akt.revision, "changed": [] }),
            );
        } else if dry_run {
            lines.push(format!("{number}: would change {}.", names.join(", ")));
            applied.push(json!({ "file": path, "number": number, "revision": akt.revision, "changed": changed.keys().collect::<Vec<_>>(), "dryRun": true }));
        } else {
            let revision = now.next().unwrap_or_default();
            lines.push(format!(
                "{number}: changed {} (revision {revision}).",
                names.join(", ")
            ));
            applied.push(json!({ "file": path, "number": number, "revision": revision, "changed": changed.keys().collect::<Vec<_>>() }));
        }
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

/// The replacements `fields`, `spans` and `texts` give, one of each in
/// order (`cli.write`).
pub fn replacements(
    fields: &[String],
    spans: &[String],
    texts: &[String],
) -> Result<Vec<kuria_engine::replace::Replacement>, CliError> {
    if fields.len() != spans.len() || spans.len() != texts.len() {
        return Err(CliError::refused(
            "BAD_REQUEST",
            "give each replacement as --field, --span and --text, one of each",
        ));
    }
    Ok(fields
        .iter()
        .zip(spans)
        .zip(texts)
        .map(|((field, span), text)| kuria_engine::replace::Replacement {
            field: field.clone(),
            span: span.clone(),
            text: text.clone(),
        })
        .collect())
}

/// What replacing answers: for each replacement, its field's text around
/// what its span became, as the field now reads (`mcp.edit`).
pub(crate) fn replaced(
    number: &str,
    revision: &Json,
    now: &serde_json::Map<String, Json>,
    replacements: &[kuria_engine::replace::Replacement],
) -> Answer {
    const AROUND: usize = 60;
    let mut lines = vec![format!(
        "{number}: replaced {} span{} (revision {revision}).",
        replacements.len(),
        if replacements.len() == 1 { "" } else { "s" }
    )];
    let mut around = Vec::new();
    for r in replacements {
        let text = now.get(&r.field).and_then(Json::as_str).unwrap_or_default();
        let shown = match (r.text.is_empty(), text.find(&r.text)) {
            (false, Some(at)) => {
                let start = (0..=at.saturating_sub(AROUND))
                    .rev()
                    .find(|&i| text.is_char_boundary(i))
                    .unwrap_or(0);
                let end = ((at + r.text.len() + AROUND).min(text.len())..=text.len())
                    .find(|&i| text.is_char_boundary(i))
                    .unwrap_or(text.len());
                text[start..end].to_owned()
            }
            _ => String::new(),
        };
        lines.push(format!("\n`{}` now reads:\n\n```\n{shown}\n```", r.field));
        around.push(json!({ "field": r.field, "text": shown }));
    }
    Answer {
        markdown: lines.join("\n"),
        json: json!({ "number": number, "revision": revision, "replaced": around }),
    }
}

/// Shows the caller's cursor at the first of `replacements` that the akt's
/// live draft holds, to those editing it (`live.presence.shown`); quietly
/// nothing if it has none, or no span is there.
// r[impl mcp.presence]
pub async fn show_cursor(
    session: &Session,
    number: &str,
    replacements: &[kuria_engine::replace::Replacement],
) {
    for r in replacements {
        let shown = session
            .graphql(
                "mutation($n: String!, $f: String!, $s: String!) {
                   showCursor(input: { number: $n, field: $f, span: $s }) }",
                json!({ "n": number, "f": r.field, "s": r.span }),
            )
            .await;
        match shown {
            Ok(_) => return,
            // No live draft, or Kuria can't show it: the edit goes ahead.
            Err(e) if e.code != "NO_MATCH" && e.code != "AMBIGUOUS" => return,
            Err(_) => {}
        }
    }
}

/// `kuria replace <number>`: spans of text fields replaced, each found in
/// the field as it is when Kuria applies it (`akt.replace`).
// r[impl cli.write]
pub async fn replace(
    session: &Session,
    number: &str,
    replacements: &[kuria_engine::replace::Replacement],
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let given: Vec<Json> = replacements
        .iter()
        .map(|r| json!({ "field": r.field, "span": r.span, "text": r.text }))
        .collect();
    let data = session
        .graphql(
            "mutation($n: String!, $r: [ReplacementInput!]!, $reason: String) {
               replaceInAkt(input: { number: $n, replacements: $r, reason: $reason }) {
                 revision fields { ... on TextValue { field text } ... on MarkdownValue { field markdown } } } }",
            json!({ "n": number, "r": given, "reason": reason }),
        )
        .await?;
    let now: serde_json::Map<String, Json> = data["replaceInAkt"]["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let text = f["text"].as_str().or(f["markdown"].as_str())?;
            Some((f["field"].as_str()?.to_owned(), Json::from(text)))
        })
        .collect();
    Ok(replaced(
        number,
        &data["replaceInAkt"]["revision"],
        &now,
        replacements,
    ))
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

/// `shape`'s verdict field, refused if it has none.
pub(crate) fn verdict_field<'a>(
    shape: &'a crate::model::Shape,
    number: &str,
) -> Result<&'a crate::model::FieldDef, CliError> {
    shape
        .fields
        .iter()
        .find(|f| f.kind == "VERDICT")
        .ok_or_else(|| {
            CliError::refused(
                "BAD_REQUEST",
                format!("{number} is a {}, which takes no verdicts", shape.name),
            )
        })
}

/// A verdict's input (`akt.field.verdict`): `pass`, `fail` or `skip` for
/// oneself, with its description, with `for` another assignee's, and
/// `clear` to take one back.
// r[impl cli.write]
pub(crate) fn verdict_input(
    given: &str,
    description: Option<&str>,
    for_whom: Option<Json>,
) -> Result<Json, CliError> {
    let given = given.trim().to_ascii_lowercase();
    match (given.as_str(), for_whom) {
        ("pass" | "fail" | "skip", who) => {
            let mut input = json!({ "verdict": given });
            if let Some(description) = description {
                input["description"] = description.into();
            }
            if let Some(who) = who {
                input["for"] = who;
            }
            Ok(input)
        }
        ("clear", _) if description.is_some() => Err(CliError::refused(
            "BAD_REQUEST",
            "a verdict taken back has no description".to_owned(),
        )),
        ("clear", None) => Ok(json!({ "clear": true })),
        ("clear", Some(who)) => Ok(json!({ "clear": true, "for": who })),
        _ => Err(CliError::refused(
            "BAD_REQUEST",
            format!("`{given}` isn't a verdict: pass, fail, skip or clear"),
        )),
    }
}

/// What `verdict` answers: the verdict given, the revision it made and
/// the outcome now.
pub(crate) fn verdict_answer(
    number: &str,
    given: &str,
    for_whom: Option<&str>,
    revision: i64,
    outcome: Option<&str>,
) -> (String, Json) {
    let given = given.trim().to_ascii_lowercase();
    let mut said = if given == "clear" {
        "verdict cleared".to_owned()
    } else {
        given.clone()
    };
    if let Some(who) = for_whom {
        said.push_str(&format!(" for {who}"));
    }
    let outcome = outcome.map(str::to_lowercase);
    let mut markdown = format!("{number}: {said} (revision {revision})");
    if let Some(outcome) = &outcome {
        markdown.push_str(&format!("; its outcome is {outcome}"));
    }
    markdown.push('.');
    (
        markdown,
        json!({
            "number": number,
            "revision": revision,
            "verdict": (given != "clear").then_some(given),
            "for": for_whom,
            "outcome": outcome,
        }),
    )
}

/// `kuria verdict <number> pass|fail|skip|clear [--description <text>]
/// [--for <user>]`.
// r[impl cli.write]
pub async fn verdict(
    session: &Session,
    number: &str,
    given: &str,
    description: Option<&str>,
    for_whom: Option<&str>,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let akt = akt(session, number).await?;
    let field = verdict_field(&akt.shape, number)?;
    let who = match for_whom {
        Some(text) => Some(
            crate::reads::people(crate::reads::Source::Server(session))
                .await?
                .input(text)?,
        ),
        None => None,
    };
    let mut fields = Map::new();
    fields.insert(field.key.clone(), verdict_input(given, description, who)?);
    let data = session
        .graphql(
            "mutation($n: String!, $v: Int!, $f: JSON!, $r: String) {
               editAkt(input: { number: $n, revision: $v, fields: $f, reason: $r }) { revision outcome } }",
            json!({ "n": number, "v": akt.revision, "f": fields, "r": reason }),
        )
        .await?;
    let done = &data["editAkt"];
    let (markdown, json) = verdict_answer(
        number,
        given,
        for_whom,
        done["revision"].as_i64().unwrap_or_default(),
        done["outcome"].as_str(),
    );
    Ok(Answer { markdown, json })
}
