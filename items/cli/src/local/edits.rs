//! `new`, `set`, `do`, `apply` and `types define` in local mode, through
//! the engine's rules (`cli.local.rules`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kuria_config::{AktTypeDocument, check_field_references, check_state_references, validate};
use kuria_engine::rules::{containment, numbering};
use kuria_engine::{Refusal, rank_between};
use serde_json::{Map, Value as Json, json};

use super::dir::LocalType;
use super::values::{after, current, input};
use super::write::{Writer, answer, numbers, revision};
use crate::error::{CliError, Kind};
use crate::file::{self, Parsed};
use crate::model::Shape;
use crate::output::Answer;
use crate::profile::io_error;
use crate::writes::{known_field, split_sets};

fn read_file(path: &Path) -> Result<Parsed, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| io_error(path, &e))?;
    file::parse(&text).map_err(|e| CliError {
        message: format!("{}: {}", path.display(), e.message),
        ..e
    })
}

fn refusal(r: &Refusal) -> CliError {
    CliError::from_server("REFUSED", r.to_string(), None)
}

/// The inputs the engine takes for `given` values of `shape`'s fields, for
/// the akt `number`.
fn inputs(
    w: &mut Writer,
    shape: &Shape,
    number: &str,
    given: &Map<String, Json>,
) -> Result<Map<String, Json>, CliError> {
    let mut inputs = Map::new();
    for (key, raw) in given {
        let field = known_field(shape, key)?;
        inputs.insert(key.clone(), field_input(w, shape, number, field, raw)?);
    }
    Ok(inputs)
}

/// The input the engine takes for `raw` given for `field` of the akt
/// `number`: an attachment as a file of the directory.
fn field_input(
    w: &mut Writer,
    shape: &Shape,
    number: &str,
    field: &crate::model::FieldDef,
    raw: &Json,
) -> Result<Json, CliError> {
    let raw = if field.kind == "ATTACHMENT" {
        let ty = w.dir.types[&shape.key].ty.clone();
        w.attachment(&ty, &field.key, number, raw)?
    } else {
        raw.clone()
    };
    input(field, &raw, &mut w.ids)
}

/// `kuria new <type>` in local mode.
// r[impl cli.local.rules]
// r[impl cli.local.numbers]
pub fn create(
    w: &mut Writer,
    akt_type: &str,
    inside: &[String],
    sets: &[String],
    path: Option<&Path>,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let shape = w.shape(akt_type)?;
    let mut given = Map::new();
    let mut containers: Vec<String> = Vec::new();
    if let Some(path) = path {
        let parsed = read_file(path)?;
        if let Some(key) = parsed.text("type").filter(|k| *k != shape.key) {
            return Err(CliError::refused(
                "BAD_FILE",
                format!("{} is a `{key}`, not a `{}`", path.display(), shape.key),
            ));
        }
        if let Some(Json::Array(listed)) = parsed.front.get("in") {
            containers.extend(listed.iter().filter_map(Json::as_str).map(str::to_owned));
        }
        given = parsed.values()?;
    }
    containers.extend(inside.iter().cloned());
    given.extend(split_sets(&shape, sets)?);
    // The number it will have, for the attachments it is given.
    let number = numbering::next_from(
        &w.dir.types[akt_type].ty,
        w.now,
        containers.first().map(String::as_str),
        numbers(&w.dir),
    );
    let mut fields = inputs(w, &shape, &number, &given)?;
    fields.retain(|_, v| !v.is_null());

    let number = create_with(w, akt_type, &fields, &containers)?;
    Ok(answer(
        format!("Created {number}."),
        revision(&number, 1),
        reason,
    ))
}

/// Creates an akt of `akt_type` with the inputs `fields` in `containers`,
/// by the rules a server creates one by, and answers its number.
// r[impl cli.local.rules]
// r[impl cli.local.numbers]
pub(crate) fn create_with(
    w: &mut Writer,
    akt_type: &str,
    fields: &Map<String, Json>,
    containers: &[String],
) -> Result<String, CliError> {
    let refs: Vec<&str> = containers.iter().map(String::as_str).collect();
    containment::check_distinct(&refs)?;
    let parents = containers
        .iter()
        .map(|n| w.akt_ref(n))
        .collect::<Result<Vec<_>, _>>()?;
    let t = w.dir.types[akt_type].clone();
    let kinds: Vec<&kuria_config::AktType> = parents.iter().map(|p| p.ty).collect();
    containment::check_created_in(&t.ty, &kinds)?;
    containment::check_create_inside(&t.ty, &parents)?;

    let change = w.change();
    let outcome = kuria_engine::create(&t.ty, fields, change).map_err(|r| refusal(&r))?;
    w.check_sections(&t.ty, None, &outcome.changes)?;
    let number = numbering::next_from(
        &t.ty,
        w.now,
        containers.first().map(String::as_str),
        numbers(&w.dir),
    );
    let rank = rank_between(w.last_rank(), None)
        .map_err(|e| CliError::refused("INTERNAL", e.to_string()))?;
    let changed: Vec<String> = outcome.changes.keys().cloned().collect();
    let values = after(None, &outcome.fields, &changed, &w.ids);
    let akt = w.new_akt(
        number.clone(),
        &t,
        outcome.state,
        rank,
        containers.to_vec(),
        values,
    );
    w.add(akt);
    for parent in containers {
        w.bump(parent);
    }
    Ok(number)
}

/// `kuria set <number> <field>=<value>…` in local mode.
// r[impl cli.local.rules]
pub fn set(
    w: &mut Writer,
    number: &str,
    sets: &[String],
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let key = w.akt(number)?.type_key.clone();
    let shape = w.shape(&key)?;
    let given = split_sets(&shape, sets)?;
    let fields = inputs(w, &shape, number, &given)?;
    let current = w.current(number)?;
    let t = w.dir.types[&key].clone();
    let change = w.change();
    let outcome = kuria_engine::edit(&t.ty, &current, &fields, change).map_err(|r| refusal(&r))?;
    w.check_sections(&t.ty, Some(number), &outcome.changes)?;
    let changed: Vec<String> = outcome.changes.keys().cloned().collect();
    let values = after(w.dir.akt(number), &outcome.fields, &changed, &w.ids);
    w.akt_mut(number).values = values;
    let now = w.bump(number);
    let names: Vec<&str> = fields.keys().map(|k| shape.field_name(k)).collect();
    Ok(answer(
        format!("{number}: set {} (revision {now}).", names.join(", ")),
        json!({ "number": number, "revision": now, "changed": fields.keys().collect::<Vec<_>>() }),
        reason,
    ))
}

/// `kuria do <number> <transition>` in local mode: a transition by key or
/// name.
// r[impl cli.local.rules]
pub fn perform(
    w: &mut Writer,
    number: &str,
    transition: &str,
    sets: &[String],
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let akt = w.akt(number)?.clone();
    let shape = w.shape(&akt.type_key)?;
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
        .cloned()
        .ok_or_else(|| {
            let allowed: Vec<String> = shape
                .transitions
                .iter()
                .filter(|t| t.from.contains(&akt.state))
                .map(|t| format!("`{}` ({})", t.key, t.name))
                .collect();
            let state = shape
                .state(&akt.state)
                .map_or(akt.state.as_str(), |s| s.name.as_str());
            CliError::refused(
                "UNKNOWN_TRANSITION",
                format!(
                    "{} has no transition `{transition}`; from {state} it can take {}",
                    shape.name,
                    if allowed.is_empty() {
                        "none".to_owned()
                    } else {
                        allowed.join(", ")
                    }
                ),
            )
        })?;
    let given = split_sets(&shape, sets)?;
    let fields = inputs(w, &shape, number, &given)?;
    let current = w.current(number)?;
    let t = w.dir.types[&akt.type_key].clone();
    let change = w.change();
    // r[impl akt.transition.outcomes]
    // The directory lock holds every other write off until this one is done.
    let gated =
        t.ty.transition(&chosen.key)
            .is_some_and(|d| !d.outcomes.is_empty());
    let held = if gated {
        super::answers::outcome(&w.dir, &akt)?
    } else {
        None
    };
    let outcome = kuria_engine::transition(&t.ty, &current, &chosen.key, &fields, change, held)
        .map_err(|r| refusal(&r))?;
    w.check_sections(&t.ty, Some(number), &outcome.changes)?;
    let changed: Vec<String> = outcome.changes.keys().cloned().collect();
    let values = after(Some(&akt), &outcome.fields, &changed, &w.ids);
    let state = outcome.state.clone();
    let target = w.akt_mut(number);
    target.values = values;
    target.state.clone_from(&state);
    let now = w.bump(number);
    let state_name = shape
        .state(&state)
        .map_or(state.as_str(), |s| s.name.as_str())
        .to_owned();
    Ok(answer(
        format!("{number}: {} → {state_name} (revision {now}).", chosen.name),
        json!({ "number": number, "revision": now, "state": state, "transition": chosen.key }),
        reason,
    ))
}

/// `kuria verdict <number> pass|fail|skip|clear [--for <user>]` in local
/// mode: as `@me`, by the engine's rules (`akt.field.verdict`).
// r[impl cli.local.rules]
pub fn verdict(
    w: &mut Writer,
    number: &str,
    given: &str,
    for_whom: Option<&str>,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let key = w.akt(number)?.type_key.clone();
    let shape = w.shape(&key)?;
    let field = crate::writes::edit::verdict_field(&shape, number)?
        .key
        .clone();
    let who = match for_whom {
        Some(text) => Some(Json::from(w.ids.person(text)?.to_string())),
        None => None,
    };
    let mut fields = Map::new();
    fields.insert(field, crate::writes::edit::verdict_input(given, who)?);
    let current = w.current(number)?;
    let t = w.dir.types[&key].clone();
    let change = w.change();
    let outcome = kuria_engine::edit(&t.ty, &current, &fields, change).map_err(|r| refusal(&r))?;
    let changed: Vec<String> = outcome.changes.keys().cloned().collect();
    let values = after(w.dir.akt(number), &outcome.fields, &changed, &w.ids);
    w.akt_mut(number).values = values;
    let now = w.bump(number);
    let akt = w.akt(number)?;
    let outcome = super::answers::outcome(&w.dir, akt)?;
    let (markdown, json) = crate::writes::edit::verdict_answer(
        number,
        given,
        for_whom,
        now.into(),
        outcome.map(kuria_engine::outcomes::Outcome::as_str),
    );
    Ok(answer(markdown, json, reason))
}

/// Refuses a file whose read-only keys differ from the akt's.
fn check_read_only(w: &Writer, number: &str, parsed: &Parsed) -> Result<(), CliError> {
    let akt = w.akt(number)?;
    let read_only = |key: &str, how: &str| CliError {
        field: Some(key.to_owned()),
        ..CliError::refused(
            "READ_ONLY",
            format!("`{key}` can't be changed in a file of {number}: {how}"),
        )
    };
    if parsed.front.contains_key("version") {
        return Err(read_only(
            "version",
            "a version never changes; edit the living version",
        ));
    }
    if parsed.text("type").is_some_and(|t| t != akt.type_key) {
        return Err(read_only("type", "an akt keeps its type"));
    }
    if parsed.text("state").is_some_and(|s| s != akt.state) {
        return Err(read_only("state", "perform a transition with `kuria do`"));
    }
    let outcome =
        super::answers::outcome(&w.dir, akt)?.map(kuria_engine::outcomes::Outcome::as_str);
    crate::writes::edit::outcome_unchanged(number, parsed, outcome)?;
    let listed = |key: &str| -> BTreeSet<String> {
        parsed.front[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
            .map(str::to_owned)
            .collect()
    };
    if parsed.front.contains_key("in") && listed("in") != akt.containers.iter().cloned().collect() {
        return Err(read_only("in", "use `kuria add`, `remove` or `transfer`"));
    }
    if parsed.front.contains_key("links") && listed("links") != akt.links.iter().cloned().collect()
    {
        return Err(read_only("links", "use `kuria link` or `unlink`"));
    }
    Ok(())
}

/// `kuria apply <file>…` in local mode: each akt edited with the fields its
/// file changed, the engine deciding what that is.
// r[impl cli.local.revision]
// r[impl cli.apply]
pub fn apply(
    w: &mut Writer,
    paths: &[PathBuf],
    dry_run: bool,
    reason: Option<&str>,
) -> Result<Answer, CliError> {
    let mut lines = Vec::new();
    let mut applied = Vec::new();
    // Nothing is written until the end, so a refusal anywhere changes
    // nothing (`cli.apply`).
    for (path, number, at, parsed) in crate::writes::read_akter(paths)? {
        let akt = w.akt(&number)?.clone();
        if i64::from(akt.revision) != at {
            return Err(CliError::new(
                Kind::Stale,
                "STALE",
                format!(
                    "{number} is at revision {}, the file at {at}. Merge the file with `kuria cat {number}` and apply it again; `git log` says what changed",
                    akt.revision
                ),
            ));
        }
        check_read_only(w, &number, &parsed)?;
        let shape = w.shape(&akt.type_key)?;
        let given = parsed.values()?;
        for key in given.keys() {
            known_field(&shape, key)?;
        }
        // Every field: what the file leaves out, it clears.
        let mut fields = Map::new();
        for field in &shape.fields {
            if field.kind == "VERDICT" {
                let now = akt
                    .values
                    .get(&field.key)
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                crate::writes::edit::verdicts_unchanged(
                    &number,
                    field,
                    given.get(&field.key),
                    &now,
                )?;
                continue;
            }
            let raw = given.get(&field.key).cloned().unwrap_or(Json::Null);
            let input = field_input(w, &shape, &number, field, &raw)?;
            fields.insert(field.key.clone(), input);
        }
        let current = w.current(&number)?;
        let t = w.dir.types[&akt.type_key].clone();
        let change = w.change();
        // An akt file applied unchanged changes nothing, also of a final
        // akt, as against a server (`cli.local.rules`).
        let edited = match kuria_engine::changes_anything(&t.ty, &current, &fields, change) {
            Ok(false) => Err(Refusal::NoChange),
            _ => kuria_engine::edit(&t.ty, &current, &fields, change),
        };
        let outcome = match edited {
            Ok(outcome) => outcome,
            Err(Refusal::NoChange) => {
                lines.push(format!("{number}: nothing changed."));
                applied.push(json!({ "file": path, "number": number, "revision": akt.revision, "changed": [] }));
                continue;
            }
            Err(r) => return Err(refusal(&r)),
        };
        w.check_sections(&t.ty, Some(&number), &outcome.changes)?;
        let changed: Vec<String> = outcome.changes.keys().cloned().collect();
        let names: Vec<&str> = changed.iter().map(|k| shape.field_name(k)).collect();
        if dry_run {
            lines.push(format!("{number}: would change {}.", names.join(", ")));
            applied.push(json!({ "file": path, "number": number, "revision": akt.revision, "changed": changed, "dryRun": true }));
            continue;
        }
        let values = after(Some(&akt), &outcome.fields, &changed, &w.ids);
        w.akt_mut(&number).values = values;
        let now = w.bump(&number);
        lines.push(format!(
            "{number}: changed {} (revision {now}).",
            names.join(", ")
        ));
        applied
            .push(json!({ "file": path, "number": number, "revision": now, "changed": changed }));
    }
    Ok(answer(
        lines.join("\n"),
        json!({ "applied": applied }),
        reason,
    ))
}

/// `kuria types define <file>…` in local mode: each document replaces its
/// type's, refused if what it names of other types doesn't hold, or if the
/// directory's akter of that type wouldn't fit it.
// r[impl cli.local.types]
pub fn define(w: &mut Writer, documents: Vec<Json>) -> Result<Answer, CliError> {
    let mut defined = Vec::new();
    for document in documents {
        let parsed: AktTypeDocument = serde_json::from_value(document).map_err(|e| {
            CliError::refused(
                "INVALID_AKT_TYPE",
                format!("the akt type document can't be read: {e}"),
            )
        })?;
        let ty = validate(parsed).map_err(|problems| {
            CliError::refused(
                "INVALID_AKT_TYPE",
                format!(
                    "the akt type document is invalid\n- {}",
                    problems
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n- ")
                ),
            )
        })?;
        let key = ty.key().to_owned();
        let others: Vec<kuria_config::AktType> = w
            .dir
            .types
            .values()
            .filter(|t| t.ty.key() != key)
            .map(|t| t.ty.clone())
            .collect();
        let mut problems = check_state_references(&ty, &others);
        problems.extend(check_field_references(&ty, &others));
        if !problems.is_empty() {
            return Err(CliError::refused(
                "INVALID_AKT_TYPE",
                format!(
                    "the akt type document is invalid\n- {}",
                    problems
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n- ")
                ),
            ));
        }
        let unfit: Vec<String> = w
            .dir
            .akter
            .iter()
            .filter(|a| a.type_key == key)
            .filter(|a| {
                ty.state(&a.state).is_none()
                    || a.values.keys().any(|k| ty.field(k).is_none())
                    || current(&ty, a, &mut super::values::Ids::default()).is_err()
            })
            .map(|a| a.number.clone())
            .collect();
        if !unfit.is_empty() {
            return Err(CliError::refused(
                "INVALID_AKT_TYPE",
                format!(
                    "`{key}`'s akter wouldn't fit it: {}; change them, or the document, first",
                    unfit.join(", ")
                ),
            ));
        }
        let document = serde_json::to_value(ty.document()).expect("serializable");
        w.define(LocalType { ty, document });
        defined.push(json!({ "key": key, "version": 1 }));
    }
    Ok(Answer {
        markdown: defined
            .iter()
            .map(|d| {
                format!(
                    "Defined `{}`, version 1.",
                    d["key"].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        json: json!({ "defined": defined }),
    })
}
