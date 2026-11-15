//! The rules for changing an akt: what a create, an edit or a transition
//! produces, or why it is refused.
//!
//! No I/O: the caller loads the akt type and the akt, asks the engine, and
//! stores what it answers.

pub mod board;
pub mod copying;
pub mod draft;
mod hash;
pub mod live;
pub mod markdown;
pub mod outcomes;
mod rank;
pub mod rules;
pub mod sbom;
pub mod sections;
pub mod typst;
mod value;
pub mod verdicts;
pub mod versions;

use std::collections::BTreeMap;

use kuria_config::{AktType, FieldKind};
use serde_json::{Map, Value as Json};

pub use hash::state_hash;
pub use rank::{RankError, nth_appended, placement, rank_between};
pub use value::{Change, Input, Someone, Value, fields_from_json, fields_to_json};

/// An akt's state and field values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Current {
    pub state: String,
    pub fields: BTreeMap<String, Value>,
}

/// What a change produces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The akt's state afterwards.
    pub state: String,
    /// Every field value afterwards.
    pub fields: BTreeMap<String, Value>,
    /// The fields the change set, with `None` for those it cleared.
    pub changes: BTreeMap<String, Option<Value>>,
    /// The SHA-256 of the resulting state and field values.
    pub hash: [u8; 32],
}

/// Why a change was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the akt type has no field `{0}`")]
    UnknownField(String),
    #[error("`{field}`: {reason}")]
    InvalidValue { field: String, reason: String },
    #[error("`{0}` is required")]
    Required(String),
    #[error("the akt is in a final state")]
    Final,
    #[error("the change changes nothing")]
    NoChange,
    #[error("the akt type has no transition `{0}`")]
    UnknownTransition(String),
    #[error("`{transition}` can't be performed from `{state}`")]
    NotFrom { transition: String, state: String },
    #[error("`{transition}` requires {}", .fields.join(", "))]
    Requires {
        transition: String,
        fields: Vec<String>,
    },
    #[error(
        "`{transition}` requires the outcome to be {}, and {}",
        one_of(.allowed),
        .outcome.map_or("the akt has no outcome".to_owned(), |o| format!("it is {}", o.as_str()))
    )]
    Outcome {
        transition: String,
        /// The akt's outcome, `None` if it has none.
        outcome: Option<outcomes::Outcome>,
        allowed: Vec<String>,
    },
}

/// `["a", "b", "c"]` as "a, b or c".
fn one_of(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// Creates an akt of `ty` with the values `input` gives.
// r[impl akt.create]
pub fn create(ty: &AktType, input: &Map<String, Json>, change: Change) -> Result<Outcome, Refusal> {
    let state = ty.initial_state().key.clone();
    let (fields, changes) = apply(ty, &BTreeMap::new(), input, change)?;
    // r[impl akt.field.required]
    if let Some(missing) = ty
        .document()
        .fields
        .iter()
        .find(|f| f.required && !fields.contains_key(&f.key))
    {
        return Err(Refusal::Required(missing.key.clone()));
    }
    Ok(outcome(state, fields, changes))
}

/// Edits `current` with the values `input` gives, where `null` clears.
// r[impl akt.edit]
pub fn edit(
    ty: &AktType,
    current: &Current,
    input: &Map<String, Json>,
    change: Change,
) -> Result<Outcome, Refusal> {
    refuse_final(ty, current)?;
    let (fields, changes) = apply(ty, &current.fields, input, change)?;
    if changes.is_empty() {
        return Err(Refusal::NoChange);
    }
    Ok(outcome(current.state.clone(), fields, changes))
}

/// Whether editing `current` with `input` would change anything, final or
/// not: what an apply of an unchanged akt file asks before editing.
pub fn changes_anything(
    ty: &AktType,
    current: &Current,
    input: &Map<String, Json>,
    change: Change,
) -> Result<bool, Refusal> {
    let (_, changes) = apply(ty, &current.fields, input, change)?;
    Ok(!changes.is_empty())
}

/// Performs `transition` on `current`, setting the values `input` gives on
/// the way. `held` is what the akt's members add up to (`outcomes::of_all`)
/// if its type has outcomes and no `verdict` field of its own; a caller
/// needs to work it out only for a transition that names `outcomes`.
// r[impl akt.transition]
pub fn transition(
    ty: &AktType,
    current: &Current,
    transition: &str,
    input: &Map<String, Json>,
    change: Change,
    held: Option<outcomes::Outcome>,
) -> Result<Outcome, Refusal> {
    let Some(def) = ty.transition(transition) else {
        return Err(Refusal::UnknownTransition(transition.to_owned()));
    };
    refuse_final(ty, current)?;
    if !def.from.contains(&current.state) {
        return Err(Refusal::NotFrom {
            transition: transition.to_owned(),
            state: current.state.clone(),
        });
    }
    let (fields, changes) = apply(ty, &current.fields, input, change)?;
    // r[impl akt.field.checkbox]
    // A checkbox counts as holding a value only while checked.
    let missing: Vec<String> = def
        .requires
        .iter()
        .filter(|f| !fields.contains_key(*f))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(Refusal::Requires {
            transition: transition.to_owned(),
            fields: missing,
        });
    }
    // r[impl akt.transition.outcomes]
    // Its own verdicts as the transition leaves them, else its members'.
    if !def.outcomes.is_empty() {
        let outcome = outcomes::of_fields(ty, &fields).or(held);
        if !outcome.is_some_and(|o| def.outcomes.iter().any(|a| a == o.as_str())) {
            return Err(Refusal::Outcome {
                transition: transition.to_owned(),
                outcome,
                allowed: def.outcomes.clone(),
            });
        }
    }
    Ok(outcome(def.to.clone(), fields, changes))
}

pub(crate) fn refuse_final(ty: &AktType, current: &Current) -> Result<(), Refusal> {
    if ty.state(&current.state).is_some_and(|s| s.is_final) {
        return Err(Refusal::Final);
    }
    Ok(())
}

type Applied = (BTreeMap<String, Value>, BTreeMap<String, Option<Value>>);

/// `fields` with `input` applied, and what actually changed.
fn apply(
    ty: &AktType,
    fields: &BTreeMap<String, Value>,
    input: &Map<String, Json>,
    change: Change,
) -> Result<Applied, Refusal> {
    let before = fields;
    let mut fields = fields.clone();
    let mut changes = BTreeMap::new();
    // Verdicts last, against the assignees as the input leaves them.
    let mut givings = Vec::new();
    for (key, raw) in input {
        let Some(def) = ty.field(key) else {
            return Err(Refusal::UnknownField(key.clone()));
        };
        if def.kind == FieldKind::Verdict {
            let invalid = |reason: String| Refusal::InvalidValue {
                field: key.clone(),
                reason,
            };
            if raw.is_null() {
                return Err(invalid(
                    "a verdict is taken back with {\"clear\": true}".to_owned(),
                ));
            }
            givings.push((
                def,
                verdicts::Giving::from_input(raw, change).map_err(invalid)?,
            ));
            continue;
        }
        // r[impl akt.field.kinds]
        let input =
            Value::from_input(def, raw, change).map_err(|reason| Refusal::InvalidValue {
                field: key.clone(),
                reason,
            })?;
        match input {
            Input::Clear => {
                if def.required {
                    return Err(Refusal::Required(key.clone()));
                }
                if fields.remove(key).is_some() {
                    changes.insert(key.clone(), None);
                }
            }
            // r[impl akt.field.checkbox]
            // Checking a checked box keeps who checked it first.
            Input::Set(Value::Checkbox { .. })
                if matches!(fields.get(key), Some(Value::Checkbox { .. })) => {}
            Input::Set(value) => {
                if fields.get(key) != Some(&value) {
                    fields.insert(key.clone(), value.clone());
                    changes.insert(key.clone(), Some(value));
                }
            }
        }
    }
    // r[impl akt.field.verdict]
    for (def, giving) in givings {
        let given = verdicts_of(&fields, &def.key);
        let assignees = assignees_of(&fields, def);
        let now =
            giving
                .apply(&given, &assignees, change)
                .map_err(|reason| Refusal::InvalidValue {
                    field: def.key.clone(),
                    reason,
                })?;
        set_verdicts(&mut fields, &mut changes, &def.key, &given, now);
    }
    // An assignee removed loses their verdict in the same change.
    for def in ty
        .document()
        .fields
        .iter()
        .filter(|f| f.kind == FieldKind::Verdict)
    {
        let given = verdicts_of(&fields, &def.key);
        let kept = verdicts::only_of(&given, &assignees_of(&fields, def));
        set_verdicts(&mut fields, &mut changes, &def.key, &given, kept);
    }
    give_heading_ids(ty, before, &mut fields, &mut changes)?;
    Ok((fields, changes))
}

/// Gives the headings of the akt's `markdown` fields their ids, unique
/// across them, and counts a field as changed only if it now differs from
/// `before`, so that text sent without the ids it already had changes
/// nothing.
// r[impl akt.field.markdown]
fn give_heading_ids(
    ty: &AktType,
    before: &BTreeMap<String, Value>,
    fields: &mut BTreeMap<String, Value>,
    changes: &mut BTreeMap<String, Option<Value>>,
) -> Result<(), Refusal> {
    let keys: Vec<String> = ty
        .document()
        .fields
        .iter()
        .filter(|f| f.kind == FieldKind::Markdown && fields.contains_key(&f.key))
        .map(|f| f.key.clone())
        .collect();
    if keys.is_empty() {
        return Ok(());
    }
    let mut texts: Vec<String> = keys
        .iter()
        .map(|k| match &fields[k] {
            Value::Markdown(text) => text.clone(),
            _ => String::new(),
        })
        .collect();
    // Each field alone first, so a refusal names the field it is in.
    for (key, text) in keys.iter().zip(&texts) {
        markdown::give_ids(&mut [&mut text.clone()]).map_err(|reason| Refusal::InvalidValue {
            field: key.clone(),
            reason,
        })?;
    }
    let mut all: Vec<&mut String> = texts.iter_mut().collect();
    markdown::give_ids(&mut all).map_err(|reason| Refusal::InvalidValue {
        field: keys.last().cloned().unwrap_or_default(),
        reason,
    })?;
    for (key, text) in keys.into_iter().zip(texts) {
        let value = Value::Markdown(text);
        if before.get(&key) == Some(&value) {
            changes.remove(&key);
        } else {
            changes.insert(key.clone(), Some(value.clone()));
        }
        fields.insert(key, value);
    }
    Ok(())
}

pub(crate) fn verdicts_of(fields: &BTreeMap<String, Value>, key: &str) -> Vec<verdicts::Given> {
    match fields.get(key) {
        Some(Value::Verdicts(given)) => given.clone(),
        _ => Vec::new(),
    }
}

pub(crate) fn assignees_of(
    fields: &BTreeMap<String, Value>,
    def: &kuria_config::FieldDef,
) -> Vec<Someone> {
    match def.assignees.as_ref().and_then(|key| fields.get(key)) {
        Some(Value::Users(people)) => people.clone(),
        _ => Vec::new(),
    }
}

/// Sets a `verdict` field to `now`, none if empty, recording a change if it
/// isn't `before`.
fn set_verdicts(
    fields: &mut BTreeMap<String, Value>,
    changes: &mut BTreeMap<String, Option<Value>>,
    key: &str,
    before: &[verdicts::Given],
    now: Vec<verdicts::Given>,
) {
    if now == before {
        return;
    }
    if now.is_empty() {
        fields.remove(key);
        changes.insert(key.to_owned(), None);
    } else {
        fields.insert(key.to_owned(), Value::Verdicts(now.clone()));
        changes.insert(key.to_owned(), Some(Value::Verdicts(now)));
    }
}

pub(crate) fn outcome(
    state: String,
    fields: BTreeMap<String, Value>,
    changes: BTreeMap<String, Option<Value>>,
) -> Outcome {
    let hash = state_hash(&state, &fields);
    Outcome {
        state,
        fields,
        changes,
        hash,
    }
}

#[cfg(test)]
mod tests;
