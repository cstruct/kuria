//! The rules for changing an akt: what a create, an edit or a transition
//! produces, or why it is refused.
//!
//! No I/O: the caller loads the akt type and the akt, asks the engine, and
//! stores what it answers.

pub mod board;
pub mod draft;
mod hash;
mod rank;
mod value;

use std::collections::BTreeMap;

use kuria_config::AktType;
use serde_json::{Map, Value as Json};

pub use hash::state_hash;
pub use rank::{RankError, nth_appended, rank_between};
pub use value::{Change, Input, Value, fields_from_json, fields_to_json};

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

/// Performs `transition` on `current`, setting the values `input` gives on
/// the way.
// r[impl akt.transition]
pub fn transition(
    ty: &AktType,
    current: &Current,
    transition: &str,
    input: &Map<String, Json>,
    change: Change,
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
    Ok(outcome(def.to.clone(), fields, changes))
}

fn refuse_final(ty: &AktType, current: &Current) -> Result<(), Refusal> {
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
    let mut fields = fields.clone();
    let mut changes = BTreeMap::new();
    for (key, raw) in input {
        let Some(def) = ty.field(key) else {
            return Err(Refusal::UnknownField(key.clone()));
        };
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
    Ok((fields, changes))
}

fn outcome(
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
