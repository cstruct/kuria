//! Versions (`version.*`): releasing an akt's current revision as its next
//! version, and reading an akt as it was at a revision.

use chrono::Utc;
use diesel_async::AsyncConnection;
use kuria_engine::markdown::{self, Target, TargetVersion};
use kuria_engine::versions;
use kuria_store::akter::{self as store, AktRow};
use kuria_store::containers;
use kuria_store::versions::{self as stored, VersionRow};
use serde_json::{Map, Value as Json};
use uuid::Uuid;

use super::{AktError, Akter, Caller, StoredType, require, require_read, type_of};
use crate::authorize::{Authorizer, Permission};

/// An akt's fields and state as they were at `revision`, replayed from its
/// revisions; `None` if it never reached it.
pub async fn at_revision(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    akt: Uuid,
    revision: i32,
) -> Result<Option<(Map<String, Json>, String)>, AktError> {
    let mut fields = Map::new();
    let mut state = None;
    for row in store::revisions(conn, tenant, akt).await? {
        if row.revision > revision {
            break;
        }
        if let Some(changes) = row.changes.as_object() {
            for (key, value) in changes {
                if value.is_null() {
                    fields.remove(key);
                } else {
                    fields.insert(key.clone(), value.clone());
                }
            }
        }
        state = Some(row.state.clone());
    }
    Ok(state.map(|state| (fields, state)))
}

/// `akt`'s row as it was at `revision`: its fields, state and title then;
/// `None` if it never reached it.
pub async fn at(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
    revision: i32,
) -> Result<Option<AktRow>, AktError> {
    let Some((fields, state)) = at_revision(conn, akt.tenant_id, akt.id, revision).await? else {
        return Ok(None);
    };
    let values = kuria_engine::fields_from_json(&ty.ty, &fields)
        .map_err(|e| AktError::Internal(format!("akt {}: {e}", akt.number)))?;
    let mut row = akt.clone();
    row.title = super::title_of(&ty.ty, &values);
    row.fields = Json::Object(fields);
    row.state = state;
    row.revision = revision;
    Ok(Some(row))
}

/// `akt` as it was at its version `number`: its row with the fields, state
/// and title of that revision, and the version; `None` if it has no such
/// version (`version.read`).
// r[impl version.read]
pub async fn at_version(
    conn: &mut kuria_store::Connection,
    ty: &StoredType,
    akt: &AktRow,
    number: i32,
) -> Result<Option<(AktRow, VersionRow)>, AktError> {
    let Some(version) = stored::one(conn, akt.tenant_id, akt.id, number).await? else {
        return Ok(None);
    };
    let Some((fields, state)) = at_revision(conn, akt.tenant_id, akt.id, version.revision).await?
    else {
        return Ok(None);
    };
    let values = kuria_engine::fields_from_json(&ty.ty, &fields)
        .map_err(|e| AktError::Internal(format!("akt {}: {e}", akt.number)))?;
    let mut row = akt.clone();
    row.title = super::title_of(&ty.ty, &values);
    row.fields = Json::Object(fields);
    row.state = state;
    row.revision = version.revision;
    Ok(Some((row, version)))
}

/// `akt`'s field values, as its row holds them.
pub(crate) fn values_of(
    ty: &StoredType,
    akt: &AktRow,
) -> Result<std::collections::BTreeMap<String, kuria_engine::Value>, AktError> {
    kuria_engine::fields_from_json(&ty.ty, akt.fields.as_object().unwrap_or(&Map::new()))
        .map_err(|e| AktError::Internal(format!("akt {}: {e}", akt.number)))
}

/// The akt type version `akt` was created with.
pub async fn type_of_akt(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    akt: &AktRow,
) -> Result<StoredType, AktError> {
    type_of(conn, tenant, akt).await
}

/// The picture an akt of `ty` with `fields` shows, if its type names one
/// and it has one (`akt.field.markdown.images`).
#[must_use]
pub fn picture_in(ty: &StoredType, fields: &Map<String, Json>) -> Option<Uuid> {
    let key = ty.ty.document().presentation.image.as_ref()?;
    let field = ty.ty.field(key)?;
    match kuria_engine::Value::from_json(field, fields.get(key)?).ok()? {
        kuria_engine::Value::Attachment(id) => Some(id),
        _ => None,
    }
}

/// The picture the akt `number` shows at its version `version`, else at
/// `revision`, else now, if it has one then; whether the reader may read
/// it is [`target`]'s to say.
pub async fn picture_at(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    number: &str,
    version: Option<u32>,
    revision: Option<i32>,
) -> Result<Option<Uuid>, AktError> {
    let Some(akt) = store::akt_by_number(conn, tenant, number).await? else {
        return Ok(None);
    };
    let ty = type_of(conn, tenant, &akt).await?;
    let revision = match version {
        Some(n) => {
            match stored::one(conn, tenant, akt.id, i32::try_from(n).unwrap_or(i32::MAX)).await? {
                Some(v) => Some(v.revision),
                None => return Ok(None),
            }
        }
        None => revision,
    };
    let fields = match revision {
        Some(r) => match at_revision(conn, tenant, akt.id, r).await? {
            Some((fields, _)) => fields,
            None => return Ok(None),
        },
        None => akt.fields.as_object().cloned().unwrap_or_default(),
    };
    Ok(picture_in(&ty, &fields))
}

/// What the akt `number` has for a reference to it, as a subject `auth`
/// allows to read it: `None` if it doesn't exist or they may not read it.
pub async fn target(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    auth: &Authorizer,
    number: &str,
) -> Result<Option<Target>, AktError> {
    let Some(akt) = store::akt_by_number(conn, tenant, number).await? else {
        return Ok(None);
    };
    let ty = type_of(conn, tenant, &akt).await?;
    if require_read(auth, &ty, number).await.is_err() {
        return Ok(None);
    }
    // r[impl live.read]: its living version is its live draft, if it has one.
    let akt = super::live::read_as_draft(conn, &ty, akt).await?;
    let headings = |ty: &StoredType, fields: &Map<String, Json>| -> Result<Vec<String>, AktError> {
        let values = kuria_engine::fields_from_json(&ty.ty, fields)
            .map_err(|e| AktError::Internal(format!("akt {number}: {e}")))?;
        Ok(markdown::heading_ids(&ty.ty, &values))
    };
    let now = akt.fields.as_object().cloned().unwrap_or_default();
    let mut found = Target {
        headings: headings(&ty, &now)?,
        versions: Vec::new(),
        revision: akt.revision,
        image: ty.ty.document().presentation.image.is_some(),
        picture: picture_in(&ty, &now).is_some(),
        versioned: ty.ty.document().versions.is_some(),
    };
    let mut rows = stored::of(conn, tenant, akt.id).await?;
    rows.reverse();
    for version in rows {
        let at = at_revision(conn, tenant, akt.id, version.revision)
            .await?
            .map(|(fields, _)| fields)
            .unwrap_or_default();
        found.versions.push(TargetVersion {
            number: u32::try_from(version.number).unwrap_or_default(),
            revision: version.revision,
            headings: headings(&ty, &at)?,
            picture: picture_in(&ty, &at).is_some(),
        });
    }
    Ok(Some(found))
}

/// One akt to release: its number, the revision the releaser read, and
/// an optional label and description.
#[derive(Clone, Debug)]
pub struct Releasing {
    pub number: String,
    pub revision: i32,
    pub label: Option<String>,
    /// What the releaser says of the version, such as what changed.
    pub description: Option<String>,
    /// The PDF a type whose versions carry a file is released with
    /// (`version.release-file`).
    pub file: Option<Uuid>,
}

impl Akter {
    /// Releases the akt `releasing` names at its current revision, which
    /// must be the one it read, as its next version, and returns the
    /// version.
    pub async fn release(
        &self,
        caller: Caller<'_>,
        releasing: Releasing,
    ) -> Result<VersionRow, AktError> {
        let releasing = [releasing];
        let mut rows = self.release_together(caller, &releasing).await?;
        rows.pop()
            .ok_or_else(|| AktError::Internal("a release answered no version".to_owned()))
    }

    /// Releases each of `releasing` as its next version, all or nothing,
    /// a reference from one to another checked against what that one is
    /// released as (`version.together`); answers the versions in order.
    // r[impl version.release]
    // r[impl version.number]
    // r[impl version.together]
    // r[impl authz.release]
    pub async fn release_together(
        &self,
        caller: Caller<'_>,
        releasing: &[Releasing],
    ) -> Result<Vec<VersionRow>, AktError> {
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| release_in(conn, caller, releasing).await)
            .await;
        self.committed(result)
    }
}

/// An akt about to be released: what it is released as, and the version
/// it gets.
struct Prepared {
    akt: AktRow,
    ty: StoredType,
    revision: i32,
    label: Option<String>,
    description: Option<String>,
    draft: Option<super::live::Loaded>,
    /// Whether the release records a revision: its live draft's, or one
    /// setting or clearing its file (`version.release-file`).
    records: bool,
    fields: std::collections::BTreeMap<String, kuria_engine::Value>,
    next: i32,
}

async fn release_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    releasing: &[Releasing],
) -> Result<Vec<VersionRow>, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    if releasing.is_empty() {
        return Err(AktError::BadRequest("name an akt to release".to_owned()));
    }
    let numbers: Vec<&str> = releasing.iter().map(|r| r.number.as_str()).collect();
    if let Some(twice) = numbers
        .iter()
        .enumerate()
        .find(|(i, n)| numbers[..*i].contains(n))
    {
        return Err(AktError::BadRequest(format!("{} is named twice", twice.1)));
    }
    let mut locked = containers::lock_akter(conn, tenant, &numbers).await?;
    let mut prepared = Vec::new();
    for one in releasing {
        let at = locked
            .iter()
            .position(|a| a.number == one.number)
            .ok_or_else(|| AktError::NotFound(one.number.clone()))?;
        let akt = locked.swap_remove(at);
        prepared.push(prepare(conn, caller, akt, one).await?);
    }
    // The targets first, since the check can't wait on the database; each
    // akt released here as what it is released as, at the version it gets.
    let mut targets = std::collections::HashMap::new();
    for one in &prepared {
        let mut found = target(conn, tenant, caller.auth, &one.akt.number).await?;
        if let Some(found) = &mut found {
            let json = kuria_engine::fields_to_json(&one.fields);
            found.versions.push(TargetVersion {
                number: u32::try_from(one.next).unwrap_or_default(),
                revision: one.akt.revision + i32::from(one.records),
                headings: markdown::heading_ids(&one.ty.ty, &one.fields),
                picture: picture_in(&one.ty, &json).is_some(),
            });
        }
        targets.insert(one.akt.number.clone(), found);
    }
    for one in &prepared {
        for (_, reference) in versions::references_of(&one.ty.ty, &one.fields) {
            if !targets.contains_key(&reference.number) {
                let found = target(conn, tenant, caller.auth, &reference.number).await?;
                targets.insert(reference.number.clone(), found);
            }
        }
    }
    let mut pins = Vec::new();
    let mut broken = Vec::new();
    let mut together: Vec<String> = Vec::new();
    for one in &prepared {
        match versions::check(&one.ty.ty, &one.fields, &|n| {
            targets.get(n).cloned().flatten()
        }) {
            Ok(found) => pins.push(found),
            Err(found) => {
                for b in found {
                    broken.push(if prepared.len() > 1 {
                        format!("{}: {b}", one.akt.number)
                    } else {
                        b.to_string()
                    });
                    if let Some(number) = b.together
                        && !together.contains(&number)
                    {
                        together.push(number);
                    }
                }
            }
        }
    }
    if !broken.is_empty() {
        return Err(AktError::BrokenReferences { broken, together });
    }
    let mut rows = Vec::new();
    for (one, pins) in prepared.into_iter().zip(pins) {
        rows.push(record_version(conn, caller, one, pins).await?);
    }
    Ok(rows)
}

/// Stores `one` as the version it gets, with `pins`, after recording the
/// revision it names, if the release makes one: its live draft's
/// (`live.release`) or its file's (`version.release-file`).
async fn record_version(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    one: Prepared,
    pins: Vec<versions::Pin>,
) -> Result<VersionRow, AktError> {
    let revision = match one.draft {
        Some(loaded) => {
            record_draft(conn, caller, &one.ty, one.akt.clone(), loaded, &one.fields).await?
        }
        None if one.records => {
            record_fields(conn, caller, &one.ty, one.akt.clone(), &one.fields)
                .await?
                .revision
        }
        None => one.revision,
    };
    let row = VersionRow {
        tenant_id: one.akt.tenant_id,
        akt_id: one.akt.id,
        number: one.next,
        revision,
        label: one.label,
        released_by: caller.actor.subject.into(),
        released_at: Utc::now(),
        pins: serde_json::to_value(pins).map_err(|e| AktError::Internal(e.to_string()))?,
        description: one.description,
    };
    stored::insert(conn, &row).await?;
    Ok(row)
}

/// Checks that `caller` may release `akt` at `one.revision` with
/// `one.label`, and answers what it is released as.
async fn prepare(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    akt: AktRow,
    one: &Releasing,
) -> Result<Prepared, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let number = akt.number.clone();
    let revision = one.revision;
    let ty = type_of(conn, tenant, &akt).await?;
    require_read(caller.auth, &ty, &number).await?;
    super::archive::writable(&akt)?;
    let Some(declared) = &ty.ty.document().versions else {
        return Err(AktError::BadRequest(format!(
            "`{}` akter don't release versions",
            ty.ty.key()
        )));
    };
    require(
        caller.auth,
        Permission::Release {
            key: ty.ty.key().to_owned(),
            action: declared.action.clone(),
        },
    )
    .await?;
    if akt.revision != revision {
        return Err(AktError::Stale {
            expected: revision,
            current: akt.revision,
        });
    }
    let (label, description) = said(one)?;
    let file = file_of(&ty, one, &number)?;
    let current = values_of(&ty, &akt)?;
    // A file set or cleared, as a live draft, makes the version's revision.
    let changes_file = match &file {
        Some((_, Some(_))) => true,
        Some((key, None)) => current.contains_key(key),
        None => false,
    };
    let existing = stored::of(conn, tenant, akt.id).await?;
    let live = super::live::has_live_draft(&ty);
    if !live && !changes_file && existing.iter().any(|v| v.revision == revision) {
        return Err(AktError::BadRequest(format!(
            "revision {revision} of {number} is already a version"
        )));
    }
    let draft = if live {
        Some(super::live::load(conn, &ty, &akt).await?)
    } else {
        None
    };
    let mut fields = match &draft {
        Some(loaded) => loaded.fields.clone(),
        None => current,
    };
    match file {
        Some((key, Some(id))) => {
            fields.insert(key, kuria_engine::Value::Attachment(id));
        }
        Some((key, None)) => {
            fields.remove(&key);
        }
        None => {}
    }
    // r[impl live.release]
    if draft.is_some()
        && let Some(latest) = existing.first()
    {
        let released = at_revision(conn, tenant, akt.id, latest.revision)
            .await?
            .map(|(fields, _)| fields)
            .unwrap_or_default();
        if released == kuria_engine::fields_to_json(&fields) {
            return Err(AktError::BadRequest(format!(
                "{number}'s live draft is version {} as it is",
                latest.number
            )));
        }
    }
    Ok(Prepared {
        next: existing.first().map_or(1, |v| v.number + 1),
        akt,
        ty,
        revision,
        label,
        description,
        records: draft.is_some() || changes_file,
        draft,
        fields,
    })
}

/// `one`'s label and description, trimmed, each `None` if blank; refused
/// unless the label is one line of at most 100 characters and the
/// description at most 2,000 (`version.release`).
fn said(one: &Releasing) -> Result<(Option<String>, Option<String>), AktError> {
    let trimmed = |text: &Option<String>| {
        text.as_ref()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    let (label, description) = (trimmed(&one.label), trimmed(&one.description));
    if label
        .as_ref()
        .is_some_and(|l| l.chars().count() > 100 || l.contains(['\n', '\r']))
    {
        return Err(AktError::BadRequest(
            "a label is one line of at most 100 characters".to_owned(),
        ));
    }
    if description
        .as_ref()
        .is_some_and(|d| d.chars().count() > 2000)
    {
        return Err(AktError::BadRequest(
            "a description is at most 2,000 characters".to_owned(),
        ));
    }
    Ok((label, description))
}

/// The field `one` releases `number`, of the type `ty`, with a file in,
/// and the PDF it is released with, if any: one that a type released only
/// as files needs, one that a type written here may have, and that any
/// other type refuses.
// r[impl version.release-file]
fn file_of(
    ty: &StoredType,
    one: &Releasing,
    number: &str,
) -> Result<Option<(String, Option<Uuid>)>, AktError> {
    match (ty.ty.released_file(), one.file) {
        (Some(key), Some(id)) => Ok(Some((key.to_owned(), Some(id)))),
        (Some(key), None) if ty.ty.written_here() => Ok(Some((key.to_owned(), None))),
        (Some(_), None) => Err(AktError::BadRequest(format!(
            "{number} is released with its PDF: upload it and name it as `file`"
        ))),
        (None, Some(_)) => Err(AktError::BadRequest(format!(
            "`{}` akter aren't released with a file",
            ty.ty.key()
        ))),
        (None, None) => Ok(None),
    }
}

/// Records `akt`'s live draft `loaded` as its next revision, checked as a
/// write would be, and answers its number (`live.release`).
// r[impl live.release]
// r[impl live.revisions]
async fn record_draft(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    ty: &StoredType,
    akt: AktRow,
    loaded: super::live::Loaded,
    fields: &std::collections::BTreeMap<String, kuria_engine::Value>,
) -> Result<i32, AktError> {
    // The live draft's fields, and the release's file (`version.release-file`).
    let akt = record_fields(conn, caller, ty, akt, fields).await?;
    super::live::released(conn, ty, &akt, &loaded, Utc::now()).await?;
    Ok(akt.revision)
}

/// Records `fields` as `akt`'s next revision, checked as a write would be,
/// and answers the akt as it then is: a live draft's (`live.release`), or a
/// release's file (`version.release-file`), whose attachment is held as an
/// edit's is.
// r[impl version.release-file]
async fn record_fields(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    ty: &StoredType,
    mut akt: AktRow,
    fields: &std::collections::BTreeMap<String, kuria_engine::Value>,
) -> Result<AktRow, AktError> {
    let tenant = akt.tenant_id;
    let before = values_of(ty, &akt)?;
    // r[impl akt.field.required]
    if let Some(missing) = ty
        .ty
        .document()
        .fields
        .iter()
        .find(|f| f.required && !fields.contains_key(&f.key))
    {
        return Err(AktError::Refused(kuria_engine::Refusal::Required(
            missing.key.clone(),
        )));
    }
    let outcome = kuria_engine::Outcome {
        state: akt.state.clone(),
        hash: kuria_engine::state_hash(&akt.state, fields),
        changes: super::live::changes(&before, fields),
        fields: fields.clone(),
    };
    let now = Utc::now();
    let expected = akt.revision;
    akt.fields = Json::Object(kuria_engine::fields_to_json(&outcome.fields));
    akt.title = super::title_of(&ty.ty, &outcome.fields);
    akt.revision += 1;
    akt.updated_at = now;
    if !store::update_akt(conn, &akt, expected).await? {
        return Err(AktError::Internal(format!(
            "akt {} moved while locked",
            akt.number
        )));
    }
    super::check_sections(conn, tenant, &ty.ty, Some(akt.id), &outcome).await?;
    super::check_contacts(conn, tenant, &outcome).await?;
    super::hold_attachments(conn, caller, &ty.ty, &akt, &outcome).await?;
    super::diagrams::render_written(conn, caller, &ty.ty, &akt, &outcome).await?;
    let mut recorded = super::revision(&akt, caller.actor, None, None, &outcome, now);
    super::references::record(conn, &ty.ty, &akt, &outcome.fields, &mut recorded).await?;
    store::append_revision(conn, &recorded).await?;
    Ok(akt)
}
