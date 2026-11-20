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
pub mod merge;
pub mod outcomes;
mod rank;
pub mod replace;
pub mod rules;
pub mod sbom;
pub mod sections;
pub mod typst;
mod value;
pub mod verdicts;
pub mod versions;
pub mod waits;

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
    #[error("`{field}` is given only while {}, not while `{state}`", one_of(.states))]
    NotInState {
        field: String,
        state: String,
        states: Vec<String>,
    },
    #[error("the akt is in a final state")]
    Final,
    #[error("the change changes nothing")]
    NoChange,
    /// A replacement's span isn't in its field (`akt.replace.match`).
    #[error("`{field}` doesn't hold the span: read it again")]
    NoMatch { field: String },
    /// A replacement's span occurs `count` times in its field.
    #[error("`{field}` holds the span {count} times: give a longer one")]
    Ambiguous { field: String, count: usize },
    /// A replacement that can't be made, whatever the field holds.
    #[error("`{field}`: {reason}")]
    BadReplacement { field: String, reason: String },
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
    #[error(
        "{} locked while `{state}`: changing it needs a reason",
        .fields.iter().map(|f| format!("`{f}` is")).collect::<Vec<_>>().join(", ")
    )]
    Locked {
        /// The locked fields the change changes.
        fields: Vec<String>,
        /// The state they are locked in.
        state: String,
    },
    #[error("`{transition}` waits for {}", waiting(.open, .lacking))]
    Waits {
        transition: String,
        /// The members and link targets not yet final, by number.
        open: Vec<String>,
        /// The link kinds it requires and the akt lacks, by key.
        lacking: Vec<String>,
    },
}

/// What a waiting transition waits for, in words.
fn waiting(open: &[String], lacking: &[String]) -> String {
    let mut parts = Vec::new();
    if !open.is_empty() {
        let these = if open.len() == 1 { "is" } else { "are" };
        parts.push(format!("{}, which {these} not final yet", all_of(open)));
    }
    if !lacking.is_empty() {
        let kinds: Vec<String> = lacking.iter().map(|k| format!("a `{k}` link")).collect();
        parts.push(all_of(&kinds));
    }
    parts.join(", and ")
}

/// `["a", "b", "c"]` as "a, b and c".
fn all_of(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What a transition needs to know of other akter: what the akt's members
/// add up to (`held`), and what it waits for (`waits`). An
/// `Option<outcomes::Outcome>` is one that waits for nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Around {
    pub held: Option<outcomes::Outcome>,
    pub waits: waits::Waits,
}

impl From<Option<outcomes::Outcome>> for Around {
    fn from(held: Option<outcomes::Outcome>) -> Self {
        Self {
            held,
            waits: waits::Waits::default(),
        }
    }
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
    in_states(ty, input, |states| states.contains(&state), &state)?;
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
    in_states(
        ty,
        input,
        |states| states.contains(&current.state),
        &current.state,
    )?;
    let (fields, changes) = apply(ty, &current.fields, input, change)?;
    if changes.is_empty() {
        return Err(Refusal::NoChange);
    }
    Ok(outcome(current.state.clone(), fields, changes))
}

/// Refuses a value `input` gives for a field that names the states it is
/// given in (`akt.field.states`) unless `open` says those states allow it
/// now, while in `state`.
// r[impl akt.field.states]
fn in_states(
    ty: &AktType,
    input: &Map<String, Json>,
    open: impl Fn(&[String]) -> bool,
    state: &str,
) -> Result<(), Refusal> {
    for key in input.keys() {
        let Some(def) = ty.document().fields.iter().find(|f| &f.key == key) else {
            continue;
        };
        if !def.states.is_empty() && !open(&def.states) {
            return Err(Refusal::NotInState {
                field: key.clone(),
                state: state.to_owned(),
                states: def.states.clone(),
            });
        }
    }
    Ok(())
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
/// the way. `around.held` is what the akt's members add up to
/// (`outcomes::of_all`) if its type has outcomes and no `verdict` field of
/// its own; a caller needs to work it out only for a transition that names
/// `outcomes`. `around.waits` is what it waits for (`waits::of`), which a
/// caller needs to work out only for one that `waits::waits_at_all`.
// r[impl akt.transition]
pub fn transition(
    ty: &AktType,
    current: &Current,
    transition: &str,
    input: &Map<String, Json>,
    change: Change,
    around: impl Into<Around>,
) -> Result<Outcome, Refusal> {
    let Around { held, waits } = around.into();
    let Some(def) = ty.transition(transition) else {
        return Err(Refusal::UnknownTransition(transition.to_owned()));
    };
    // r[impl akt.transition.reopens]
    if !def.reopens {
        refuse_final(ty, current)?;
    }
    if !def.from.contains(&current.state) {
        return Err(Refusal::NotFrom {
            transition: transition.to_owned(),
            state: current.state.clone(),
        });
    }
    // A transition gives what it requires, and what belongs to the state
    // it leaves or the one it leads to.
    for key in input.keys() {
        if def.requires.contains(key) {
            continue;
        }
        in_states(
            ty,
            &Map::from_iter([(key.clone(), Json::Null)]),
            |states| states.contains(&current.state) || states.contains(&def.to),
            &current.state,
        )?;
    }
    let (fields, changes) = apply(ty, &current.fields, input, change)?;
    // r[impl akt.field.checkbox]
    // A checkbox counts as holding a value only while checked.
    // r[impl akt.transition.requires-when]
    let mut missing: Vec<String> = def
        .requires
        .iter()
        .filter(|f| !fields.contains_key(*f))
        .cloned()
        .collect();
    for key in required_when(def, &fields) {
        if !fields.contains_key(&key) && !missing.contains(&key) {
            missing.push(key);
        }
    }
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
    // r[impl akt.transition.members]
    // r[impl akt.transition.links]
    if !waits.is_empty() {
        return Err(Refusal::Waits {
            transition: transition.to_owned(),
            open: waits.open,
            lacking: waits.lacking,
        });
    }
    Ok(outcome(def.to.clone(), fields, changes))
}

/// The fields `def` requires only while others hold certain values
/// (`akt.transition.requires-when`), that `fields` meet the condition of.
// r[impl akt.transition.requires-when]
#[must_use]
pub fn required_when(
    def: &kuria_config::TransitionDef,
    fields: &BTreeMap<String, Value>,
) -> Vec<String> {
    let holds = |key: &str| -> Vec<&str> {
        match fields.get(key) {
            Some(Value::Choice(option)) => vec![option.as_str()],
            Some(Value::Choices(options)) => options.iter().map(String::as_str).collect(),
            _ => Vec::new(),
        }
    };
    def.requires_when
        .iter()
        .filter(|rule| rule.applies(holds))
        .map(|rule| rule.field.clone())
        .collect()
}

/// The due fields (`akt.field.due`) an akt of `ty` in `current` is overdue
/// on, `today`: those holding a date before it, while its state isn't
/// final. Worked out when read, never stored.
// r[impl akt.field.due]
#[must_use]
pub fn overdue(ty: &AktType, current: &Current, today: chrono::NaiveDate) -> Vec<String> {
    if ty.state(&current.state).is_none_or(|s| s.is_final) {
        return Vec::new();
    }
    ty.document()
        .fields
        .iter()
        .filter(|f| f.due)
        .filter(|f| matches!(current.fields.get(&f.key), Some(Value::Date(d)) if *d < today))
        .map(|f| f.key.clone())
        .collect()
}

/// Refuses `outcome`, a change of `current`, if it changes a value a field
/// locked in `current`'s state holds, and gives no `reason`
/// (`akt.field.locked`).
// r[impl akt.field.locked]
pub fn check_locked(
    ty: &AktType,
    current: &Current,
    outcome: &Outcome,
    reason: Option<&str>,
) -> Result<(), Refusal> {
    if reason.is_some_and(|r| !r.trim().is_empty()) {
        return Ok(());
    }
    let fields: Vec<String> = ty
        .document()
        .fields
        .iter()
        .filter(|f| f.locked.contains(&current.state))
        .filter(|f| current.fields.contains_key(&f.key) && outcome.changes.contains_key(&f.key))
        .map(|f| f.key.clone())
        .collect();
    if fields.is_empty() {
        return Ok(());
    }
    Err(Refusal::Locked {
        fields,
        state: current.state.clone(),
    })
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
    // r[impl version.file]
    if let Some(file) = ty.released_file()
        && changes.contains_key(file)
    {
        return Err(Refusal::InvalidValue {
            field: file.to_owned(),
            reason: "only releasing a version sets it, with the version's PDF".to_owned(),
        });
    }
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
