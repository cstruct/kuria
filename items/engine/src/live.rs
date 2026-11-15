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

use crate::Value;

/// The name of the `Y.Map` holding every field that isn't a text.
pub const FIELDS: &str = "fields";

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

    /// A live draft holding `fields` of `ty`, and the update that makes it
    /// from an empty one.
    #[must_use]
    pub fn of(ty: &AktType, fields: &BTreeMap<String, Value>) -> Self {
        let mut draft = Self::new();
        draft.write(ty, fields);
        draft
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
}
