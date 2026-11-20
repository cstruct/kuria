//! Akt types, akter and akt revisions in GraphQL.

// async-graphql requires every resolver to be async, also those that don't
// await anything.
#![allow(clippy::unused_async, clippy::unused_async_trait_impl)]

use std::collections::HashMap;

use async_graphql::{
    ComplexObject, Context, Enum, ErrorExtensions, ID, InputObject, Json, Object, OneofObject,
    SimpleObject, Union,
};
use chrono::{DateTime, Utc};
use kuria_config::{
    AttachmentFormat as ConfigFormat, ChoiceOption as ConfigOption, FieldDef,
    FieldKind as ConfigKind,
};
use kuria_engine::board::{UNMAPPED, column_of};
use kuria_engine::versions::Pin;
use kuria_engine::{Value, fields_from_json};
use kuria_store::akter::{self as store, AktRow, ChangedAfter, RevisionRow};
use kuria_store::containers;
use kuria_store::versions::VersionRow;
use serde_json::{Map, Value as JsonValue};
use uuid::Uuid;

use super::Principal;
use super::attachments::Attachment;
use super::filter::{self, FieldCondition};
use super::page::{After, AktConnection, List, page_size};
use super::people::{Someone, User};
use crate::akter::{AktError, Akter, Caller, Operation, Place, StoredType, outcomes};
use crate::authorize::{Authorizer, Permission};

/// The most members one board shows.
const MAX_BOARD: i32 = 1_000;

fn principal<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a Principal> {
    ctx.data::<Principal>()
}

fn akter<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a Akter> {
    ctx.data::<Akter>()
}

fn authorizer<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a Authorizer> {
    ctx.data::<std::sync::Arc<Authorizer>>().map(AsRef::as_ref)
}

/// Whether the request may read the akter of the akt type `key`.
async fn reads(ctx: &Context<'_>, key: &str) -> async_graphql::Result<bool> {
    authorizer(ctx)?
        .allows(&Permission::Read {
            key: key.to_owned(),
        })
        .await
        .map_err(|e| error(e.into()))
}

/// Whether the request may see the akt type `key`.
async fn sees_type(ctx: &Context<'_>, key: &str) -> async_graphql::Result<bool> {
    authorizer(ctx)?
        .sees_type(key)
        .await
        .map_err(|e| error(e.into()))
}

/// An [`AktError`] as a GraphQL error with a `code` extension, and for an
/// invalid akt type, its `problems`.
fn error(error: AktError) -> async_graphql::Error {
    let message = error.to_string();
    match error {
        AktError::InvalidType(problems) => {
            async_graphql::Error::new(message).extend_with(|_, e| {
                e.set("code", "INVALID_AKT_TYPE");
                e.set(
                    "problems",
                    problems
                        .iter()
                        .map(|p| {
                            let mut problem = async_graphql::indexmap::IndexMap::new();
                            problem.insert(
                                async_graphql::Name::new("path"),
                                async_graphql::Value::from(p.path.clone()),
                            );
                            problem.insert(
                                async_graphql::Name::new("message"),
                                async_graphql::Value::from(p.message.clone()),
                            );
                            async_graphql::Value::Object(problem)
                        })
                        .collect::<Vec<_>>(),
                );
            })
        }
        AktError::UnreadableType(_) => coded(message, "INVALID_AKT_TYPE"),
        AktError::UnknownType(_) => coded(message, "UNKNOWN_AKT_TYPE"),
        AktError::NotFound(_) | AktError::NotLinked { .. } => coded(message, "NOT_FOUND"),
        AktError::Refused(_) | AktError::NotDraftable(_) => coded(message, "REFUSED"),
        AktError::Forbidden => coded(message, "FORBIDDEN"),
        AktError::NumbersExhausted(_) => coded(message, "NUMBERS_EXHAUSTED"),
        AktError::NumberTaken(_) => coded(message, "NUMBER_TAKEN"),
        AktError::Archived(_) => coded(message, "ARCHIVED"),
        AktError::NotContainable { .. } => coded(message, "NOT_CONTAINABLE"),
        AktError::AlreadyMember { .. } => coded(message, "ALREADY_MEMBER"),
        AktError::NotMember { .. } => coded(message, "NOT_MEMBER"),
        AktError::Exclusive { .. } => coded(message, "EXCLUSIVE"),
        AktError::Cycle { .. } => coded(message, "CYCLE"),
        AktError::Frozen(_) => coded(message, "FROZEN"),
        AktError::NotAccepting { .. } => coded(message, "NOT_ACCEPTING"),
        AktError::CreatedIn { .. } => coded(message, "CREATED_IN"),
        AktError::SelfMove | AktError::BadRequest(_) => coded(message, "BAD_REQUEST"),
        AktError::InvalidLink(_) => coded(message, "INVALID_LINK"),
        AktError::BrokenReferences { broken, together } => {
            let (broken, together) = (broken.clone(), together.clone());
            async_graphql::Error::new(message).extend_with(move |_, e| {
                e.set("code", "BROKEN_REFERENCES");
                e.set("references", broken.clone());
                e.set("together", together.clone());
            })
        }
        AktError::Unavailable(detail) => {
            tracing::warn!("akter: {detail}");
            coded(
                "authorization is unavailable; try again".to_owned(),
                "UNAVAILABLE",
            )
        }
        AktError::InvalidDiagram { field, block, .. } => async_graphql::Error::new(message)
            .extend_with(move |_, e| {
                e.set("code", "INVALID_DIAGRAM");
                e.set("field", field.clone());
                e.set("block", i32::try_from(block).unwrap_or(i32::MAX));
            }),
        AktError::Stale { current, .. } => {
            async_graphql::Error::new(message).extend_with(|_, e| {
                e.set("code", "STALE");
                e.set("currentRevision", current);
            })
        }
        AktError::StaleDraft {
            number,
            base,
            state_vector,
        } => {
            use base64::Engine as _;
            let base64 = base64::engine::general_purpose::STANDARD;
            let (base, state_vector) = (base64.encode(base), base64.encode(state_vector));
            async_graphql::Error::new(message).extend_with(move |_, e| {
                e.set("code", "STALE");
                e.set("number", number.clone());
                e.set("base", base.clone());
                e.set("stateVector", state_vector.clone());
            })
        }
        AktError::Internal(detail) => {
            tracing::error!("akter: {detail}");
            coded("internal error".to_owned(), "INTERNAL")
        }
    }
}

fn coded(message: String, code: &'static str) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, e| e.set("code", code))
}

fn internal(e: impl std::fmt::Display) -> async_graphql::Error {
    error(AktError::Internal(e.to_string()))
}

/// The JSON object of field values a mutation takes.
fn field_values(fields: Option<Json<JsonValue>>) -> async_graphql::Result<Map<String, JsonValue>> {
    match fields.map(|f| f.0) {
        None => Ok(Map::new()),
        Some(JsonValue::Object(map)) => Ok(map),
        Some(_) => Err(coded(
            "fields is an object of field keys to values".to_owned(),
            "BAD_REQUEST",
        )),
    }
}

#[derive(Default)]
pub struct AkterQuery;

#[Object]
impl AkterQuery {
    /// The latest version of every akt type in the tenant.
    async fn akt_types(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<AktType>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let rows = store::latest_type_versions(&mut conn, tenant)
            .await
            .map_err(internal)?;
        let mut visible = Vec::new();
        for row in rows {
            // r[impl authz.read-types]
            if sees_type(ctx, &row.key).await? {
                visible.push(AktType(StoredType::read(row).map_err(error)?));
            }
        }
        Ok(visible)
    }

    /// An akt type: its latest version, or the version named.
    async fn akt_type(
        &self,
        ctx: &Context<'_>,
        key: String,
        version: Option<i32>,
    ) -> async_graphql::Result<Option<AktType>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let row = match version {
            Some(version) => store::type_version(&mut conn, tenant, &key, version).await,
            None => store::latest_type_version(&mut conn, tenant, &key).await,
        }
        .map_err(internal)?;
        match row {
            Some(row) if sees_type(ctx, &row.key).await? => {
                Ok(Some(AktType(StoredType::read(row).map_err(error)?)))
            }
            _ => Ok(None),
        }
    }

    /// A diagram block's `source` rendered to SVG, or typst's message if it
    /// doesn't compile, storing nothing (`diagram.preview`).
    // r[impl diagram.preview]
    async fn render_diagram(
        &self,
        ctx: &Context<'_>,
        source: String,
    ) -> async_graphql::Result<RenderedDiagram> {
        principal(ctx)?;
        if source.len() > 64 * 1024 {
            return Ok(RenderedDiagram {
                svg: None,
                error: Some("a diagram is at most 64 KiB of typst".to_owned()),
            });
        }
        Ok(
            match crate::akter::diagrams::render(akter(ctx)?.renderer(), source)
                .await
                .map_err(error)?
            {
                Ok(svg) => RenderedDiagram {
                    svg: Some(svg),
                    error: None,
                },
                Err(message) => RenderedDiagram {
                    svg: None,
                    error: Some(message),
                },
            },
        )
    }

    /// The akt numbered `number`.
    async fn akt(
        &self,
        ctx: &Context<'_>,
        number: String,
        #[graphql(desc = "Read it as it was at this version of it (`version.read`).")]
        version: Option<i32>,
        #[graphql(desc = "Read it as it was at this revision, its fields, state and title.")]
        revision: Option<i32>,
    ) -> async_graphql::Result<Option<Akt>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let Some(row) = store::akt_by_number(&mut conn, tenant, &number)
            .await
            .map_err(internal)?
        else {
            return Ok(None);
        };
        let akt = with_types(&mut conn, tenant, vec![row]).await?.pop();
        // r[impl authz.read]
        let akt = match akt {
            Some(akt) if reads(ctx, akt.ty.ty.key()).await? => akt,
            _ => return Ok(None),
        };
        if let Some(revision) = revision.filter(|_| version.is_none()) {
            return Ok(
                crate::akter::versions::at(&mut conn, &akt.ty, &akt.row, revision)
                    .await
                    .map_err(error)?
                    .map(|row| Akt {
                        row,
                        ty: akt.ty,
                        version: None,
                        at_revision: true,
                    }),
            );
        }
        let Some(version) = version else {
            return Ok(Some(akt));
        };
        // r[impl version.read]
        Ok(
            crate::akter::versions::at_version(&mut conn, &akt.ty, &akt.row, version)
                .await
                .map_err(error)?
                .map(|(row, version)| Akt {
                    row,
                    ty: akt.ty,
                    version: Some(version),
                    at_revision: false,
                }),
        )
    }

    /// Akter, most recently changed first: of one akt type and in one state,
    /// if given, a page of at most `first` (50 by default, 200 at most)
    /// after the cursor `after`.
    // r[impl akt.list]
    // r[impl akt.page]
    #[expect(
        clippy::too_many_arguments,
        reason = "each is one of the query's GraphQL arguments"
    )]
    async fn akter(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "type")] key: Option<String>,
        state: Option<String>,
        first: Option<i32>,
        after: Option<String>,
        #[graphql(
            desc = "Akter whose number starts with this, or whose title contains each of its words, ignoring case; the most recently changed first."
        )]
        search: Option<String>,
        #[graphql(
            name = "where",
            desc = "Conditions on field values, all of which an akt must meet; `type` must be given."
        )]
        conditions: Option<Vec<FieldCondition>>,
        #[graphql(
            desc = "Only akter with this outcome, as it is now; `type` must be given, and have a `verdict` field."
        )]
        outcome: Option<OutcomeKind>,
        #[graphql(desc = "Live akter (the default), archived ones, or both.")] archived: Option<
            ArchivedFilter,
        >,
    ) -> async_graphql::Result<AktConnection> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let size = page_size(first);
        let conditions = conditions.unwrap_or_default();
        let where_key = serde_json::to_string(&conditions).map_err(internal)?;
        let archived = archived.unwrap_or_default();
        let list = List::new(
            if search.is_some() { "search" } else { "akter" },
            &[
                key.as_deref(),
                state.as_deref(),
                search.as_deref(),
                Some(&where_key),
                outcome.map(OutcomeKind::key),
                Some(archived_key(archived)),
            ],
        );
        let after = match list.after(after.as_deref())? {
            Some(After::Changed(changed)) => Some(changed),
            Some(After::Ranked(_)) => return Err(bad_cursor()),
            None => None,
        };
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        // r[impl authz.read]
        // Only the akt types the subject may read are asked for, so a page
        // holds akter they can see.
        let mut readable = Vec::new();
        for ty in store::latest_type_versions(&mut conn, tenant)
            .await
            .map_err(internal)?
        {
            if key.as_ref().is_none_or(|k| *k == ty.key) && reads(ctx, &ty.key).await? {
                readable.push(ty.key);
            }
        }
        let mut conditions = match key.as_deref() {
            _ if conditions.is_empty() => Vec::new(),
            Some(key) => matching(ctx, &mut conn, tenant, key, &conditions).await?,
            None => {
                return Err(coded(
                    "filtering by fields needs `type`".to_owned(),
                    "BAD_REQUEST",
                ));
            }
        };
        if let Some(outcome) = outcome {
            match by_outcome(&mut conn, tenant, key.as_deref(), outcome).await? {
                ByOutcome::Verdicts(condition) => conditions.push(condition),
                ByOutcome::Members(..) => {
                    return Err(coded(
                        "this type's outcome comes from its members, so only a container's members filter by it".to_owned(),
                        "BAD_REQUEST",
                    ));
                }
            }
        }
        let listing = store::Listing {
            keys: &readable,
            archived: archived.into(),
            state: state.as_deref(),
            conditions: &conditions,
            after,
            limit: size + 1,
        };
        // r[impl akt.search]
        let rows = match &search {
            Some(text) => store::search_akter(&mut conn, tenant, text, &listing)
                .await
                .map_err(internal)?,
            None => store::list_akter(&mut conn, tenant, &listing)
                .await
                .map_err(internal)?,
        };
        let (rows, page_info) = list.page(
            rows,
            size,
            |row| After::Changed(ChangedAfter::of(row)),
            |row| row.id,
        );
        Ok(AktConnection {
            nodes: with_types(&mut conn, tenant, rows).await?,
            page_info,
        })
    }
}

/// The store's conditions for `conditions` on fields of the akt type `key`,
/// by its latest version: whom a user or checkbox condition names includes
/// the contacts linked to a named user, so `me` finds values naming one.
// r[impl akt.filter]
async fn matching(
    ctx: &Context<'_>,
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    key: &str,
    conditions: &[FieldCondition],
) -> async_graphql::Result<Vec<store::Condition>> {
    let Some(row) = store::latest_type_versions(conn, tenant)
        .await
        .map_err(internal)?
        .into_iter()
        .find(|row| row.key == key)
    else {
        return Err(coded(
            format!("there is no akt type `{key}`"),
            "BAD_REQUEST",
        ));
    };
    let ty = StoredType::read(row).map_err(error)?;
    let me: Uuid = principal(ctx)?.subject.into();
    let mut matches = Vec::new();
    for condition in conditions {
        let field = filter::field_of(&ty.ty, condition)?;
        let named = if matches!(field.kind, ConfigKind::User | ConfigKind::Checkbox) {
            let (user, contact) = filter::named(&field.key, &condition.equals, me)?;
            let mut named = filter::Named {
                users: user.into_iter().collect(),
                contacts: contact.into_iter().collect(),
            };
            if let Some(user) = user {
                // r[impl bundle.import.subjects]
                // Subjects an import aliased to the user filter as them.
                named.users.extend(
                    kuria_store::bundles::aliases_of(conn, tenant, user)
                        .await
                        .map_err(internal)?,
                );
                named.contacts.extend(
                    kuria_store::contacts::linked_to(conn, tenant, user)
                        .await
                        .map_err(internal)?,
                );
            }
            Some(named)
        } else {
            None
        };
        matches.push(filter::condition(field, &condition.equals, named.as_ref())?);
    }
    Ok(matches)
}

/// A container's members of some akt types, meeting some conditions.
struct Members<'a> {
    container: Uuid,
    keys: &'a [String],
    archived: store::Archived,
    conditions: &'a [store::Condition],
}

impl Members<'_> {
    /// Up to `limit` of them, in rank order after `after`, whose outcome,
    /// from their own members, is `wanted`. They are read a chunk at a time
    /// until the page is full.
    // r[impl akt.filter.outcome]
    // r[impl akt.page]
    async fn held(
        &self,
        conn: &mut kuria_store::Connection,
        tenant: Uuid,
        types: &outcomes::Types,
        wanted: kuria_engine::outcomes::Outcome,
        mut after: Option<String>,
        limit: i64,
    ) -> async_graphql::Result<Vec<AktRow>> {
        const CHUNK: i64 = 200;
        let mut found = Vec::new();
        loop {
            let chunk = containers::member_page(
                conn,
                tenant,
                self.container,
                self.keys,
                self.archived,
                self.conditions,
                after.as_deref(),
                CHUNK,
            )
            .await
            .map_err(internal)?;
            let last = chunk.len() < usize::try_from(CHUNK).unwrap_or(usize::MAX);
            for row in chunk {
                after = Some(row.rank.clone());
                if outcomes::of(conn, tenant, types, &row)
                    .await
                    .map_err(error)?
                    == Some(wanted)
                {
                    found.push(row);
                    if i64::try_from(found.len()).unwrap_or(i64::MAX) >= limit {
                        return Ok(found);
                    }
                }
            }
            if last {
                return Ok(found);
            }
        }
    }
}

/// How a list of the akt type `key` filters by `outcome`
/// (`akt.filter.outcome`).
enum ByOutcome {
    /// By a condition on its own verdicts, which the store pages.
    Verdicts(store::Condition),
    /// By its members' outcomes, worked out for each akt listed.
    Members(kuria_engine::outcomes::Outcome, outcomes::Types),
}

// r[impl akt.filter.outcome]
async fn by_outcome(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    key: Option<&str>,
    outcome: OutcomeKind,
) -> async_graphql::Result<ByOutcome> {
    let refused = |message: String| Err(coded(message, "BAD_REQUEST"));
    let Some(key) = key else {
        return refused("filtering by outcome needs `type`".to_owned());
    };
    let types = outcomes::Types::of(conn, tenant).await.map_err(error)?;
    let Some(row) = store::latest_type_versions(conn, tenant)
        .await
        .map_err(internal)?
        .into_iter()
        .find(|row| row.key == key)
    else {
        return refused(format!("there is no akt type `{key}`"));
    };
    let ty = StoredType::read(row).map_err(error)?;
    if !types.has_outcomes(&ty.ty) {
        return refused(format!("akter of `{key}` have no outcome"));
    }
    let wanted = kuria_engine::outcomes::Outcome::parse(outcome.key())
        .ok_or_else(|| internal("an outcome"))?;
    Ok(
        match ty
            .ty
            .document()
            .fields
            .iter()
            .find(|f| f.kind == ConfigKind::Verdict)
        {
            Some(field) => ByOutcome::Verdicts(store::Condition(vec![store::Match::Outcome {
                verdict: field.key.clone(),
                assignees: field.assignees.clone(),
                outcome: wanted.as_str().to_owned(),
            }])),
            None => ByOutcome::Members(wanted, types),
        },
    )
}

fn bad_cursor() -> async_graphql::Error {
    coded(
        "that cursor isn't one of this list's".to_owned(),
        "BAD_REQUEST",
    )
}

/// Pairs each akt with its akt type version, read in one query.
async fn with_types(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    rows: Vec<AktRow>,
) -> async_graphql::Result<Vec<Akt>> {
    let mut ids: Vec<Uuid> = rows.iter().map(|r| r.type_version_id).collect();
    ids.sort_unstable();
    ids.dedup();
    let types: HashMap<Uuid, StoredType> = store::type_versions_by_id(conn, tenant, &ids)
        .await
        .map_err(internal)?
        .into_iter()
        .map(|row| StoredType::read(row).map(|t| (t.row.id, t)))
        .collect::<Result<_, _>>()
        .map_err(error)?;
    let mut paired = rows
        .into_iter()
        .map(|row| {
            let ty = types
                .get(&row.type_version_id)
                .ok_or_else(|| internal(format!("akt {} has no akt type", row.number)))?;
            Ok((row, ty))
        })
        .collect::<async_graphql::Result<Vec<_>>>()?;
    // r[impl live.read]
    crate::akter::live::read_as_drafts(conn, &mut paired)
        .await
        .map_err(error)?;
    Ok(paired
        .into_iter()
        .map(|(row, ty)| Akt {
            row,
            ty: ty.clone(),
            version: None,
            at_revision: false,
        })
        .collect())
}

#[derive(Default)]
pub struct AkterMutation;

/// What creating an akt takes.
#[derive(InputObject)]
pub struct CreateAktInput {
    /// The akt type's key; the akt gets its latest version.
    #[graphql(name = "type")]
    key: String,
    /// Field keys to values.
    fields: Option<Json<JsonValue>>,
    reason: Option<String>,
    /// The containers to create it in, by number; the first is its parent.
    #[graphql(name = "in", default)]
    inside: Vec<String>,
    /// Its number, such as the one it had where it is imported from, which
    /// its type's pattern must give and no akt may have
    /// (`akt.number.given`); else its next.
    number: Option<String>,
}

/// What editing an akt takes.
#[derive(InputObject)]
pub struct EditAktInput {
    number: String,
    /// The revision the edit is based on.
    revision: i32,
    /// Field keys to values; `null` clears a field.
    fields: Json<JsonValue>,
    reason: Option<String>,
}

/// What copying an akt takes (`akt.copy`).
#[derive(InputObject)]
pub struct CopyAktInput {
    /// The akt to copy.
    number: String,
    /// The akter of its tree to keep, with those between them and it and
    /// every member under them; all of them if not given.
    only: Option<Vec<String>>,
    /// The containers to make the copy in; the copied akt's own if not
    /// given.
    #[graphql(name = "in")]
    inside: Option<Vec<String>>,
    reason: Option<String>,
}

/// What performing a transition takes.
#[derive(InputObject)]
pub struct PerformTransitionInput {
    number: String,
    /// The revision the transition is based on.
    revision: i32,
    transition: String,
    /// Field values to set on the way; `null` clears a field.
    fields: Option<Json<JsonValue>>,
    reason: Option<String>,
}

/// One operation of a batch: exactly one of the akt mutations, with its
/// input. Wherever it takes an akt's number, `@<n>` names the akt the
/// batch's operation `n`, an earlier `createAkt`, created.
#[derive(OneofObject)]
pub enum OperationInput {
    CreateAkt(CreateAktInput),
    EditAkt(EditAktInput),
    PerformTransition(PerformTransitionInput),
    AddToContainer(MembershipInput),
    RemoveFromContainer(MembershipInput),
    TransferMember(TransferInput),
    AddLink(LinkInput),
    RemoveLink(UnlinkInput),
    MoveAkt(MoveAktInput),
    DraftField(DraftFieldInput),
}

/// What a batch takes: its operations, run in order, all or none.
#[derive(InputObject)]
pub struct BatchInput {
    operations: Vec<OperationInput>,
}

/// `operation` as the batch's write path takes it.
fn operation(input: OperationInput) -> async_graphql::Result<Operation> {
    Ok(match input {
        OperationInput::CreateAkt(i) => Operation::Create {
            fields: field_values(i.fields)?,
            key: i.key,
            reason: i.reason,
            inside: i.inside,
            number: i.number,
        },
        OperationInput::EditAkt(i) => Operation::Edit {
            fields: field_values(Some(i.fields))?,
            number: i.number,
            revision: i.revision,
            reason: i.reason,
        },
        OperationInput::PerformTransition(i) => Operation::Transition {
            fields: field_values(i.fields)?,
            number: i.number,
            revision: i.revision,
            transition: i.transition,
            reason: i.reason,
        },
        OperationInput::AddToContainer(i) => Operation::AddMember {
            container: i.container,
            member: i.member,
            reason: i.reason,
        },
        OperationInput::RemoveFromContainer(i) => Operation::RemoveMember {
            container: i.container,
            member: i.member,
            reason: i.reason,
        },
        OperationInput::TransferMember(i) => Operation::Transfer {
            member: i.member,
            from: i.from,
            to: i.to,
            reason: i.reason,
        },
        OperationInput::AddLink(i) => Operation::AddLink {
            source: i.source,
            kind: i.kind,
            target: i.target,
            pin: i.pin_revision,
            reason: i.reason,
        },
        OperationInput::RemoveLink(i) => Operation::RemoveLink {
            source: i.source,
            kind: i.kind,
            target: i.target,
            reason: i.reason,
        },
        OperationInput::MoveAkt(i) => {
            let (before, next_to) = match (i.before, i.after) {
                (Some(before), None) => (true, before),
                (None, Some(after)) => (false, after),
                _ => {
                    return Err(coded(
                        "give exactly one of before and after".to_owned(),
                        "BAD_REQUEST",
                    ));
                }
            };
            Operation::Move {
                number: i.number,
                before,
                next_to,
            }
        }
        OperationInput::DraftField(i) => Operation::Draft {
            number: i.number,
            revision: i.revision,
            field: i.field,
            reason: i.reason,
        },
    })
}

/// One live draft's part of a push from a clone (`clone.server`).
#[derive(InputObject)]
pub struct DraftPushInput {
    /// The akt's number.
    number: String,
    /// The live draft's `base` as the clone read it (`liveState`),
    /// base64-encoded: the push is refused as `STALE` unless it is the
    /// draft's now.
    base: String,
    /// What the clone changed, as one Yjs v1 update made against `base`,
    /// base64-encoded.
    update: String,
}

/// What a push from a clone takes (`clone.server`).
#[derive(InputObject)]
pub struct PushCloneInput {
    /// One line of at most 200 characters, kept with each live draft's
    /// change and as each revision's reason.
    message: String,
    /// Each live draft's update.
    #[graphql(default)]
    drafts: Vec<DraftPushInput>,
    /// The writes to akter without a live draft, as a batch's, each at the
    /// revision the clone read; their reasons are the message.
    #[graphql(default)]
    operations: Vec<OperationInput>,
}

/// What a push from a clone wrote, as reading answers it.
#[derive(SimpleObject)]
pub struct PushedClone {
    /// Each akt whose live draft the push changed, in the order of its
    /// drafts.
    drafts: Vec<Akt>,
    /// The akt each operation answers, in order.
    operations: Vec<Akt>,
}

/// `text`, base64-decoded, or a `BAD_REQUEST` naming `what`, said of the
/// push's draft `n`, counting from 1.
fn base64_of(text: &str, what: &str, n: usize) -> async_graphql::Result<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|e| at_draft(coded(format!("{what} isn't base64: {e}"), "BAD_REQUEST"), n))
}

/// `error`, said of the push's draft `n`, counting from 1.
fn at_draft(error: async_graphql::Error, n: usize) -> async_graphql::Error {
    error.extend_with(|_, e| e.set("draft", n))
}

/// `error`, said of the batch's operation `n`, counting from 1.
fn at_operation(error: async_graphql::Error, n: usize) -> async_graphql::Error {
    error.extend_with(|_, e| e.set("operation", n))
}

#[Object]
impl AkterMutation {
    /// Runs several akt writes in order in one transaction: all of them, or
    /// none. Answers the akt each operation answers, in order; a refusal
    /// carries the operation's position, from 1, as `operation`.
    // r[impl akt.batch]
    // r[impl akt.batch.atomic]
    async fn batch(&self, ctx: &Context<'_>, input: BatchInput) -> async_graphql::Result<Vec<Akt>> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let mut operations = Vec::with_capacity(input.operations.len());
        for (i, op) in input.operations.into_iter().enumerate() {
            operations.push(operation(op).map_err(|e| at_operation(e, i + 1))?);
        }
        let akter = akter(ctx)?;
        let rows = akter.batch(caller, operations).await.map_err(|refused| {
            let e = error(refused.error);
            match refused.operation {
                Some(n) => at_operation(e, n),
                None => e,
            }
        })?;
        let mut answered = Vec::with_capacity(rows.len());
        for row in rows {
            answered.push(reload(ctx, akter, row).await?);
        }
        Ok(answered)
    }

    /// Pushes from a clone, all of it or none (`clone.server`): each live
    /// draft's update made against the base the clone read, refused as
    /// `STALE`, with the draft's `number`, current `base` and
    /// `stateVector`, unless the base is the draft's now, and checked as a
    /// live edit; then the
    /// operations, as a batch runs them, with the message as each one's
    /// reason. Each draft's update is kept as one change, by the subject,
    /// with the message. A refusal carries the draft's position, from 1, as
    /// `draft`, or the operation's as `operation`.
    // r[impl clone.server]
    async fn push_clone(
        &self,
        ctx: &Context<'_>,
        input: PushCloneInput,
    ) -> async_graphql::Result<PushedClone> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let mut drafts = Vec::with_capacity(input.drafts.len());
        for (i, draft) in input.drafts.into_iter().enumerate() {
            drafts.push(crate::akter::DraftPush {
                base: base64_of(&draft.base, "base", i + 1)?,
                update: base64_of(&draft.update, "update", i + 1)?,
                number: draft.number,
            });
        }
        let mut operations = Vec::with_capacity(input.operations.len());
        for (i, op) in input.operations.into_iter().enumerate() {
            operations.push(operation(op).map_err(|e| at_operation(e, i + 1))?);
        }
        let akter = akter(ctx)?;
        let pushed = akter
            .push_clone(caller, &input.message, drafts, operations)
            .await
            .map_err(|refused| {
                let e = error(refused.error);
                match refused.part {
                    Some(crate::akter::PushPart::Draft(n)) => at_draft(e, n),
                    Some(crate::akter::PushPart::Operation(n)) => at_operation(e, n),
                    None => e,
                }
            })?;
        let mut answered = PushedClone {
            drafts: Vec::with_capacity(pushed.drafts.len()),
            operations: Vec::with_capacity(pushed.operations.len()),
        };
        for row in pushed.drafts {
            answered.drafts.push(reload(ctx, akter, row).await?);
        }
        for row in pushed.operations {
            answered.operations.push(reload(ctx, akter, row).await?);
        }
        Ok(answered)
    }

    /// Submits an akt type document, creating the next version of its key.
    async fn define_akt_type(
        &self,
        ctx: &Context<'_>,
        document: Json<JsonValue>,
    ) -> async_graphql::Result<AktType> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        akter(ctx)?
            .define_type(caller, document.0)
            .await
            .map(AktType)
            .map_err(error)
    }

    /// Creates an akt.
    async fn create_akt(
        &self,
        ctx: &Context<'_>,
        input: CreateAktInput,
    ) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let fields = field_values(input.fields)?;
        let akter = akter(ctx)?;
        let row = akter
            .create(
                caller,
                &input.key,
                &fields,
                input.reason,
                &input.inside,
                input.number.as_deref(),
            )
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }

    /// Copies an akt, with the part of its member tree chosen, into new
    /// akter of the same types in their initial states, and answers the copy.
    // r[impl akt.copy]
    async fn copy_akt(&self, ctx: &Context<'_>, input: CopyAktInput) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let akter = akter(ctx)?;
        let row = akter
            .copy(
                caller,
                &input.number,
                input.only.as_deref(),
                input.inside.as_deref(),
                input.reason,
            )
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }

    /// Edits an akt's field values.
    async fn edit_akt(&self, ctx: &Context<'_>, input: EditAktInput) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let fields = field_values(Some(input.fields))?;
        let akter = akter(ctx)?;
        let row = akter
            .edit(caller, &input.number, input.revision, &fields, input.reason)
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }

    /// Releases the akt's current revision as its next version
    /// (`version.release`).
    async fn release_version(
        &self,
        ctx: &Context<'_>,
        input: ReleaseVersionInput,
    ) -> async_graphql::Result<Version> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let row = akter(ctx)?
            .release(
                caller,
                crate::akter::versions::Releasing {
                    file: file_of(input.file.as_ref())?,
                    number: input.number,
                    revision: input.revision,
                    label: input.label,
                    description: input.description,
                },
            )
            .await
            .map_err(error)?;
        Ok(Version::from(&row))
    }

    /// Releases each akt's current revision as its next version, all or
    /// nothing; a reference from one to another is checked against what
    /// that one is released as, and pinned to the version it gets
    /// (`version.together`). Answers the versions in order.
    async fn release_versions(
        &self,
        ctx: &Context<'_>,
        input: ReleaseVersionsInput,
    ) -> async_graphql::Result<Vec<Version>> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let releasing = input
            .akter
            .into_iter()
            .map(|a| {
                Ok(crate::akter::versions::Releasing {
                    file: file_of(a.file.as_ref())?,
                    number: a.number,
                    revision: a.revision,
                    label: a.label,
                    description: a.description,
                })
            })
            .collect::<async_graphql::Result<Vec<_>>>()?;
        let rows = akter(ctx)?
            .release_together(caller, &releasing)
            .await
            .map_err(error)?;
        Ok(rows.iter().map(Version::from).collect())
    }

    /// Archives a live akt (`archive.act`).
    async fn archive_akt(
        &self,
        ctx: &Context<'_>,
        input: ArchiveAktInput,
    ) -> async_graphql::Result<Akt> {
        set_archived(ctx, input, true).await
    }

    /// Makes an archived akt live again (`archive.act`).
    async fn unarchive_akt(
        &self,
        ctx: &Context<'_>,
        input: ArchiveAktInput,
    ) -> async_graphql::Result<Akt> {
        set_archived(ctx, input, false).await
    }

    /// Moves an akt along one of its akt type's transitions.
    async fn perform_transition(
        &self,
        ctx: &Context<'_>,
        input: PerformTransitionInput,
    ) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let fields = field_values(input.fields)?;
        let akter = akter(ctx)?;
        let row = akter
            .transition(
                caller,
                &input.number,
                input.revision,
                &input.transition,
                &fields,
                input.reason,
            )
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }
}

#[derive(Default)]
pub struct ContainmentMutation;

/// What adding to or removing from a container takes.
#[derive(InputObject)]
pub struct MembershipInput {
    /// The container's number.
    container: String,
    /// The member's number.
    member: String,
    reason: Option<String>,
}

/// What linking an akt to another takes.
#[derive(InputObject)]
pub struct LinkInput {
    /// The source's number.
    source: String,
    /// The kind of link, as the source's akt type declares it.
    kind: String,
    /// The target's number.
    target: String,
    /// A revision of the target to pin, if any.
    pin_revision: Option<i32>,
    reason: Option<String>,
}

/// What unlinking an akt from another takes.
#[derive(InputObject)]
pub struct UnlinkInput {
    source: String,
    kind: String,
    target: String,
    reason: Option<String>,
}

/// What transferring an akt between containers takes.
#[derive(InputObject)]
pub struct TransferInput {
    /// The akt to transfer.
    member: String,
    /// The container it leaves.
    from: String,
    /// The container it joins.
    to: String,
    reason: Option<String>,
}

/// What drafting a field takes.
#[derive(InputObject)]
pub struct DraftFieldInput {
    number: String,
    /// The revision the draft is based on.
    revision: i32,
    /// The field to draft, which must declare a recipe.
    field: String,
    reason: Option<String>,
}

/// What moving an akt takes: exactly one of `before` and `after`.
#[derive(InputObject)]
pub struct MoveAktInput {
    number: String,
    /// The akt to place it directly before.
    before: Option<String>,
    /// The akt to place it directly after.
    after: Option<String>,
}

#[Object]
impl ContainmentMutation {
    /// Adds an akt to a container, and returns the container.
    async fn add_to_container(
        &self,
        ctx: &Context<'_>,
        input: MembershipInput,
    ) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let akter = akter(ctx)?;
        let row = akter
            .add_member(caller, &input.container, &input.member, input.reason)
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }

    /// Links an akt to another by a kind its akt type declares, and returns
    /// the source.
    async fn add_link(&self, ctx: &Context<'_>, input: LinkInput) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let akter = akter(ctx)?;
        let row = akter
            .add_link(
                caller,
                &input.source,
                &input.kind,
                &input.target,
                input.pin_revision,
                input.reason,
            )
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }

    /// Ends a link between two akter, and returns the source.
    async fn remove_link(
        &self,
        ctx: &Context<'_>,
        input: UnlinkInput,
    ) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let akter = akter(ctx)?;
        let row = akter
            .remove_link(
                caller,
                &input.source,
                &input.kind,
                &input.target,
                input.reason,
            )
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }

    /// Removes an akt from a container, and returns the container.
    async fn remove_from_container(
        &self,
        ctx: &Context<'_>,
        input: MembershipInput,
    ) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let akter = akter(ctx)?;
        let row = akter
            .remove_member(caller, &input.container, &input.member, input.reason)
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }

    /// Transfers an akt from one container to another as one change, and
    /// returns the container it joined.
    async fn transfer_member(
        &self,
        ctx: &Context<'_>,
        input: TransferInput,
    ) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let akter = akter(ctx)?;
        let row = akter
            .transfer_member(caller, &input.member, &input.from, &input.to, input.reason)
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }

    /// Fills a field with the draft its recipe gives, as an edit.
    async fn draft_field(
        &self,
        ctx: &Context<'_>,
        input: DraftFieldInput,
    ) -> async_graphql::Result<Akt> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let akter = akter(ctx)?;
        let row = akter
            .draft_field(
                caller,
                &input.number,
                input.revision,
                &input.field,
                input.reason,
            )
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }

    /// Moves an akt directly before or after another, on every board.
    async fn move_akt(&self, ctx: &Context<'_>, input: MoveAktInput) -> async_graphql::Result<Akt> {
        let place = match (&input.before, &input.after) {
            (Some(before), None) => Place::Before(before),
            (None, Some(after)) => Place::After(after),
            _ => {
                return Err(coded(
                    "give exactly one of before and after".to_owned(),
                    "BAD_REQUEST",
                ));
            }
        };
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
            storage: akter(ctx)?.storage(),
            renderer: akter(ctx)?.renderer(),
        };
        let akter = akter(ctx)?;
        let row = akter
            .move_akt(caller, &input.number, place)
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
    }
}

async fn reload(ctx: &Context<'_>, akter: &Akter, row: AktRow) -> async_graphql::Result<Akt> {
    let tenant: Uuid = principal(ctx)?.tenant.into();
    let mut conn = akter.pool().get().await.map_err(internal)?;
    with_types(&mut conn, tenant, vec![row])
        .await?
        .pop()
        .ok_or_else(|| internal("the akt vanished"))
}

/// One version of an akt type.
pub struct AktType(StoredType);

#[Object]
impl AktType {
    async fn key(&self) -> &str {
        &self.0.row.key
    }

    async fn version(&self) -> i32 {
        self.0.row.version
    }

    async fn name(&self) -> &str {
        &self.0.ty.document().name
    }

    /// The number pattern, such as `CAPA-{year}-{seq:4}`.
    async fn number_pattern(&self) -> &str {
        &self.0.ty.document().number
    }

    async fn fields(&self) -> Vec<Field> {
        self.0
            .ty
            .document()
            .fields
            .iter()
            .map(Field::from)
            .collect()
    }

    /// What the request's subject may do with its akter: what the app
    /// offers by. Every write is still decided on its own.
    // r[impl authz.viewer]
    async fn viewer_may(&self) -> ViewerMay {
        ViewerMay {
            key: self.0.row.key.clone(),
        }
    }

    async fn states(&self) -> Vec<State> {
        self.0
            .ty
            .document()
            .states
            .iter()
            .map(|s| State {
                key: s.key.clone(),
                name: s.name.clone(),
                initial: s.initial,
                is_final: s.is_final,
                tone: self.0.ty.tone(&s.key).into(),
            })
            .collect()
    }

    /// How its akter are shown: the title field, and what cards and lists
    /// show.
    // r[impl akt.type.presentation]
    async fn presentation(&self) -> Presentation {
        let presentation = &self.0.ty.document().presentation;
        Presentation {
            title: self.0.ty.title_field().map(|f| f.key.clone()),
            card: presentation.card.clone(),
            list: presentation.list.clone(),
            icon: presentation.icon.map(Into::into),
            colour: presentation.colour.unwrap_or_default().into(),
            navigation: presentation.navigation.unwrap_or_default().into(),
            image: presentation.image.clone(),
            numbered_references: presentation.numbered_references,
            plural: presentation.plural.clone(),
            group: presentation.group.clone(),
        }
    }

    async fn transitions(&self) -> Vec<Transition> {
        self.0
            .ty
            .document()
            .transitions
            .iter()
            .map(|t| Transition {
                key: t.key.clone(),
                name: t.name.clone(),
                from: t.from.clone(),
                to: t.to.clone(),
                requires: t.requires.clone(),
                outcomes: t
                    .outcomes
                    .iter()
                    .filter_map(|o| kuria_engine::outcomes::Outcome::parse(o))
                    .map(OutcomeKind::from)
                    .collect(),
            })
            .collect()
    }

    /// The document this version was defined by.
    async fn document(&self) -> Json<JsonValue> {
        Json(self.0.row.definition.clone())
    }

    /// Whether its akter have a live draft, edited together, that only
    /// releases make revisions of (`live.declare`).
    async fn live_drafts(&self) -> bool {
        self.0.ty.document().live_drafts
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.0.row.created_at
    }

    /// How many akter of this type the tenant has, to a subject who may
    /// read them; `null` to anyone else.
    // r[impl authz.read]
    async fn akter_count(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Live akter (the default), archived ones, or both.")] archived: Option<
            ArchivedFilter,
        >,
    ) -> async_graphql::Result<Option<i64>> {
        let key = self.0.ty.key();
        if !reads(ctx, key).await? {
            return Ok(None);
        }
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        Ok(Some(
            store::count_akter(&mut conn, tenant, key, archived.unwrap_or_default().into())
                .await
                .map_err(internal)?,
        ))
    }

    async fn created_by(&self) -> ID {
        ID(self.0.row.created_by.to_string())
    }
}

/// Which akter a read answers (`archive.read`).
#[derive(Enum, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArchivedFilter {
    /// Live akter only, the default.
    #[default]
    Live,
    /// Archived akter only.
    Archived,
    /// Both.
    All,
}

impl From<ArchivedFilter> for store::Archived {
    fn from(filter: ArchivedFilter) -> Self {
        match filter {
            ArchivedFilter::Live => Self::Live,
            ArchivedFilter::Archived => Self::Archived,
            ArchivedFilter::All => Self::All,
        }
    }
}

/// How an akt is archived (`archive.flag`).
#[derive(SimpleObject)]
pub struct Archived {
    at: DateTime<Utc>,
    /// Who archived it; `null` when a rule did.
    by: Option<User>,
    /// The kind of rule that archived it, if one did (`archive.rules`).
    rule: Option<String>,
    reason: Option<String>,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Choice,
    Choices,
    Date,
    User,
    Checkbox,
    Reference,
    Git,
    Docker,
    Attachment,
    Sections,
    Verdict,
    Markdown,
}

impl From<ConfigKind> for FieldKind {
    fn from(kind: ConfigKind) -> Self {
        match kind {
            ConfigKind::Text => Self::Text,
            ConfigKind::Choice => Self::Choice,
            ConfigKind::Choices => Self::Choices,
            ConfigKind::Date => Self::Date,
            ConfigKind::User => Self::User,
            ConfigKind::Checkbox => Self::Checkbox,
            ConfigKind::Git => Self::Git,
            ConfigKind::Docker => Self::Docker,
            ConfigKind::Reference => Self::Reference,
            ConfigKind::Attachment => Self::Attachment,
            ConfigKind::Sections => Self::Sections,
            ConfigKind::Verdict => Self::Verdict,
            ConfigKind::Markdown => Self::Markdown,
        }
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "a field's document, as GraphQL answers it"
)]
#[derive(SimpleObject)]
pub struct Field {
    key: String,
    name: String,
    kind: FieldKind,
    description: Option<String>,
    required: bool,
    multiline: bool,
    options: Vec<ChoiceOption>,
    /// For attachment fields: what files they hold.
    format: Option<AttachmentFormat>,
    /// For user fields: whether they hold a list of users and contacts.
    multiple: bool,
    /// For sections fields: the akt types of the members they place; any
    /// the akt type contains if empty.
    of: Vec<String>,
    /// For verdict fields: the key of the user field naming the assignees.
    assignees: Option<String>,
    /// For markdown fields: the link kind its references are kept as.
    references: Option<String>,
    /// For markdown fields: whether its `diagram` blocks are diagrams.
    diagrams: bool,
    /// For attachment fields: the media types it takes, any if none.
    media_types: Vec<String>,
}

/// What files an attachment field holds.
#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentFormat {
    Any,
    Sbom,
}

impl From<&FieldDef> for Field {
    fn from(def: &FieldDef) -> Self {
        Self {
            key: def.key.clone(),
            name: def.name.clone(),
            kind: def.kind.into(),
            description: def.description.clone(),
            required: def.required,
            multiline: def.multiline,
            options: def.options.iter().map(ChoiceOption::from).collect(),
            format: def.format.map(|format| match format {
                ConfigFormat::Any => AttachmentFormat::Any,
                ConfigFormat::Sbom => AttachmentFormat::Sbom,
            }),
            multiple: def.multiple,
            of: def.of.clone(),
            assignees: def.assignees.clone(),
            references: def.references.clone(),
            diagrams: def.diagrams,
            media_types: def.media_types.clone(),
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct ChoiceOption {
    key: String,
    name: String,
}

impl From<&ConfigOption> for ChoiceOption {
    fn from(option: &ConfigOption) -> Self {
        Self {
            key: option.key.clone(),
            name: option.name.clone(),
        }
    }
}

#[derive(SimpleObject)]
pub struct State {
    key: String,
    name: String,
    initial: bool,
    #[graphql(name = "final")]
    is_final: bool,
    /// How the state is shown, as the akt type's presentation says.
    tone: Tone,
}

/// How strongly, and in which sense, a state is shown.
#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Active,
    Success,
    Warning,
    Danger,
}

impl From<kuria_config::Tone> for Tone {
    fn from(tone: kuria_config::Tone) -> Self {
        match tone {
            kuria_config::Tone::Neutral => Self::Neutral,
            kuria_config::Tone::Active => Self::Active,
            kuria_config::Tone::Success => Self::Success,
            kuria_config::Tone::Warning => Self::Warning,
            kuria_config::Tone::Danger => Self::Danger,
        }
    }
}

/// Mirrors a config enum as a GraphQL enum of the same variants.
macro_rules! mirrored {
    ($(#[$meta:meta])* $name:ident from $config:path { $($variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Enum, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($variant),+ }

        impl From<$config> for $name {
            fn from(value: $config) -> Self {
                match value { $(<$config>::$variant => Self::$variant),+ }
            }
        }
    };
}

mirrored! {
    /// An icon an akt type is shown with.
    AktTypeIcon from kuria_config::Icon {
        Archive, BookOpen, Box, Bug, Calendar, CircleCheck, ClipboardCheck, FileText,
        Flag, FlaskConical, GitBranch, Layers, ListChecks, Milestone, NotebookPen,
        Package, Rocket, ShieldCheck, Tag, Target, TriangleAlert, Users, Wrench, Zap,
    }
}

mirrored! {
    /// The colour an akt type is shown in.
    AktTypeColour from kuria_config::Colour {
        Neutral, Blue, Green, Amber, Red, Purple, Teal, Pink,
    }
}

mirrored! {
    /// Where the app's sidebar lists an akt type.
    AktTypeNavigation from kuria_config::Navigation {
        Main, More,
    }
}

/// How an akt type's akter are shown.
#[derive(SimpleObject)]
pub struct Presentation {
    /// The key of the field whose value is each akt's title, if any.
    title: Option<String>,
    /// The fields board cards show after the title.
    card: Vec<String>,
    /// The fields lists of this type's akter show as columns.
    list: Vec<String>,
    /// The icon shown with the type's name, if it has one.
    icon: Option<AktTypeIcon>,
    /// The colour the type is shown in.
    colour: AktTypeColour,
    /// Where the app's sidebar lists the type (`app.navigation`).
    navigation: AktTypeNavigation,
    /// The `attachment` field holding each akt's picture, if its akter are
    /// images (`akt.field.markdown.images`).
    image: Option<String>,
    /// Whether references to its akter are shown with the akt's number
    /// first (`app.references`).
    numbered_references: bool,
    /// The type's name in the plural, if it gives one.
    plural: Option<String>,
    /// The sidebar's heading the type is listed under, if any
    /// (`app.navigation`).
    group: Option<String>,
}

#[derive(SimpleObject)]
pub struct Transition {
    key: String,
    name: String,
    from: Vec<String>,
    to: String,
    requires: Vec<String>,
    /// The outcomes the akt must have for it to be performed; any, or
    /// none, if empty (`akt.transition.outcomes`).
    outcomes: Vec<OutcomeKind>,
}

/// A link between this akt and another, as this akt reads it.
pub struct Link {
    kind: String,
    name: String,
    outgoing: bool,
    by_reference: bool,
    pinned_revision: Option<i32>,
    akt: Akt,
}

#[Object]
impl Link {
    /// The kind's key, as the source's akt type declares it.
    async fn kind(&self) -> &str {
        &self.kind
    }

    /// The link as this akt reads it: the kind's name from the source, its
    /// inverse name from the target.
    async fn name(&self) -> &str {
        &self.name
    }

    /// Whether the link goes from this akt to the other.
    async fn outgoing(&self) -> bool {
        self.outgoing
    }

    /// Whether a reference in the source's text keeps it
    /// (`link.referenced`), so that it is changed by changing the text.
    async fn by_reference(&self) -> bool {
        self.by_reference
    }

    /// The other akt.
    async fn akt(&self) -> &Akt {
        &self.akt
    }

    /// The other akt's revision the link pins, if it pins one.
    async fn pinned_revision(&self) -> Option<i32> {
        self.pinned_revision
    }

    /// The other akt as it was at the pinned revision.
    // r[impl link.pin]
    async fn pinned(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Pinned>> {
        let Some(pinned) = self.pinned_revision else {
            return Ok(None);
        };
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let revisions = store::revisions(&mut conn, tenant, self.akt.row.id)
            .await
            .map_err(internal)?;
        let mut fields = Map::new();
        let mut state = None;
        for revision in revisions.iter().filter(|r| r.revision <= pinned) {
            if let Some(changes) = revision.changes.as_object() {
                for (key, value) in changes {
                    if value.is_null() {
                        fields.remove(key);
                    } else {
                        fields.insert(key.clone(), value.clone());
                    }
                }
            }
            state = Some(revision.state.clone());
        }
        let Some(state) = state else {
            return Ok(None);
        };
        let ty = &self.akt.ty.ty;
        let values = fields_from_json(ty, &fields).map_err(internal)?;
        let def = ty.state(&state).ok_or_else(|| {
            internal(format!(
                "a revision of {} is in an unknown state",
                self.akt.row.number
            ))
        })?;
        Ok(Some(Pinned {
            revision: pinned,
            title: crate::akter::title_of(ty, &values),
            state: State {
                key: def.key.clone(),
                name: def.name.clone(),
                initial: def.initial,
                is_final: def.is_final,
                tone: ty.tone(&def.key).into(),
            },
        }))
    }
}

/// An akt as it was at a revision.
#[derive(SimpleObject)]
pub struct Pinned {
    revision: i32,
    title: Option<String>,
    state: State,
}

/// A container's members in columns.
#[derive(SimpleObject)]
pub struct Board {
    /// The configured columns, then `unmapped`.
    columns: Vec<BoardColumn>,
}

/// One column of a board.
#[derive(SimpleObject)]
pub struct BoardColumn {
    key: String,
    name: String,
    /// Its members, in rank order.
    akter: Vec<Akt>,
}

impl Akt {
    /// Whether it is read as it was, at a version or a revision.
    fn is_past(&self) -> bool {
        self.version.is_some() || self.at_revision
    }

    /// Up to `limit` members that the subject may read, of the akt type
    /// `key` if given, in rank order, in a fixed number of queries.
    async fn readable_members(
        &self,
        ctx: &Context<'_>,
        key: Option<&str>,
        archived: store::Archived,
        limit: i32,
    ) -> async_graphql::Result<Vec<Akt>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut readable = Vec::new();
        for contained in self.ty.ty.document().contains.iter().map(|c| &c.akt_type) {
            if key.is_none_or(|k| k == contained) && reads(ctx, contained).await? {
                readable.push(contained.clone());
            }
        }
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let rows = containers::members_of(
            &mut conn,
            tenant,
            &[self.row.id],
            &readable,
            archived,
            i64::from(limit),
        )
        .await
        .map_err(internal)?
        .into_iter()
        .map(|(_, row)| row)
        .collect();
        with_types(&mut conn, tenant, rows).await
    }
}

/// An akt, with the akt type version it was created with; read at one of
/// its versions, if `version` says so.
pub struct Akt {
    row: AktRow,
    ty: StoredType,
    version: Option<VersionRow>,
    /// Whether it is read at a revision: as it was, not as it is.
    at_revision: bool,
}

#[Object]
impl Akt {
    async fn id(&self) -> ID {
        ID(self.row.id.to_string())
    }

    async fn number(&self) -> &str {
        &self.row.number
    }

    /// Its title: the value of its akt type's title field, if it has one.
    // r[impl akt.title]
    async fn title(&self) -> Option<&str> {
        self.row.title.as_deref()
    }

    /// The akt type version the akt was created with.
    #[graphql(name = "type")]
    async fn akt_type(&self) -> AktType {
        AktType(self.ty.clone())
    }

    /// When, by whom and why it is archived; `null` while it is live
    /// (`archive.read`).
    // r[impl archive.read]
    async fn archived(&self) -> Option<Archived> {
        let at = self.row.archived_at?;
        let by = self.row.archived_by.as_ref();
        Some(Archived {
            at,
            by: by
                .and_then(|b| b["subject"].as_str())
                .and_then(|s| s.parse().ok())
                .map(User),
            rule: by.and_then(|b| b["rule"].as_str()).map(str::to_owned),
            reason: self.row.archive_reason.clone(),
        })
    }

    async fn state(&self) -> async_graphql::Result<State> {
        let state =
            self.ty.ty.state(&self.row.state).ok_or_else(|| {
                internal(format!("akt {} is in an unknown state", self.row.number))
            })?;
        Ok(State {
            key: state.key.clone(),
            name: state.name.clone(),
            initial: state.initial,
            is_final: state.is_final,
            tone: self.ty.ty.tone(&state.key).into(),
        })
    }

    /// The latest revision's number.
    async fn revision(&self) -> i32 {
        self.row.revision
    }

    /// The fields that hold a value, in the akt type's order.
    async fn fields(&self) -> async_graphql::Result<Vec<FieldValue>> {
        let empty = Map::new();
        let values = fields_from_json(&self.ty.ty, self.row.fields.as_object().unwrap_or(&empty))
            .map_err(internal)?;
        Ok(self
            .ty
            .ty
            .document()
            .fields
            .iter()
            .filter_map(|def| {
                values
                    .get(&def.key)
                    .map(|v| FieldValue::new(def, v, &self.pins(), self.row.id))
            })
            .collect())
    }

    /// The version it is read at, if it is read at one (`version.read`).
    async fn version(&self) -> Option<Version> {
        self.version.as_ref().map(Version::from)
    }

    /// Its versions, latest first (`version.list`).
    // r[impl version.list]
    async fn versions(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Version>> {
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        Ok(
            kuria_store::versions::of(&mut conn, self.row.tenant_id, self.row.id)
                .await
                .map_err(internal)?
                .iter()
                .map(Version::from)
                .collect(),
        )
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.row.created_at
    }

    async fn updated_at(&self) -> DateTime<Utc> {
        self.row.updated_at
    }

    /// The akt's place in its tenant's priority order. Lists of akter are
    /// in this order.
    async fn rank(&self) -> &str {
        &self.row.rank
    }

    /// Its members that the subject may read, in rank order: of one akt
    /// type if given, a page of at most `first` (50 by default, 200 at most)
    /// after the cursor `after`.
    // r[impl authz.members]
    // r[impl akt.page]
    #[expect(
        clippy::too_many_arguments,
        reason = "each is one of the field's GraphQL arguments"
    )]
    async fn members(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "type")] key: Option<String>,
        first: Option<i32>,
        after: Option<String>,
        #[graphql(
            name = "where",
            desc = "Conditions on field values, all of which a member must meet; `type` must be given."
        )]
        conditions: Option<Vec<FieldCondition>>,
        #[graphql(
            desc = "Only members with this outcome, as it is now; `type` must be given, and have outcomes."
        )]
        outcome: Option<OutcomeKind>,
        #[graphql(desc = "Live members (the default), archived ones, or both.")] archived: Option<
            ArchivedFilter,
        >,
    ) -> async_graphql::Result<AktConnection> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let size = page_size(first);
        let container = self.row.id.to_string();
        let conditions = conditions.unwrap_or_default();
        let where_key = serde_json::to_string(&conditions).map_err(internal)?;
        let archived = archived.unwrap_or_default();
        let list = List::new(
            "members",
            &[
                Some(&container),
                key.as_deref(),
                Some(&where_key),
                outcome.map(OutcomeKind::key),
                Some(archived_key(archived)),
            ],
        );
        let after = match list.after(after.as_deref())? {
            Some(After::Ranked(rank)) => Some(rank),
            Some(After::Changed(_)) => return Err(bad_cursor()),
            None => None,
        };
        let mut readable = Vec::new();
        for contained in self.ty.ty.document().contains.iter().map(|c| &c.akt_type) {
            if key.as_deref().is_none_or(|k| k == contained) && reads(ctx, contained).await? {
                readable.push(contained.clone());
            }
        }
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let mut conditions = match key.as_deref() {
            _ if conditions.is_empty() => Vec::new(),
            Some(key) => matching(ctx, &mut conn, tenant, key, &conditions).await?,
            None => {
                return Err(coded(
                    "filtering by fields needs `type`".to_owned(),
                    "BAD_REQUEST",
                ));
            }
        };
        let by = match outcome {
            Some(outcome) => Some(by_outcome(&mut conn, tenant, key.as_deref(), outcome).await?),
            None => None,
        };
        let rows = match by {
            Some(ByOutcome::Members(wanted, types)) => {
                let page = Members {
                    container: self.row.id,
                    keys: &readable,
                    archived: archived.into(),
                    conditions: &conditions,
                };
                page.held(&mut conn, tenant, &types, wanted, after, size + 1)
                    .await?
            }
            by => {
                if let Some(ByOutcome::Verdicts(condition)) = by {
                    conditions.push(condition);
                }
                containers::member_page(
                    &mut conn,
                    tenant,
                    self.row.id,
                    &readable,
                    archived.into(),
                    &conditions,
                    after.as_deref(),
                    size + 1,
                )
                .await
                .map_err(internal)?
            }
        };
        let (rows, page_info) = list.page(
            rows,
            size,
            |row| After::Ranked(row.rank.clone()),
            |row| row.id,
        );
        Ok(AktConnection {
            nodes: with_types(&mut conn, tenant, rows).await?,
            page_info,
        })
    }

    /// The akt as a board: its members that the subject may read, in the
    /// columns its akt type configures and then `unmapped`, each in rank
    /// order. At most `first` members are shown (500 by default, 1000 at
    /// most).
    // r[impl board.placement]
    // r[impl board.visibility]
    /// How many of its members the subject may read are archived
    /// (`app.archive`).
    async fn archived_member_count(&self, ctx: &Context<'_>) -> async_graphql::Result<i64> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut readable = Vec::new();
        for contained in self.ty.ty.document().contains.iter().map(|c| &c.akt_type) {
            if reads(ctx, contained).await? {
                readable.push(contained.clone());
            }
        }
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        containers::count_members(
            &mut conn,
            tenant,
            self.row.id,
            &readable,
            store::Archived::Archived,
        )
        .await
        .map_err(internal)
    }

    // r[impl archive.board]
    async fn board(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        #[graphql(
            desc = "Live members (the default, unless the container is archived, when all are), archived ones, or both."
        )]
        archived: Option<ArchivedFilter>,
    ) -> async_graphql::Result<Board> {
        let limit = first.unwrap_or(500).clamp(1, MAX_BOARD);
        let archived = archived.unwrap_or(if self.row.archived {
            ArchivedFilter::All
        } else {
            ArchivedFilter::Live
        });
        let members = self
            .readable_members(ctx, None, archived.into(), limit)
            .await?;
        let config = self.ty.ty.board();
        let mut columns: Vec<BoardColumn> = config
            .map(|board| board.columns.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|column| BoardColumn {
                key: column.key.clone(),
                name: column.name.clone(),
                akter: Vec::new(),
            })
            .collect();
        columns.push(BoardColumn {
            key: UNMAPPED.to_owned(),
            name: "Unmapped".to_owned(),
            akter: Vec::new(),
        });
        // Members arrive in rank order, so each column keeps it.
        for member in members {
            let key = column_of(config, member.ty.ty.key(), &member.row.state);
            if let Some(column) = columns.iter_mut().find(|c| c.key == key) {
                column.akter.push(member);
            }
        }
        Ok(Board { columns })
    }

    /// The containers it is in that the subject may read, in rank order.
    // r[impl authz.members]
    async fn containers(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Akt>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let mut rows = Vec::new();
        for (row, key) in containers::containers_of(&mut conn, tenant, self.row.id)
            .await
            .map_err(internal)?
        {
            if reads(ctx, &key).await? {
                rows.push(row);
            }
        }
        with_types(&mut conn, tenant, rows).await
    }

    /// Its links: those from it, as its akt type names them, and those to
    /// it, by the inverse names their sources' types give them, leaving out
    /// those whose other akt the subject may not read.
    // r[impl link.read]
    async fn links(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Link>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let from = kuria_store::links::from(&mut conn, tenant, self.row.id)
            .await
            .map_err(internal)?;
        let to = kuria_store::links::to(&mut conn, tenant, self.row.id)
            .await
            .map_err(internal)?;
        let mut links = Vec::new();
        for (outgoing, rows) in [(true, from), (false, to)] {
            let (rows, others): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
            let others = with_types(&mut conn, tenant, others).await?;
            for (row, other) in rows.into_iter().zip(others) {
                if !reads(ctx, other.ty.ty.key()).await? {
                    continue;
                }
                let source_ty = if outgoing { &self.ty } else { &other.ty };
                let def = source_ty.ty.link(&row.kind);
                let name = match (def, outgoing) {
                    (Some(def), true) => def.name.clone(),
                    (Some(def), false) => def.inverse.clone(),
                    (None, _) => row.kind.clone(),
                };
                let by_reference = source_ty
                    .ty
                    .document()
                    .fields
                    .iter()
                    .any(|f| f.references.as_deref() == Some(row.kind.as_str()));
                links.push(Link {
                    kind: row.kind,
                    name,
                    outgoing,
                    by_reference,
                    pinned_revision: row.pinned_revision,
                    akt: other,
                });
            }
        }
        Ok(links)
    }

    /// The akt it was copied from, if it is a copy and the viewer may read
    /// it (`akt.copy.record`).
    async fn copied_from(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Akt>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let Some(origin) = store::origin_of(&mut conn, tenant, self.row.id)
            .await
            .map_err(internal)?
        else {
            return Ok(None);
        };
        let akt = with_types(&mut conn, tenant, vec![origin]).await?.pop();
        match akt {
            Some(akt) if reads(ctx, akt.ty.ty.key()).await? => Ok(Some(akt)),
            _ => Ok(None),
        }
    }

    /// What its verdicts, or its members' outcomes, add up to, if its type
    /// has outcomes (`akt.outcome`); derived now, never stored.
    async fn outcome(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<OutcomeKind>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let types = outcomes::Types::of(&mut conn, tenant)
            .await
            .map_err(error)?;
        Ok(outcomes::of(&mut conn, tenant, &types, &self.row)
            .await
            .map_err(error)?
            .map(Into::into))
    }

    /// The changes to its live draft since its latest release, or all of
    /// them with `all`, oldest first: who made each, when, and which fields
    /// it changed (`live.history`). None for an akt without a live draft,
    /// or read at a version or revision.
    // r[impl live.history]
    async fn draft_changes(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] all: bool,
    ) -> async_graphql::Result<Vec<DraftChange>> {
        if !crate::akter::live::has_live_draft(&self.ty) || self.is_past() {
            return Ok(Vec::new());
        }
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let tenant = self.row.tenant_id;
        let after = if all {
            0
        } else {
            kuria_store::drafts::released_seq(&mut conn, tenant, self.row.id)
                .await
                .map_err(internal)?
                .unwrap_or(0)
        };
        Ok(
            kuria_store::drafts::updates_after(&mut conn, tenant, self.row.id, after)
                .await
                .map_err(internal)?
                .into_iter()
                .map(|row| DraftChange {
                    seq: row.seq,
                    by: User(row.by),
                    at: row.at,
                    fields: serde_json::from_value(row.fields).unwrap_or_default(),
                    message: row.message,
                })
                // A refused update, kept with what undid it, changed
                // nothing.
                .filter(|change| !change.fields.is_empty())
                .collect(),
        )
    }

    /// Its live draft's state, for a clone to make updates against
    /// (`live.state`, `clone.server`). None for an akt without a live
    /// draft, or read at a version or revision.
    // r[impl live.state]
    async fn live_state(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<LiveState>> {
        use base64::Engine as _;
        if !crate::akter::live::has_live_draft(&self.ty) || self.is_past() {
            return Ok(None);
        }
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let loaded = crate::akter::live::load(&mut conn, &self.ty, &self.row)
            .await
            .map_err(error)?;
        let base64 = base64::engine::general_purpose::STANDARD;
        Ok(Some(LiveState {
            state: base64.encode(loaded.draft.state()),
            state_vector: base64.encode(loaded.draft.state_vector()),
            base: base64.encode(loaded.draft.base()),
        }))
    }

    /// Every revision, oldest first.
    async fn revisions(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<AktRevision>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        Ok(store::revisions(&mut conn, tenant, self.row.id)
            .await
            .map_err(internal)?
            .into_iter()
            .map(|row| AktRevision {
                row,
                ty: self.ty.clone(),
            })
            .collect())
    }
}

/// One change to an akt's live draft (`live.history`).
#[derive(SimpleObject)]
pub struct DraftChange {
    /// Its place among the live draft's changes, from 1.
    seq: i64,
    /// Who made it.
    by: User,
    at: DateTime<Utc>,
    /// The keys of the fields it changed.
    fields: Vec<String>,
    /// Its message, if it was pushed from a clone (`clone.server`); none
    /// for live typing and other writes.
    message: Option<String>,
}

/// An akt's live draft, as a clone makes updates against it
/// (`live.state`).
#[derive(SimpleObject)]
pub struct LiveState {
    /// The whole draft, as one Yjs v1 update, base64-encoded.
    state: String,
    /// Its state vector, Yjs v1, base64-encoded.
    state_vector: String,
    /// What a push made against it names as its base, base64-encoded: its
    /// Yjs v1 snapshot, its state vector and what it deleted, which moves
    /// with every change, also one that only deletes.
    base: String,
}

/// A field's value, by its kind.
#[derive(Union)]
pub enum FieldValue {
    Text(TextValue),
    Choice(ChoiceValue),
    Choices(ChoicesValue),
    Date(DateValue),
    User(UserValue),
    Checkbox(CheckboxValue),
    Git(GitValue),
    Docker(DockerValue),
    Reference(ReferenceValue),
    Attachment(AttachmentValue),
    Users(UsersValue),
    Sections(SectionsValue),
    Verdict(VerdictValue),
    Markdown(MarkdownValue),
}

/// A `markdown` field's text, with its headings and references
/// (`akt.field.markdown`).
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct MarkdownValue {
    field: String,
    /// The text, every heading with its id.
    markdown: String,
    headings: Vec<MarkdownHeading>,
    #[graphql(skip)]
    found: Vec<kuria_engine::markdown::Reference>,
    #[graphql(skip)]
    embedded: Vec<kuria_engine::markdown::Reference>,
    /// The akt, if the field allows diagrams, whose renderings it holds.
    #[graphql(skip)]
    drawn_by: Option<Uuid>,
    /// What its references meant, if it is read at a version.
    #[graphql(skip)]
    pins: Vec<Pin>,
}

/// A heading of a `markdown` field.
#[derive(SimpleObject)]
pub struct MarkdownHeading {
    id: String,
    /// 1 to 6.
    level: i32,
    text: String,
}

/// A reference in a `markdown` field to an akt, one of its versions and
/// one of its headings (`akt.field.markdown.references`).
#[derive(SimpleObject)]
pub struct MarkdownReference {
    /// As the text says it: `kuria:<number>[@<version>][#<heading>]`.
    destination: String,
    number: String,
    version: Option<i32>,
    heading: Option<String>,
    /// The akt, if it exists and the subject may read it.
    akt: Option<Akt>,
    /// The version a version read pinned it to, if it names none
    /// (`version.pins`).
    pinned_version: Option<i32>,
    /// The revision a version read pinned it to, for a target that had no
    /// version then.
    pinned_revision: Option<i32>,
    /// Why the reference is broken, if it is.
    broken: Option<String>,
}

impl MarkdownValue {
    /// `text`, the value of the `markdown` field `def` of the akt `akt`.
    fn of(def: &FieldDef, text: &str, pins: &[Pin], akt: Uuid) -> Self {
        Self::new(def.key.clone(), text, pins, def.diagrams.then_some(akt))
    }

    fn new(field: String, text: &str, pins: &[Pin], drawn_by: Option<Uuid>) -> Self {
        use kuria_engine::markdown;
        Self {
            drawn_by,
            pins: pins.to_vec(),
            field,
            markdown: text.to_owned(),
            headings: markdown::headings(text)
                .into_iter()
                .filter_map(|h| {
                    Some(MarkdownHeading {
                        id: h.id?,
                        level: i32::from(h.level),
                        text: h.text,
                    })
                })
                .collect(),
            found: markdown::references(text),
            embedded: markdown::images(text),
        }
    }
}

#[ComplexObject]
impl MarkdownValue {
    /// The references in the text, in order, each once, each with its akt
    /// or why it is broken.
    // r[impl akt.field.markdown.references]
    async fn references(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<MarkdownReference>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let auth = authorizer(ctx)?;
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let mut answered = Vec::new();
        for reference in &self.found {
            let target = crate::akter::versions::target(&mut conn, tenant, auth, &reference.number)
                .await
                .map_err(error)?;
            let akt = match &target {
                Some(_) => match store::akt_by_number(&mut conn, tenant, &reference.number)
                    .await
                    .map_err(internal)?
                {
                    Some(row) => with_types(&mut conn, tenant, vec![row]).await?.pop(),
                    None => None,
                },
                None => None,
            };
            // r[impl version.read]
            // Read at a version, a reference naming none follows its pin.
            let (pinned_version, pinned_revision) =
                kuria_engine::versions::as_pinned(reference, &self.pins);
            let pinned_version = pinned_version.filter(|_| reference.version.is_none());
            let as_read = kuria_engine::markdown::Reference {
                version: reference.version.or(pinned_version),
                ..reference.clone()
            };
            let broken = if pinned_revision.is_some() {
                // Unbroken at release, and a revision never changes.
                target
                    .is_none()
                    .then(|| format!("there is no akt `{}`", reference.number))
            } else {
                kuria_engine::markdown::broken(&as_read, target.as_ref())
            };
            answered.push(MarkdownReference {
                destination: reference.to_string(),
                number: reference.number.clone(),
                version: reference.version.and_then(|v| i32::try_from(v).ok()),
                heading: reference.heading.clone(),
                broken,
                akt,
                pinned_version: pinned_version.and_then(|v| i32::try_from(v).ok()),
                pinned_revision,
            });
        }
        Ok(answered)
    }

    /// The diagram blocks of the text, in order, each with its rendering,
    /// in a field that allows them; none in any other.
    // r[impl akt.field.markdown.diagrams]
    async fn diagrams(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<MarkdownDiagram>> {
        let Some(akt) = self.drawn_by else {
            return Ok(Vec::new());
        };
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let held = kuria_store::attachments::diagrams_of(&mut conn, &[akt])
            .await
            .map_err(internal)?;
        Ok(kuria_engine::markdown::diagrams(&self.markdown)
            .into_iter()
            .map(|source| {
                let hash = crate::akter::diagrams::source_hash(&source);
                let attachment = held
                    .iter()
                    .find(|d| d.field == self.field && d.source_sha256 == hash)
                    .map(|d| Attachment(d.attachment_id));
                MarkdownDiagram { source, attachment }
            })
            .collect())
    }

    /// The images the text embeds, in order, each once, each with the
    /// picture it shows or why it is broken.
    // r[impl akt.field.markdown.images]
    // r[impl version.read]
    async fn images(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<MarkdownImage>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let auth = authorizer(ctx)?;
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let mut answered = Vec::new();
        for reference in &self.embedded {
            let target = crate::akter::versions::target(&mut conn, tenant, auth, &reference.number)
                .await
                .map_err(error)?;
            let (pinned_version, pinned_revision) =
                kuria_engine::versions::as_pinned(reference, &self.pins);
            let version = reference.version.or(pinned_version);
            let as_read = kuria_engine::markdown::Reference {
                version,
                ..reference.clone()
            };
            let picture = match &target {
                Some(_) => crate::akter::versions::picture_at(
                    &mut conn,
                    tenant,
                    &reference.number,
                    version,
                    pinned_revision,
                )
                .await
                .map_err(error)?,
                None => None,
            };
            let broken = if pinned_revision.is_some() && target.is_some() {
                // Unbroken at release, and a revision never changes.
                picture
                    .is_none()
                    .then(|| format!("`{}` has no picture", reference.number))
            } else {
                kuria_engine::markdown::broken_image(&as_read, target.as_ref())
            };
            answered.push(MarkdownImage {
                destination: reference.to_string(),
                number: reference.number.clone(),
                version: reference.version.and_then(|v| i32::try_from(v).ok()),
                attachment: picture.filter(|_| broken.is_none()).map(Attachment),
                broken,
            });
        }
        Ok(answered)
    }
}

/// A diagram block of a `markdown` field that allows them
/// (`akt.field.markdown.diagrams`).
#[derive(SimpleObject)]
pub struct MarkdownDiagram {
    /// The block's typst, as the text has it.
    source: String,
    /// Its rendering, an SVG.
    attachment: Option<Attachment>,
}

/// What `renderDiagram` answers: the SVG, or why it didn't compile.
#[derive(SimpleObject)]
pub struct RenderedDiagram {
    svg: Option<String>,
    error: Option<String>,
}

/// An image a `markdown` field embeds (`akt.field.markdown.images`).
#[derive(SimpleObject)]
pub struct MarkdownImage {
    /// As the text says it: `kuria:<number>[@<version>]`.
    destination: String,
    number: String,
    version: Option<i32>,
    /// The picture it shows, as read: at its version, at a version read's
    /// pin, else now.
    attachment: Option<Attachment>,
    /// Why it is broken, if it is.
    broken: Option<String>,
}

/// A version of an akt (`version.number`).
#[derive(SimpleObject)]
pub struct Version {
    number: i32,
    /// The revision it names.
    revision: i32,
    label: Option<String>,
    /// What its releaser said of it, such as what changed.
    description: Option<String>,
    released_by: User,
    released_at: DateTime<Utc>,
    /// What its references naming no version meant (`version.pins`).
    pins: Vec<VersionPin>,
}

/// What a reference meant when its akt was released.
#[derive(SimpleObject)]
pub struct VersionPin {
    destination: String,
    number: String,
    version: Option<i32>,
    revision: Option<i32>,
}

impl From<&VersionRow> for Version {
    fn from(row: &VersionRow) -> Self {
        let pins: Vec<Pin> = serde_json::from_value(row.pins.clone()).unwrap_or_default();
        Self {
            number: row.number,
            revision: row.revision,
            label: row.label.clone(),
            description: row.description.clone(),
            released_by: User(row.released_by),
            released_at: row.released_at,
            pins: pins
                .into_iter()
                .map(|p| VersionPin {
                    destination: p.destination,
                    number: p.number,
                    version: p.version.and_then(|v| i32::try_from(v).ok()),
                    revision: p.revision,
                })
                .collect(),
        }
    }
}

impl Akt {
    /// What its references meant, if it is read at a version.
    fn pins(&self) -> Vec<Pin> {
        self.version
            .as_ref()
            .and_then(|v| serde_json::from_value(v.pins.clone()).ok())
            .unwrap_or_default()
    }
}

/// What releasing a version takes.
#[derive(InputObject)]
pub struct ReleaseVersionInput {
    number: String,
    /// The revision read, which must be the akt's current one.
    revision: i32,
    /// One line, at most 100 characters.
    label: Option<String>,
    /// At most 2,000 characters, over several lines if need be.
    description: Option<String>,
    /// The uploaded PDF the version is released with, for a type whose
    /// versions carry a file (`version.release-file`).
    file: Option<ID>,
}

/// The attachment `file` names, if it names one.
fn file_of(file: Option<&ID>) -> async_graphql::Result<Option<Uuid>> {
    file.map(|id| {
        Uuid::parse_str(id)
            .map_err(|_| coded(format!("`{}` is no attachment", id.as_str()), "BAD_REQUEST"))
    })
    .transpose()
}

/// What archiving or unarchiving an akt takes.
#[derive(InputObject)]
pub struct ArchiveAktInput {
    number: String,
    /// Why, one line of at most 200 characters.
    reason: Option<String>,
}

async fn set_archived(
    ctx: &Context<'_>,
    input: ArchiveAktInput,
    archive: bool,
) -> async_graphql::Result<Akt> {
    let caller = Caller {
        actor: principal(ctx)?.actor(),
        auth: authorizer(ctx)?,
        storage: akter(ctx)?.storage(),
        renderer: akter(ctx)?.renderer(),
    };
    let akter = akter(ctx)?;
    let row = akter
        .set_archived(caller, &input.number, archive, input.reason)
        .await
        .map_err(error)?;
    reload(ctx, akter, row).await
}

/// What releasing several akter together takes.
#[derive(InputObject)]
pub struct ReleaseVersionsInput {
    /// Each akt to release, at the revision read, with its label.
    akter: Vec<ReleaseVersionInput>,
}

/// A `multiple` user field's users and contacts, in order.
#[derive(SimpleObject)]
pub struct UsersValue {
    field: String,
    users: Vec<Someone>,
}

/// An akt's members laid out in sections.
#[derive(SimpleObject)]
pub struct SectionsValue {
    field: String,
    items: Vec<SectionItem>,
    /// The value as `editAkt` takes it, whatever its depth: a list of akt
    /// numbers and `{title, text, collapsible, items}`.
    value: Json<JsonValue>,
}

/// An item of a sections field: a member, by number, or a section.
#[derive(SimpleObject)]
pub struct SectionItem {
    /// The member's number, if it is one.
    number: Option<String>,
    /// The section, if it is one.
    section: Option<Section>,
}

/// A section.
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct Section {
    title: Option<String>,
    /// Markdown.
    text: Option<String>,
    collapsible: bool,
    items: Vec<SectionItem>,
}

impl SectionItem {
    fn of(items: &[kuria_engine::sections::Item]) -> Vec<Self> {
        items
            .iter()
            .map(|item| match item {
                kuria_engine::sections::Item::Akt(number) => Self {
                    number: Some(number.clone()),
                    section: None,
                },
                kuria_engine::sections::Item::Section(s) => Self {
                    number: None,
                    section: Some(Section {
                        title: s.title.clone(),
                        text: s.text.clone(),
                        collapsible: s.collapsible,
                        items: Self::of(&s.items),
                    }),
                },
            })
            .collect()
    }
}

/// What a section's tests add up to (`akt.outcome.section`).
#[ComplexObject]
impl Section {
    async fn outcome(&self, ctx: &Context<'_>) -> async_graphql::Result<OutcomeKind> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        let types = outcomes::Types::of(&mut conn, tenant)
            .await
            .map_err(error)?;
        Ok(section_outcome(&mut conn, tenant, &types, &self.items)
            .await
            .map_err(error)?
            .into())
    }
}

/// The outcome of a section holding `items`, by the same rules as a
/// container's.
// r[impl akt.outcome.section]
async fn section_outcome(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    types: &outcomes::Types,
    items: &[SectionItem],
) -> Result<kuria_engine::outcomes::Outcome, AktError> {
    let mut held = Vec::new();
    for item in items {
        if let Some(number) = &item.number {
            if let Some(outcome) = outcomes::of_number(conn, tenant, types, number).await? {
                held.push(outcome);
            }
        } else if let Some(section) = &item.section {
            held.push(Box::pin(section_outcome(conn, tenant, types, &section.items)).await?);
        }
    }
    Ok(kuria_engine::outcomes::of_all(held))
}

/// An outcome: what verdicts, or what a container holds, add up to.
#[derive(Enum, Clone, Copy, PartialEq, Eq)]
#[graphql(name = "Outcome")]
pub enum OutcomeKind {
    Pass,
    Fail,
    Skip,
    Pending,
}

impl OutcomeKind {
    /// As the engine names it.
    fn key(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Skip => "skip",
            Self::Pending => "pending",
        }
    }
}

impl From<kuria_engine::outcomes::Outcome> for OutcomeKind {
    fn from(outcome: kuria_engine::outcomes::Outcome) -> Self {
        use kuria_engine::outcomes::Outcome as O;
        match outcome {
            O::Pass => Self::Pass,
            O::Fail => Self::Fail,
            O::Skip => Self::Skip,
            O::Pending => Self::Pending,
        }
    }
}

/// Each assignee's verdict.
#[derive(SimpleObject)]
pub struct VerdictValue {
    field: String,
    verdicts: Vec<GivenVerdict>,
}

/// One assignee's verdict.
#[derive(SimpleObject)]
pub struct GivenVerdict {
    assignee: Someone,
    verdict: VerdictKind,
    /// What the assignee saw: a fail or a skip given since has one.
    description: Option<String>,
    /// Who gave it: the assignee, unless given for them.
    by: User,
    at: DateTime<Utc>,
}

impl From<&kuria_engine::verdicts::Given> for GivenVerdict {
    fn from(g: &kuria_engine::verdicts::Given) -> Self {
        Self {
            assignee: g.assignee.into(),
            verdict: match g.verdict {
                kuria_engine::verdicts::Verdict::Pass => VerdictKind::Pass,
                kuria_engine::verdicts::Verdict::Fail => VerdictKind::Fail,
                kuria_engine::verdicts::Verdict::Skip => VerdictKind::Skip,
            },
            description: g.description.clone(),
            by: User(g.by),
            at: g.at,
        }
    }
}

/// A verdict: pass, fail or skip.
#[derive(Enum, Clone, Copy, PartialEq, Eq)]
#[graphql(name = "Verdict")]
pub enum VerdictKind {
    Pass,
    Fail,
    Skip,
}

impl FieldValue {
    fn new(def: &FieldDef, value: &Value, pins: &[Pin], akt: Uuid) -> Self {
        let field = def.key.clone();
        let option = |key: &str| {
            def.options.iter().find(|o| o.key == key).map_or_else(
                || ChoiceOption {
                    key: key.to_owned(),
                    name: key.to_owned(),
                },
                ChoiceOption::from,
            )
        };
        match value {
            Value::Text(text) => Self::Text(TextValue {
                field,
                text: text.clone(),
            }),
            Value::Markdown(text) => Self::Markdown(MarkdownValue::of(def, text, pins, akt)),
            Value::Choice(key) => Self::Choice(ChoiceValue {
                field,
                option: option(key),
            }),
            Value::Choices(keys) => Self::Choices(ChoicesValue {
                field,
                options: keys.iter().map(|k| option(k)).collect(),
            }),
            Value::Date(date) => Self::Date(DateValue {
                field,
                date: date.format("%Y-%m-%d").to_string(),
            }),
            Value::User(someone) => Self::User(UserValue {
                field,
                user: (*someone).into(),
            }),
            Value::Checkbox {
                by,
                at,
                on_behalf_of,
            } => Self::Checkbox(CheckboxValue {
                field,
                checked: true,
                by: User(*by),
                at: *at,
                on_behalf_of: on_behalf_of
                    .unwrap_or(kuria_engine::Someone::User(*by))
                    .into(),
            }),
            Value::Git {
                repository,
                sha,
                tag,
            } => Self::Git(GitValue {
                field,
                repository: repository.clone(),
                sha: sha.clone(),
                tag: tag.clone(),
            }),
            Value::Docker {
                repository,
                digest,
                tag,
            } => Self::Docker(DockerValue {
                field,
                repository: repository.clone(),
                digest: digest.clone(),
                tag: tag.clone(),
            }),
            Value::Reference { url, label } => Self::Reference(ReferenceValue {
                field,
                url: url.clone(),
                label: label.clone(),
            }),
            Value::Attachment(id) => Self::Attachment(AttachmentValue {
                field,
                attachment: Attachment(*id),
            }),
            Value::Users(people) => Self::Users(UsersValue {
                field,
                users: people.iter().map(|s| (*s).into()).collect(),
            }),
            Value::Sections(items) => Self::Sections(SectionsValue {
                field,
                items: SectionItem::of(items),
                value: Json(value.to_json()),
            }),
            Value::Verdicts(given) => Self::Verdict(VerdictValue {
                field,
                verdicts: given.iter().map(GivenVerdict::from).collect(),
            }),
        }
    }
}

#[derive(SimpleObject)]
pub struct AttachmentValue {
    field: String,
    attachment: Attachment,
}

#[derive(SimpleObject)]
pub struct TextValue {
    field: String,
    text: String,
}

#[derive(SimpleObject)]
pub struct ChoiceValue {
    field: String,
    option: ChoiceOption,
}

#[derive(SimpleObject)]
pub struct ChoicesValue {
    field: String,
    options: Vec<ChoiceOption>,
}

#[derive(SimpleObject)]
pub struct DateValue {
    field: String,
    /// `YYYY-MM-DD`.
    date: String,
}

#[derive(SimpleObject)]
pub struct UserValue {
    field: String,
    /// The user or contact it names.
    user: Someone,
}

/// A checked box. An unchecked box holds no value, so it isn't listed.
#[derive(SimpleObject)]
pub struct CheckboxValue {
    field: String,
    /// Always true: listed boxes are checked.
    checked: bool,
    /// Who checked it.
    by: User,
    /// When it was checked.
    at: DateTime<Utc>,
    /// Whom it was checked for: the user who checked it, unless they
    /// checked it for someone else.
    #[graphql(name = "for")]
    on_behalf_of: Someone,
}

/// A commit in a git repository.
#[derive(SimpleObject)]
pub struct GitValue {
    field: String,
    repository: String,
    sha: Option<String>,
    tag: Option<String>,
}

/// A container image.
#[derive(SimpleObject)]
pub struct DockerValue {
    field: String,
    repository: String,
    /// `sha256:` and 64 hexadecimal digits.
    digest: String,
    tag: Option<String>,
}

#[derive(SimpleObject)]
pub struct ReferenceValue {
    field: String,
    url: String,
    label: Option<String>,
}

/// One recorded change to an akt.
/// One revision of an akt, with the akt type version its values are read by.
pub struct AktRevision {
    row: RevisionRow,
    ty: StoredType,
}

#[Object]
impl AktRevision {
    async fn revision(&self) -> i32 {
        self.row.revision
    }

    /// The git commit the revision was pushed in, if it was, as bundles
    /// carry it: `{author, committer, message}`.
    async fn commit(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Json<JsonValue>>> {
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        Ok(
            kuria_store::versions::commits_of(&mut conn, self.row.tenant_id, self.row.akt_id)
                .await
                .map_err(internal)?
                .into_iter()
                .find(|(revision, _)| *revision == self.row.revision)
                .map(|(_, commit)| Json(commit)),
        )
    }

    /// The subject who made the change.
    async fn author(&self) -> ID {
        ID(self.row.author.to_string())
    }

    /// Who made the change, shown by name.
    async fn by(&self) -> User {
        User(self.row.author)
    }

    async fn transition(&self) -> Option<&str> {
        self.row.transition.as_deref()
    }

    async fn reason(&self) -> Option<&str> {
        self.row.reason.as_deref()
    }

    /// The field values the change set, with `null` for those it cleared.
    async fn changes(&self) -> Json<JsonValue> {
        Json(self.row.changes.clone())
    }

    /// The users and contacts that the values the change set name: a user
    /// field's, and a checkbox's checker and whom it was checked for, so
    /// that the history can show them by name (`contact.show`).
    // r[impl app.history]
    async fn named(&self) -> Vec<Someone> {
        let Some(changes) = self.row.changes.as_object() else {
            return Vec::new();
        };
        let mut named = Vec::new();
        for (key, json) in changes {
            let Some(def) = self.ty.ty.document().fields.iter().find(|f| &f.key == key) else {
                continue;
            };
            if let Ok(value) = Value::from_json(def, json) {
                for someone in value.named() {
                    if !named.contains(&someone) {
                        named.push(someone);
                    }
                }
            }
        }
        named.into_iter().map(Someone::from).collect()
    }

    /// The state the akt was left in.
    async fn state(&self) -> &str {
        &self.row.state
    }

    /// For a membership revision, what it added or removed: `added` or
    /// `removed` with the containers on a member, `added_member` or
    /// `removed_member` with the members on a container.
    async fn membership(&self) -> Option<Json<JsonValue>> {
        self.row.membership.clone().map(Json)
    }

    /// SHA-256 of the resulting state and field values, in hex.
    async fn hash(&self) -> String {
        use std::fmt::Write;
        self.row.hash.iter().fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        })
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.row.created_at
    }
}

/// An archived filter as a list's cursor keeps it.
fn archived_key(archived: ArchivedFilter) -> &'static str {
    match archived {
        ArchivedFilter::Live => "live",
        ArchivedFilter::Archived => "archived",
        ArchivedFilter::All => "all",
    }
}

/// What the request's subject may do with an akt type's akter
/// (`authz.viewer`), each decided only when asked for.
pub struct ViewerMay {
    key: String,
}

impl ViewerMay {
    async fn allows(
        &self,
        ctx: &Context<'_>,
        permission: Permission,
    ) -> async_graphql::Result<bool> {
        authorizer(ctx)?
            .allows(&permission)
            .await
            .map_err(|e| error(e.into()))
    }
}

#[Object]
impl ViewerMay {
    /// Whether they may create its akter.
    async fn create(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        self.allows(
            ctx,
            Permission::Create {
                key: self.key.clone(),
            },
        )
        .await
    }

    /// Whether they may edit its akter.
    async fn edit(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        self.allows(
            ctx,
            Permission::Edit {
                key: self.key.clone(),
            },
        )
        .await
    }

    /// Whether they may perform its transitions, those that name no
    /// action of their own.
    async fn transition(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        self.allows(
            ctx,
            Permission::Transition {
                key: self.key.clone(),
                action: None,
            },
        )
        .await
    }

    /// Whether they may archive and unarchive its akter.
    async fn archive(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        self.allows(
            ctx,
            Permission::Archive {
                key: self.key.clone(),
            },
        )
        .await
    }
}
