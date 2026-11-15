//! Live drafts (`live.*`): loading an akt's live draft, writing fields to
//! it as Yjs updates, and what reading answers for it.
//!
//! An akt whose type says `"live_drafts": true` keeps its latest revision's
//! fields in its row, as every akt does, and its live draft beside it
//! (`akt_draft`, `akt_draft_update`). One without a stored draft yet, such
//! as one imported, has the draft its latest revision's fields make.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use kuria_engine::Value;
use kuria_engine::live::{LiveDraft, changed_keys};
use kuria_store::akter::AktRow;
use kuria_store::drafts::{self as stored, DraftRow};
use serde_json::Value as Json;
use uuid::Uuid;

use super::{AktError, StoredType};

/// How many updates a draft takes before its state is compacted.
const COMPACT_AFTER: usize = 64;

/// Whether akter of `ty` have a live draft (`live.declare`).
#[must_use]
pub fn has_live_draft(ty: &StoredType) -> bool {
    ty.ty.document().live_drafts
}

/// An akt's live draft, loaded, with what it reads as.
pub(crate) struct Loaded {
    pub draft: LiveDraft,
    pub fields: BTreeMap<String, Value>,
    /// Whether it is stored yet.
    stored: bool,
    /// Its last update's sequence number, 0 if it has none.
    pub last_seq: i64,
    /// How many updates came after its compacted state.
    pending: usize,
}

/// `akt`'s live draft, as stored, or as its fields make it if it has none
/// stored yet.
pub(crate) async fn load(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
) -> Result<Loaded, AktError> {
    let unreadable =
        |e: String| AktError::Internal(format!("akt {}'s live draft: {e}", akt.number));
    match stored::load(conn, akt.tenant_id, akt.id).await? {
        Some((row, updates)) => {
            let draft = LiveDraft::load(&row.state, updates.iter().map(|u| u.update.as_slice()))
                .map_err(unreadable)?;
            let fields = draft.fields(&ty.ty).map_err(unreadable)?;
            Ok(Loaded {
                draft,
                fields,
                stored: true,
                last_seq: updates.last().map_or(row.compacted_seq, |u| u.seq),
                pending: updates.len(),
            })
        }
        None => {
            let fields = super::versions::values_of(ty, akt)?;
            let draft = LiveDraft::of(&ty.ty, &fields);
            let fields = draft.fields(&ty.ty).map_err(unreadable)?;
            Ok(Loaded {
                draft,
                fields,
                stored: false,
                last_seq: 0,
                pending: 0,
            })
        }
    }
}

/// Starts `akt`'s live draft from `fields`, as a create does.
pub(crate) async fn start(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
    fields: &BTreeMap<String, Value>,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    let draft = LiveDraft::of(&ty.ty, fields);
    store_new(conn, ty, akt, &draft, now).await
}

async fn store_new(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
    draft: &LiveDraft,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    let fields = draft
        .fields(&ty.ty)
        .map_err(|e| AktError::Internal(format!("akt {}'s live draft: {e}", akt.number)))?;
    stored::insert(
        conn,
        &DraftRow {
            tenant_id: akt.tenant_id,
            akt_id: akt.id,
            state: draft.state(),
            fields: Json::Object(kuria_engine::fields_to_json(&fields)),
            compacted_seq: 0,
            released_seq: 0,
            updated_at: now,
        },
    )
    .await?;
    Ok(())
}

/// Writes `fields` to `akt`'s live draft `loaded`, as one update by `by`,
/// and answers what the draft then reads as (`live.edit`, `live.history`).
pub(crate) async fn write(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
    mut loaded: Loaded,
    fields: &BTreeMap<String, Value>,
    by: Uuid,
    now: DateTime<Utc>,
) -> Result<BTreeMap<String, Value>, AktError> {
    if !loaded.stored {
        store_new(conn, ty, akt, &loaded.draft, now).await?;
    }
    let Some(update) = loaded.draft.write(&ty.ty, fields) else {
        return Ok(loaded.fields);
    };
    let after = loaded
        .draft
        .fields(&ty.ty)
        .map_err(|e| AktError::Internal(format!("akt {}'s live draft: {e}", akt.number)))?;
    let keys = changed_keys(&loaded.fields, &after);
    let seq = stored::append(
        conn,
        akt.tenant_id,
        akt.id,
        &update,
        by,
        now,
        &keys,
        &Json::Object(kuria_engine::fields_to_json(&after)),
    )
    .await?;
    if loaded.pending + 1 >= COMPACT_AFTER {
        stored::compact(conn, akt.tenant_id, akt.id, &loaded.draft.state(), seq).await?;
    }
    Ok(after)
}

/// Records that a release took `akt`'s live draft `loaded` as it is, and
/// compacts it.
pub(crate) async fn released(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
    loaded: &Loaded,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    if !loaded.stored {
        store_new(conn, ty, akt, &loaded.draft, now).await?;
    }
    stored::compact(
        conn,
        akt.tenant_id,
        akt.id,
        &loaded.draft.state(),
        loaded.last_seq,
    )
    .await?;
    stored::released(conn, akt.tenant_id, akt.id, loaded.last_seq).await?;
    Ok(())
}

/// Takes `member` out of the sections of `akt`'s live draft, as when it
/// leaves the container (`akt.field.sections`).
pub(crate) async fn leave_sections(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
    member: &str,
    by: Uuid,
    now: DateTime<Utc>,
) -> Result<(), AktError> {
    let loaded = load(conn, ty, akt).await?;
    let mut fields = loaded.fields.clone();
    let mut changed = false;
    for field in ty
        .ty
        .document()
        .fields
        .iter()
        .filter(|f| f.kind == kuria_config::FieldKind::Sections)
    {
        if let Some(Value::Sections(items)) = fields.get(&field.key)
            && kuria_engine::sections::numbers(items).contains(&member)
        {
            let kept = kuria_engine::sections::without(items, member);
            fields.insert(field.key.clone(), Value::Sections(kept));
            changed = true;
        }
    }
    if changed {
        write(conn, ty, akt, loaded, &fields, by, now).await?;
    }
    Ok(())
}

/// `rows`, each akt with a live draft answering its live draft's fields
/// as its own (`live.read`).
pub async fn read_as_drafts(
    conn: &mut kuria_store::Connection,
    rows: &mut [(AktRow, &StoredType)],
) -> Result<(), AktError> {
    let ids: Vec<Uuid> = rows
        .iter()
        .filter(|(_, ty)| has_live_draft(ty))
        .map(|(row, _)| row.id)
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    let drafts: std::collections::HashMap<Uuid, Json> =
        stored::fields_of(conn, &ids).await?.into_iter().collect();
    for (row, ty) in rows.iter_mut() {
        if let Some(fields) = drafts.get(&row.id) {
            row.fields = fields.clone();
            let values = super::versions::values_of(ty, row)?;
            row.title = super::title_of(&ty.ty, &values);
        }
    }
    Ok(())
}

/// `akt` as reading answers it: with its live draft's fields if it has one
/// (`live.read`).
pub async fn read_as_draft(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: AktRow,
) -> Result<AktRow, AktError> {
    let mut rows = [(akt, ty)];
    read_as_drafts(conn, &mut rows).await?;
    let [(akt, _)] = rows;
    Ok(akt)
}

/// `akt`'s row answering `fields` as its fields, and the title they give.
pub(crate) fn with_fields(
    ty: &StoredType,
    akt: &AktRow,
    fields: &BTreeMap<String, Value>,
) -> AktRow {
    let mut row = akt.clone();
    row.fields = Json::Object(kuria_engine::fields_to_json(fields));
    row.title = super::title_of(&ty.ty, fields);
    row
}

/// The changes `before` to `after` make, as a revision records them.
pub(crate) fn changes(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) -> BTreeMap<String, Option<Value>> {
    changed_keys(before, after)
        .into_iter()
        .map(|key| {
            let value = after.get(&key).cloned();
            (key, value)
        })
        .collect()
}
