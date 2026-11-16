//! Live drafts (`live.*`): an akt's fields as a Yjs document, read and
//! written with `yrs`.
//!
//! Each `markdown` and multiline `text` field is a `Y.Text` named by its
//! key, holding its text, empty for no value. Every other field is a value
//! in the `Y.Map` named [`FIELDS`], in its JSON form as Kuria answers it
//! ([`Value::to_json`]), absent for no value. Updates are encoded as Yjs v1
//! updates, as y-websocket speaks them.
//!
//! Writing fields to a draft changes only what differs, and a text only by
//! the characters that differ, so that edits made from the same draft
//! merge as text.

use std::collections::BTreeMap;
use std::time::Duration;

use kuria_config::{AktType, FieldDef, FieldKind};
use serde_json::Value as Json;
use similar::{DiffOp, TextDiff};
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;
use yrs::{Any, Doc, GetString, Map, MapRef, ReadTxn, StateVector, Text, TextRef, Transact};

use crate::{Change, Current, Outcome, Refusal, Value};

/// The name of the `Y.Map` holding every field that isn't a text.
pub const FIELDS: &str = "fields";

/// The Yjs client that writes a draft made from fields ([`LiveDraft::of`]).
pub const SEED: u64 = 1;

/// How long a text diff may take before it settles for a coarser one.
const DIFF_TIMEOUT: Duration = Duration::from_millis(500);

/// Whether `field` is a `Y.Text` of its own in a live draft: a `markdown`
/// field or a multiline `text` field (`live.fields`).
#[must_use]
pub fn is_text(field: &FieldDef) -> bool {
    field.kind == FieldKind::Markdown || (field.kind == FieldKind::Text && field.multiline)
}

/// An akt's live draft: a Yjs document of its fields.
pub struct LiveDraft {
    doc: Doc,
}

impl Default for LiveDraft {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for LiveDraft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveDraft").finish_non_exhaustive()
    }
}

impl LiveDraft {
    /// An empty live draft: every field without a value.
    #[must_use]
    pub fn new() -> Self {
        Self { doc: Doc::new() }
    }

    /// A live draft holding `fields` of `ty`. The same fields always make
    /// the same items, written by the client [`SEED`], so that a draft not
    /// stored yet reads alike each time it is made, and an update made
    /// against one applies to the next (`live.state`); what is written to
    /// it after is written by a client of its own.
    #[must_use]
    pub fn of(ty: &AktType, fields: &BTreeMap<String, Value>) -> Self {
        let mut seed = Self {
            doc: Doc::with_client_id(SEED),
        };
        seed.write(ty, fields);
        let mut draft = Self::new();
        // A draft's own state always applies; were it not to, the seed
        // still holds the fields.
        if draft.apply(&seed.state()).is_err() {
            return seed;
        }
        draft
    }

    /// The draft's base, as a push names it (`live.state`, `clone.server`):
    /// its Yjs v1 snapshot, what it holds and what of that is deleted. Its
    /// state vector alone wouldn't move with a change that only deletes.
    #[must_use]
    pub fn base(&self) -> Vec<u8> {
        use yrs::updates::encoder::Encode;
        self.doc.transact().snapshot().encode_v1()
    }

    /// Whether `base`, a Yjs v1 snapshot, is the draft's
    /// ([`LiveDraft::base`]): an update made against it is made against the
    /// whole draft as it is now (`clone.server`).
    pub fn is_at(&self, base: &[u8]) -> Result<bool, String> {
        let theirs = yrs::Snapshot::decode_v1(base).map_err(|e| e.to_string())?;
        Ok(theirs == self.doc.transact().snapshot())
    }

    /// The live draft `state` encodes, with `updates` applied in order.
    pub fn load<'a>(
        state: &[u8],
        updates: impl IntoIterator<Item = &'a [u8]>,
    ) -> Result<Self, String> {
        let mut draft = Self::new();
        if !state.is_empty() {
            draft.apply(state)?;
        }
        for update in updates {
            draft.apply(update)?;
        }
        Ok(draft)
    }

    /// Applies a Yjs v1 update to the draft.
    pub fn apply(&mut self, update: &[u8]) -> Result<(), String> {
        let update = yrs::Update::decode_v1(update).map_err(|e| e.to_string())?;
        self.doc
            .transact_mut()
            .apply_update(update)
            .map_err(|e| e.to_string())
    }

    /// The whole draft, as one Yjs v1 update.
    #[must_use]
    pub fn state(&self) -> Vec<u8> {
        self.doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
    }

    /// The draft's field values for `ty`, or why one can't be read.
    // r[impl live.fields]
    pub fn fields(&self, ty: &AktType) -> Result<BTreeMap<String, Value>, String> {
        let (texts, map) = self.roots(ty);
        let txn = self.doc.transact();
        let mut fields = BTreeMap::new();
        for field in &ty.document().fields {
            if let Some(text) = texts.get(&field.key) {
                let text = text.get_string(&txn);
                let empty = if field.kind == FieldKind::Markdown {
                    crate::markdown::is_empty(&text)
                } else {
                    text.is_empty()
                };
                if !empty {
                    let value = Value::from_json(field, &Json::String(text))
                        .map_err(|reason| format!("`{}`: {reason}", field.key))?;
                    fields.insert(field.key.clone(), value);
                }
                continue;
            }
            let Some(out) = map.get(&txn, &field.key) else {
                continue;
            };
            let json = json_of(&out.to_json(&txn))?;
            if json.is_null() {
                continue;
            }
            let value = Value::from_json(field, &json)
                .map_err(|reason| format!("`{}`: {reason}", field.key))?;
            fields.insert(field.key.clone(), value);
        }
        Ok(fields)
    }

    /// Makes the draft hold `fields` of `ty`, changing only what differs,
    /// each text by the characters that differ, and answers the update
    /// that did, `None` if nothing differed.
    // r[impl live.fields]
    // r[impl live.edit]
    pub fn write(&mut self, ty: &AktType, fields: &BTreeMap<String, Value>) -> Option<Vec<u8>> {
        let (texts, map) = self.roots(ty);
        let mut txn = self.doc.transact_mut();
        let mut changed = false;
        for field in &ty.document().fields {
            let value = fields.get(&field.key);
            if let Some(text) = texts.get(&field.key) {
                let new = match value {
                    Some(Value::Markdown(s) | Value::Text(s)) => s.as_str(),
                    _ => "",
                };
                let old = text.get_string(&txn);
                if old != new {
                    edit_text(text, &mut txn, &old, new);
                    changed = true;
                }
                continue;
            }
            let old = map
                .get(&txn, &field.key)
                .and_then(|out| json_of(&out.to_json(&txn)).ok())
                .filter(|json| !json.is_null());
            let new = value.map(Value::to_json);
            if old == new {
                continue;
            }
            changed = true;
            match new {
                Some(json) => {
                    let any: Any = serde_json::from_value(json).unwrap_or(Any::Null);
                    map.insert(&mut txn, field.key.as_str(), any);
                }
                None => {
                    map.remove(&mut txn, &field.key);
                }
            }
        }
        changed.then(|| txn.encode_update_v1())
    }

    /// The draft's state vector, as a Yjs v1 sync step 1 carries it.
    #[must_use]
    pub fn state_vector(&self) -> Vec<u8> {
        use yrs::updates::encoder::Encode;
        self.doc.transact().state_vector().encode_v1()
    }

    /// What a client whose state vector is `remote` lacks of the draft, as
    /// one Yjs v1 update: a sync step 2.
    pub fn missing_from(&self, remote: &[u8]) -> Result<Vec<u8>, String> {
        let remote = StateVector::decode_v1(remote).map_err(|e| e.to_string())?;
        Ok(self.doc.transact().encode_state_as_update_v1(&remote))
    }

    /// The keys of the draft's map that aren't fields of `ty` kept there.
    #[must_use]
    pub fn stray_keys(&self, ty: &AktType) -> Vec<String> {
        let (texts, map) = self.roots(ty);
        let txn = self.doc.transact();
        map.keys(&txn)
            .filter(|key| ty.field(key).is_none() || texts.contains_key(*key))
            .map(ToOwned::to_owned)
            .collect()
    }

    /// Makes the draft hold `fields` of `ty` again, as [`LiveDraft::write`]
    /// does, also taking out of its map what isn't a field kept there: the
    /// update that undoes a refused one, `None` if nothing differed.
    pub fn restore(&mut self, ty: &AktType, fields: &BTreeMap<String, Value>) -> Option<Vec<u8>> {
        let stray = self.stray_keys(ty);
        let mut updates: Vec<Vec<u8>> = Vec::new();
        if !stray.is_empty() {
            let map = self.doc.get_or_insert_map(FIELDS);
            let mut txn = self.doc.transact_mut();
            for key in &stray {
                map.remove(&mut txn, key);
            }
            updates.push(txn.encode_update_v1());
        }
        updates.extend(self.write(ty, fields));
        match updates.len() {
            0 => None,
            1 => updates.pop(),
            _ => merge(&updates).ok(),
        }
    }

    /// The draft's text roots of `ty`, by field key, and its map.
    fn roots(&self, ty: &AktType) -> (BTreeMap<String, TextRef>, MapRef) {
        let texts = ty
            .document()
            .fields
            .iter()
            .filter(|f| is_text(f))
            .map(|f| (f.key.clone(), self.doc.get_or_insert_text(f.key.as_str())))
            .collect();
        (texts, self.doc.get_or_insert_map(FIELDS))
    }
}

/// The field keys whose values differ between `before` and `after`.
#[must_use]
pub fn changed_keys(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) -> Vec<String> {
    let mut keys: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|k| before.get(*k) != after.get(*k))
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Whether `update`, a Yjs v1 update, holds nothing: what a client in step
/// with its server sends as its sync step 2. One that can't be read isn't.
#[must_use]
pub fn is_empty(update: &[u8]) -> bool {
    yrs::Update::decode_v1(update).is_ok_and(|u| u.is_empty())
}

/// `updates`, Yjs v1 updates, as one.
pub fn merge(updates: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    yrs::merge_updates_v1(updates.iter().map(Vec::as_slice)).map_err(|e| e.to_string())
}

/// Checks `after`, the fields an update to the live draft of `current`
/// leaves, as a write would check them (`live.edit`), and answers the
/// outcome: `after` with each box it newly checks checked by `change`, a
/// box checked before kept as it was, and the verdicts of assignees it
/// removed taken back. A verdict is given by a write, which checks who
/// gives it, never by an update. Headings keep the ids they have, the next
/// write giving them theirs, so that typing a heading isn't interrupted
/// (`akt.field.markdown`).
// r[impl live.edit]
pub fn check(
    ty: &AktType,
    current: &Current,
    after: &BTreeMap<String, Value>,
    change: Change,
) -> Result<Outcome, Refusal> {
    crate::refuse_final(ty, current)?;
    let before = &current.fields;
    let mut fields = after.clone();
    for field in &ty.document().fields {
        let key = &field.key;
        let was = before.get(key);
        let now = fields.get(key);
        if was == now {
            continue;
        }
        match field.kind {
            // r[impl akt.field.checkbox]
            FieldKind::Checkbox => match (was, now) {
                (Some(was @ Value::Checkbox { .. }), Some(_)) => {
                    let was = was.clone();
                    fields.insert(key.clone(), was);
                }
                (_, Some(Value::Checkbox { on_behalf_of, .. })) => {
                    let on_behalf_of =
                        (*on_behalf_of).filter(|s| *s != crate::Someone::User(change.by));
                    fields.insert(
                        key.clone(),
                        Value::Checkbox {
                            by: change.by,
                            at: change.at,
                            on_behalf_of,
                        },
                    );
                }
                _ => {}
            },
            // r[impl akt.field.verdict]
            FieldKind::Verdict => {
                return Err(Refusal::InvalidValue {
                    field: key.clone(),
                    reason: "a verdict is given by editing the akt, not its live draft".to_owned(),
                });
            }
            _ => {}
        }
    }
    // r[impl akt.field.required]
    if let Some(missing) = ty
        .document()
        .fields
        .iter()
        .find(|f| f.required && !fields.contains_key(&f.key))
    {
        return Err(Refusal::Required(missing.key.clone()));
    }
    // An assignee removed loses their verdict in the same change.
    for def in ty
        .document()
        .fields
        .iter()
        .filter(|f| f.kind == FieldKind::Verdict)
    {
        let given = crate::verdicts_of(&fields, &def.key);
        let kept = crate::verdicts::only_of(&given, &crate::assignees_of(&fields, def));
        if kept != given {
            if kept.is_empty() {
                fields.remove(&def.key);
            } else {
                fields.insert(def.key.clone(), Value::Verdicts(kept));
            }
        }
    }
    check_heading_ids(ty, &fields)?;
    let changes = changed_keys(before, &fields)
        .into_iter()
        .map(|key| {
            let value = fields.get(&key).cloned();
            (key, value)
        })
        .collect();
    Ok(crate::outcome(current.state.clone(), fields, changes))
}

/// Refuses headings of `fields` with ids that aren't ids, or that two of
/// them share, saying which field (`akt.field.markdown`).
// r[impl akt.field.markdown]
fn check_heading_ids(ty: &AktType, fields: &BTreeMap<String, Value>) -> Result<(), Refusal> {
    let mut texts: Vec<(String, String)> = ty
        .document()
        .fields
        .iter()
        .filter_map(|f| match fields.get(&f.key) {
            Some(Value::Markdown(text)) => Some((f.key.clone(), text.clone())),
            _ => None,
        })
        .collect();
    let invalid = |field: &str| {
        let field = field.to_owned();
        move |reason| Refusal::InvalidValue { field, reason }
    };
    for (key, text) in &texts {
        crate::markdown::give_ids(&mut [&mut text.clone()]).map_err(invalid(key))?;
    }
    let last = texts.last().map(|(key, _)| key.clone()).unwrap_or_default();
    let mut all: Vec<&mut String> = texts.iter_mut().map(|(_, text)| text).collect();
    crate::markdown::give_ids(&mut all).map_err(invalid(&last))
}

/// A Yjs value as JSON.
fn json_of(any: &Any) -> Result<Json, String> {
    serde_json::to_value(any).map_err(|e| e.to_string())
}

/// Turns `text`, which holds `old`, into `new` by deleting and inserting
/// only the characters that differ, last first so that each offset still
/// counts from the start of `old`.
fn edit_text(text: &TextRef, txn: &mut yrs::TransactionMut, old: &str, new: &str) {
    let diff = TextDiff::configure()
        .timeout(DIFF_TIMEOUT)
        .diff_chars(old, new);
    let old_at = byte_offsets(old);
    let new_at = byte_offsets(new);
    let offset = |at: &[usize], i: usize| u32::try_from(at[i]).unwrap_or(u32::MAX);
    for op in diff.ops().iter().rev() {
        let (old_index, old_len, new_index, new_len) = match *op {
            DiffOp::Equal { .. } => continue,
            DiffOp::Delete {
                old_index,
                old_len,
                new_index,
            } => (old_index, old_len, new_index, 0),
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => (old_index, 0, new_index, new_len),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => (old_index, old_len, new_index, new_len),
        };
        let start = offset(&old_at, old_index);
        if old_len > 0 {
            text.remove_range(txn, start, offset(&old_at, old_index + old_len) - start);
        }
        if new_len > 0 {
            text.insert(
                txn,
                start,
                &new[new_at[new_index]..new_at[new_index + new_len]],
            );
        }
    }
}

/// The byte offset of each character of `text`, and of its end.
fn byte_offsets(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Someone;
    use chrono::{TimeZone, Utc};
    use kuria_config::validate;
    use serde_json::json;
    use uuid::Uuid;

    fn document() -> AktType {
        validate(
            serde_json::from_value(json!({
                "key": "document", "name": "Document", "number": "D-{seq}",
                "fields": [
                    { "key": "title", "name": "Title", "kind": "text", "required": true },
                    { "key": "summary", "name": "Summary", "kind": "text", "multiline": true },
                    { "key": "body", "name": "Body", "kind": "markdown" },
                    { "key": "owner", "name": "Owner", "kind": "user" },
                    { "key": "approved", "name": "Approved", "kind": "checkbox" },
                    { "key": "tags", "name": "Tags", "kind": "choices",
                      "options": [{ "key": "a", "name": "A" }, { "key": "b", "name": "B" }] }
                ],
                "states": [{ "key": "draft", "name": "Draft", "initial": true }],
                "versions": {},
                "live_drafts": true
            }))
            .unwrap(),
        )
        .unwrap()
    }

    fn fields() -> BTreeMap<String, Value> {
        let by = Uuid::from_u128(7);
        BTreeMap::from([
            ("title".to_owned(), Value::Text("Guide".to_owned())),
            ("summary".to_owned(), Value::Text("Two\nlines".to_owned())),
            (
                "body".to_owned(),
                Value::Markdown("# Intro {#intro}\n\nHello, wörld.\n".to_owned()),
            ),
            ("owner".to_owned(), Value::User(Someone::User(by))),
            (
                "approved".to_owned(),
                Value::Checkbox {
                    by,
                    at: Utc.with_ymd_and_hms(2026, 11, 15, 12, 0, 0).unwrap(),
                    on_behalf_of: None,
                },
            ),
            (
                "tags".to_owned(),
                Value::Choices(vec!["b".to_owned(), "a".to_owned()]),
            ),
        ])
    }

    fn text_of(draft: &LiveDraft, key: &str) -> String {
        let text = draft.doc.get_or_insert_text(key);
        text.get_string(&draft.doc.transact())
    }

    // r[verify live.fields]
    #[test]
    fn a_draft_holds_texts_as_texts_and_the_rest_in_the_map() {
        let ty = document();
        let draft = LiveDraft::of(&ty, &fields());
        assert_eq!(draft.fields(&ty).unwrap(), fields());
        assert_eq!(text_of(&draft, "summary"), "Two\nlines");
        assert_eq!(
            text_of(&draft, "body"),
            "# Intro {#intro}\n\nHello, wörld.\n"
        );
        let map = draft.doc.get_or_insert_map(FIELDS);
        let txn = draft.doc.transact();
        let mut keys: Vec<String> = map.keys(&txn).map(ToOwned::to_owned).collect();
        keys.sort();
        assert_eq!(keys, ["approved", "owner", "tags", "title"]);
        assert_eq!(
            json_of(&map.get(&txn, "tags").unwrap().to_json(&txn)).unwrap(),
            json!(["b", "a"])
        );
    }

    // r[verify live.fields]
    #[test]
    fn a_draft_survives_its_encoding() {
        let ty = document();
        let draft = LiveDraft::of(&ty, &fields());
        let loaded = LiveDraft::load(&draft.state(), []).unwrap();
        assert_eq!(loaded.fields(&ty).unwrap(), fields());
        // An empty draft has no values.
        assert!(
            LiveDraft::load(&[], [])
                .unwrap()
                .fields(&ty)
                .unwrap()
                .is_empty()
        );
    }

    // r[verify live.fields]
    #[test]
    fn writing_changes_only_what_differs() {
        let ty = document();
        let mut draft = LiveDraft::of(&ty, &fields());
        let before = draft.state();
        assert_eq!(draft.write(&ty, &fields()), None);
        let mut edited = fields();
        edited.remove("summary");
        edited.remove("owner");
        edited.insert(
            "body".to_owned(),
            Value::Markdown("# Intro {#intro}\n\nHello, wörld!\n".to_owned()),
        );
        let update = draft.write(&ty, &edited).unwrap();
        assert_eq!(draft.fields(&ty).unwrap(), edited);
        assert_eq!(text_of(&draft, "summary"), "");
        assert_eq!(
            changed_keys(&fields(), &edited),
            ["body", "owner", "summary"]
        );
        // The update alone carries the edit to the draft as it was.
        let copy = LiveDraft::load(&before, [update.as_slice()]).unwrap();
        assert_eq!(copy.fields(&ty).unwrap(), edited);
    }

    // r[verify live.state]
    #[test]
    fn a_draft_made_from_fields_reads_alike_each_time() {
        let ty = document();
        let (mut one, other) = (LiveDraft::of(&ty, &fields()), LiveDraft::of(&ty, &fields()));
        // The same items, by the same client: a map's object may encode its
        // keys in another order.
        assert_eq!(one.fields(&ty).unwrap(), other.fields(&ty).unwrap());
        assert!(other.is_at(&one.base()).unwrap());
        // An update made against one applies to the other, and moves it on,
        // though it only deletes and leaves the state vector as it was.
        let mut edited = fields();
        edited.remove("summary");
        let update = one.write(&ty, &edited).unwrap();
        assert_eq!(one.state_vector(), other.state_vector());
        assert!(!one.is_at(&other.base()).unwrap());
        let mut other = other;
        other.apply(&update).unwrap();
        assert_eq!(other.fields(&ty).unwrap(), edited);
        assert!(other.is_at(&one.base()).unwrap());
        assert!(other.is_at(b"\xff").is_err());
    }

    /// A draft and a copy of it, as two clients have it.
    fn two(ty: &AktType, fields: &BTreeMap<String, Value>) -> (LiveDraft, LiveDraft) {
        let first = LiveDraft::of(ty, fields);
        let second = LiveDraft::load(&first.state(), []).unwrap();
        (first, second)
    }

    fn with_body(body: &str) -> BTreeMap<String, Value> {
        let mut fields = fields();
        fields.insert("body".to_owned(), Value::Markdown(body.to_owned()));
        fields
    }

    // r[verify live.fields]
    #[test]
    fn concurrent_edits_of_one_text_merge() {
        let ty = document();
        let base = "# Intro {#intro}\n\nOne paragraph.\n\nAnother paragraph.\n";
        let (mut a, mut b) = two(&ty, &with_body(base));
        let from_a = a
            .write(
                &ty,
                &with_body("# Intro {#intro}\n\nOne short paragraph.\n\nAnother paragraph.\n"),
            )
            .unwrap();
        let from_b = b
            .write(
                &ty,
                &with_body("# Intro {#intro}\n\nOne paragraph.\n\nAnother paragraph, ünïcode.\n"),
            )
            .unwrap();
        a.apply(&from_b).unwrap();
        b.apply(&from_a).unwrap();
        let merged =
            with_body("# Intro {#intro}\n\nOne short paragraph.\n\nAnother paragraph, ünïcode.\n");
        assert_eq!(a.fields(&ty).unwrap(), merged);
        assert_eq!(b.fields(&ty).unwrap(), merged);
    }

    // r[verify live.fields]
    #[test]
    fn concurrent_values_in_the_map_keep_one_and_other_fields_both() {
        let ty = document();
        let (mut a, mut b) = two(&ty, &fields());
        let mut from_a = fields();
        from_a.insert("title".to_owned(), Value::Text("A's".to_owned()));
        let mut from_b = fields();
        from_b.insert("title".to_owned(), Value::Text("B's".to_owned()));
        from_b.insert("tags".to_owned(), Value::Choices(vec!["a".to_owned()]));
        let ua = a.write(&ty, &from_a).unwrap();
        let ub = b.write(&ty, &from_b).unwrap();
        a.apply(&ub).unwrap();
        b.apply(&ua).unwrap();
        let read_a = a.fields(&ty).unwrap();
        // Both agree on one title, the last writer's as Yjs orders them,
        // and B's tags, which A didn't touch.
        assert_eq!(read_a, b.fields(&ty).unwrap());
        assert!(matches!(&read_a["title"], Value::Text(t) if t == "A's" || t == "B's"));
        assert_eq!(read_a["tags"], Value::Choices(vec!["a".to_owned()]));
    }

    // r[verify live.fields]
    #[test]
    fn a_value_that_doesnt_fit_its_field_is_refused() {
        let ty = document();
        let draft = LiveDraft::new();
        {
            let map = draft.doc.get_or_insert_map(FIELDS);
            let mut txn = draft.doc.transact_mut();
            map.insert(&mut txn, "tags", Any::from(vec!["c".to_owned()]));
        }
        assert!(draft.fields(&ty).unwrap_err().starts_with("`tags`: "));
    }

    fn current() -> Current {
        Current {
            state: "draft".to_owned(),
            fields: fields(),
        }
    }

    fn change() -> Change {
        Change {
            by: Uuid::from_u128(9),
            at: Utc.with_ymd_and_hms(2026, 11, 16, 9, 0, 0).unwrap(),
        }
    }

    // r[verify live.edit]
    #[test]
    fn an_update_is_checked_as_a_write_would_be() {
        let ty = document();
        let mut after = fields();
        after.remove("title");
        assert_eq!(
            check(&ty, &current(), &after, change()),
            Err(Refusal::Required("title".to_owned()))
        );
        let mut after = fields();
        after.insert(
            "body".to_owned(),
            Value::Markdown("# A {#x}\n\n# B {#x}\n".to_owned()),
        );
        assert!(matches!(
            check(&ty, &current(), &after, change()),
            Err(Refusal::InvalidValue { field, .. }) if field == "body"
        ));
        // Headings without ids keep none until a write gives them.
        let mut after = fields();
        after.insert("body".to_owned(), Value::Markdown("# New\n".to_owned()));
        let outcome = check(&ty, &current(), &after, change()).unwrap();
        assert_eq!(outcome.fields, after);
        assert_eq!(outcome.changes.keys().collect::<Vec<_>>(), ["body"]);
    }

    // r[verify live.edit]
    #[test]
    fn a_box_an_update_checks_is_checked_by_whoever_sent_it() {
        let ty = document();
        let mut before = current();
        before.fields.remove("approved");
        let posed = Value::Checkbox {
            by: Uuid::from_u128(1),
            at: Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap(),
            on_behalf_of: None,
        };
        let mut after = before.fields.clone();
        after.insert("approved".to_owned(), posed.clone());
        let outcome = check(&ty, &before, &after, change()).unwrap();
        assert_eq!(
            outcome.fields["approved"],
            Value::Checkbox {
                by: change().by,
                at: change().at,
                on_behalf_of: None
            }
        );
        // One checked before stays as it was checked.
        let mut after = fields();
        after.insert("approved".to_owned(), posed);
        let outcome = check(&ty, &current(), &after, change()).unwrap();
        assert_eq!(outcome.fields, fields());
        assert!(outcome.changes.is_empty());
    }

    // r[verify live.edit]
    #[test]
    fn restoring_undoes_a_refused_update_for_everyone() {
        let ty = document();
        let (mut server, mut client) = two(&ty, &fields());
        let mut bad = fields();
        bad.remove("title");
        let refused = client.write(&ty, &bad).unwrap();
        {
            let map = client.doc.get_or_insert_map(FIELDS);
            let mut txn = client.doc.transact_mut();
            map.insert(&mut txn, "stray", Any::from(1));
        }
        let refused = merge(&[refused, client.state()]).unwrap();
        server.apply(&refused).unwrap();
        assert_eq!(server.stray_keys(&ty), ["stray"]);
        let undo = server.restore(&ty, &fields()).unwrap();
        assert_eq!(server.fields(&ty).unwrap(), fields());
        assert_eq!(server.stray_keys(&ty), Vec::<String>::new());
        client.apply(&undo).unwrap();
        assert_eq!(client.fields(&ty).unwrap(), fields());
        assert_eq!(client.stray_keys(&ty), Vec::<String>::new());
        // What a client lacks is what brings it level.
        let mut fresh = LiveDraft::new();
        fresh
            .apply(&server.missing_from(&fresh.state_vector()).unwrap())
            .unwrap();
        assert_eq!(fresh.fields(&ty).unwrap(), fields());
    }
}
