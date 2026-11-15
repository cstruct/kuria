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

use diesel_async::AsyncConnection;

use super::{AktError, Caller, StoredType};
use crate::authorize::Permission;

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
        None,
    )
    .await?;
    if loaded.pending + 1 >= COMPACT_AFTER {
        stored::compact(conn, akt.tenant_id, akt.id, &loaded.draft.state(), seq).await?;
    }
    Ok(after)
}

/// What an update a client sent to a live draft did (`live.sync`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Taken {
    /// It changed nothing: the draft already had it.
    Unchanged,
    /// It was checked and kept, as the draft's update `seq`.
    Accepted { seq: i64 },
    /// It was refused for `reason`, and kept with what undoes it, as the
    /// draft's update `seq`, so that every client undoes it too.
    Refused { seq: i64, reason: String },
}

/// An akt with a live draft, as a client of it opens it.
#[derive(Clone, Debug)]
pub struct Opened {
    pub akt: Uuid,
    pub number: String,
    /// Whether the subject may edit it: else their updates aren't taken.
    pub may_edit: bool,
}

/// The akt `number`, if it has a live draft and the caller may read it
/// (`live.sync`); one without, or that they may not read, doesn't exist for
/// them.
// r[impl live.sync]
pub(crate) async fn open_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    number: &str,
) -> Result<Opened, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let akt = kuria_store::akter::akt_by_number(conn, tenant, number)
        .await?
        .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
    let ty = super::type_of(conn, tenant, &akt).await?;
    let key = ty.ty.key().to_owned();
    if !has_live_draft(&ty)
        || !caller
            .auth
            .allows(&Permission::Read { key: key.clone() })
            .await?
    {
        return Err(AktError::NotFound(number.to_owned()));
    }
    Ok(Opened {
        akt: akt.id,
        number: akt.number,
        may_edit: caller.auth.allows(&Permission::Edit { key }).await?,
    })
}

/// The live draft of the akt `number` as stored now, for a client to sync
/// with.
pub(crate) async fn draft_in(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    number: &str,
) -> Result<LiveDraft, AktError> {
    let row = kuria_store::akter::akt_by_number(conn, tenant, number)
        .await?
        .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
    let ty = super::type_of(conn, tenant, &row).await?;
    Ok(load(conn, &ty, &row).await?.draft)
}

/// Takes `update`, sent by `caller` to the live draft of the akt `number`:
/// applies it, checks the fields it leaves as a write would be checked,
/// and keeps it, with who sent it; or, refused, keeps it with the update
/// that undoes it (`live.sync`, `live.edit`, `live.history`).
// r[impl live.sync]
// r[impl live.edit]
// r[impl live.history]
pub(crate) async fn take_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    number: &str,
    update: &[u8],
) -> Result<Taken, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let mut akt = kuria_store::akter::lock_akt_by_number(conn, tenant, number)
        .await?
        .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
    let ty = super::type_of(conn, tenant, &akt).await?;
    let key = ty.ty.key().to_owned();
    if !has_live_draft(&ty)
        || !caller
            .auth
            .allows(&Permission::Read { key: key.clone() })
            .await?
    {
        return Err(AktError::NotFound(number.to_owned()));
    }
    if !caller.auth.allows(&Permission::Edit { key }).await? {
        return Err(AktError::Forbidden);
    }
    let now = Utc::now();
    let by: Uuid = caller.actor.subject.into();
    let mut loaded = load(conn, &ty, &akt).await?;
    if !loaded.stored {
        store_new(conn, &ty, &akt, &loaded.draft, now).await?;
    }
    let before = loaded.fields.clone();
    let state = loaded.draft.state();
    loaded
        .draft
        .apply(update)
        .map_err(|e| AktError::BadRequest(format!("not a Yjs update: {e}")))?;
    if loaded.draft.state() == state {
        return Ok(Taken::Unchanged);
    }
    let checked = match checked(conn, caller, &ty, &akt, &loaded, now).await {
        Ok(outcome) => Ok(outcome),
        Err(e @ (AktError::Internal(_) | AktError::Unavailable(_))) => return Err(e),
        Err(refused) => Err(refused.to_string()),
    };
    let (kept, fields, keys) = match &checked {
        Ok(outcome) => {
            let mut kept = vec![update.to_vec()];
            // What the checks changed, such as who checked a box, follows.
            kept.extend(loaded.draft.write(&ty.ty, &outcome.fields));
            let after = loaded
                .draft
                .fields(&ty.ty)
                .map_err(|e| AktError::Internal(format!("akt {number}'s live draft: {e}")))?;
            let keys = changed_keys(&before, &after);
            (kept, after, keys)
        }
        Err(_) => {
            let mut kept = vec![update.to_vec()];
            kept.extend(loaded.draft.restore(&ty.ty, &before));
            (kept, before, Vec::new())
        }
    };
    let kept = kuria_engine::live::merge(&kept).map_err(AktError::Internal)?;
    let seq = stored::append(
        conn,
        tenant,
        akt.id,
        &kept,
        by,
        now,
        &keys,
        &Json::Object(kuria_engine::fields_to_json(&fields)),
        None,
    )
    .await?;
    if loaded.pending + 1 >= COMPACT_AFTER {
        stored::compact(conn, tenant, akt.id, &loaded.draft.state(), seq).await?;
    }
    match checked {
        Ok(_) => {
            akt.title = super::title_of(&ty.ty, &fields);
            akt.updated_at = now;
            if !kuria_store::akter::update_akt(conn, &akt, akt.revision).await? {
                return Err(AktError::Internal(format!(
                    "akt {number} moved while locked"
                )));
            }
            Ok(Taken::Accepted { seq })
        }
        Err(reason) => Ok(Taken::Refused { seq, reason }),
    }
}

/// Takes `update`, which `caller` made against `base`, the live draft of
/// the akt `number` as their clone read it ([`LiveDraft::base`]), and
/// pushes with `message` (`clone.server`): refused as stale unless `base`
/// is the draft's now; applied, checked as a live edit, its diagrams
/// rendered as a write's are, and kept as one change by the caller with
/// the message. A refusal leaves the caller's transaction to undo
/// everything. Answers the akt as reading it then does.
// r[impl clone.server]
// r[impl live.edit]
// r[impl live.history]
pub(crate) async fn push_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    number: &str,
    base: &[u8],
    update: &[u8],
    message: &str,
) -> Result<AktRow, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let mut akt = kuria_store::akter::lock_akt_by_number(conn, tenant, number)
        .await?
        .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
    let ty = super::type_of(conn, tenant, &akt).await?;
    let key = ty.ty.key().to_owned();
    if !caller
        .auth
        .allows(&Permission::Read { key: key.clone() })
        .await?
    {
        return Err(AktError::NotFound(number.to_owned()));
    }
    if !has_live_draft(&ty) {
        return Err(AktError::BadRequest(format!(
            "{number} has no live draft: push its edits as operations"
        )));
    }
    if !caller.auth.allows(&Permission::Edit { key }).await? {
        return Err(AktError::Forbidden);
    }
    let now = Utc::now();
    let mut loaded = load(conn, &ty, &akt).await?;
    let current = loaded
        .draft
        .is_at(base)
        .map_err(|e| AktError::BadRequest(format!("{number}'s base isn't a Yjs snapshot: {e}")))?;
    if !current {
        return Err(AktError::StaleDraft {
            number: number.to_owned(),
            base: loaded.draft.base(),
            state_vector: loaded.draft.state_vector(),
        });
    }
    if !loaded.stored {
        store_new(conn, &ty, &akt, &loaded.draft, now).await?;
    }
    let before = loaded.fields.clone();
    let state = loaded.draft.state();
    loaded
        .draft
        .apply(update)
        .map_err(|e| AktError::BadRequest(format!("{number}'s update isn't a Yjs update: {e}")))?;
    if loaded.draft.state() == state {
        return Ok(with_fields(&ty, &akt, &before));
    }
    let outcome = checked(conn, caller, &ty, &akt, &loaded, now).await?;
    super::diagrams::render_written(conn, caller, &ty.ty, &akt, &outcome).await?;
    let mut kept = vec![update.to_vec()];
    // What the checks changed, such as who checked a box, follows.
    kept.extend(loaded.draft.write(&ty.ty, &outcome.fields));
    let kept = kuria_engine::live::merge(&kept).map_err(AktError::Internal)?;
    let after = loaded
        .draft
        .fields(&ty.ty)
        .map_err(|e| AktError::Internal(format!("akt {number}'s live draft: {e}")))?;
    let keys = changed_keys(&before, &after);
    let seq = stored::append(
        conn,
        tenant,
        akt.id,
        &kept,
        caller.actor.subject.into(),
        now,
        &keys,
        &Json::Object(kuria_engine::fields_to_json(&after)),
        Some(message),
    )
    .await?;
    if loaded.pending + 1 >= COMPACT_AFTER {
        stored::compact(conn, tenant, akt.id, &loaded.draft.state(), seq).await?;
    }
    akt.title = super::title_of(&ty.ty, &after);
    akt.updated_at = now;
    if !kuria_store::akter::update_akt(conn, &akt, akt.revision).await? {
        return Err(AktError::Internal(format!(
            "akt {number} moved while locked"
        )));
    }
    // r[impl live.sync]
    kuria_store::akter::append_draft_change(conn, tenant, akt.id, akt.revision, now).await?;
    Ok(with_fields(&ty, &akt, &after))
}

/// The outcome of the live draft `loaded` of `akt` as an update left it,
/// checked as a write of it would be: by the engine, then the contacts,
/// sections and attachments it names, in a savepoint undone if refused.
/// Diagrams are rendered by the next write or release, as one being typed
/// doesn't compile yet.
async fn checked(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    ty: &StoredType,
    akt: &AktRow,
    loaded: &Loaded,
    now: DateTime<Utc>,
) -> Result<kuria_engine::Outcome, AktError> {
    if let Some(stray) = loaded.draft.stray_keys(&ty.ty).first() {
        return Err(AktError::BadRequest(format!(
            "the akt type has no field `{stray}` kept in `{}`",
            kuria_engine::live::FIELDS
        )));
    }
    let after = loaded.draft.fields(&ty.ty).map_err(AktError::BadRequest)?;
    let current = kuria_engine::Current {
        state: akt.state.clone(),
        fields: loaded.fields.clone(),
    };
    let change = kuria_engine::Change {
        by: caller.actor.subject.into(),
        at: now,
    };
    let outcome = kuria_engine::live::check(&ty.ty, &current, &after, change)?;
    let tenant = akt.tenant_id;
    conn.transaction::<_, AktError, _>(async |conn| {
        super::check_sections(conn, tenant, &ty.ty, Some(akt.id), &outcome).await?;
        super::check_contacts(conn, tenant, &outcome).await?;
        super::hold_attachments(conn, caller, &ty.ty, akt, &outcome).await
    })
    .await?;
    Ok(outcome)
}

/// Records that the live draft of the akt `number` changed, as a change event, so that
/// subscribers learn of it (`live.sync`).
pub(crate) async fn changed_in(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    number: &str,
) -> Result<(), AktError> {
    let Some(row) = kuria_store::akter::akt_by_number(conn, tenant, number).await? else {
        return Ok(());
    };
    kuria_store::akter::append_draft_change(conn, tenant, row.id, row.revision, Utc::now()).await?;
    Ok(())
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
