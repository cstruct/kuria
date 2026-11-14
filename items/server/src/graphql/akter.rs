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
use kuria_engine::{Value, fields_from_json};
use kuria_store::akter::{self as store, AktRow, ChangedAfter, RevisionRow};
use kuria_store::containers;
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
        AktError::Unavailable(detail) => {
            tracing::warn!("akter: {detail}");
            coded(
                "authorization is unavailable; try again".to_owned(),
                "UNAVAILABLE",
            )
        }
        AktError::Stale { current, .. } => {
            async_graphql::Error::new(message).extend_with(|_, e| {
                e.set("code", "STALE");
                e.set("currentRevision", current);
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

    /// The akt numbered `number`.
    async fn akt(&self, ctx: &Context<'_>, number: String) -> async_graphql::Result<Option<Akt>> {
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
        match akt {
            Some(akt) if reads(ctx, akt.ty.ty.key()).await? => Ok(Some(akt)),
            _ => Ok(None),
        }
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
    ) -> async_graphql::Result<AktConnection> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let size = page_size(first);
        let conditions = conditions.unwrap_or_default();
        let where_key = serde_json::to_string(&conditions).map_err(internal)?;
        let list = List::new(
            if search.is_some() { "search" } else { "akter" },
            &[
                key.as_deref(),
                state.as_deref(),
                search.as_deref(),
                Some(&where_key),
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
        let conditions = match key.as_deref() {
            _ if conditions.is_empty() => Vec::new(),
            Some(key) => matching(ctx, &mut conn, tenant, key, &conditions).await?,
            None => {
                return Err(coded(
                    "filtering by fields needs `type`".to_owned(),
                    "BAD_REQUEST",
                ));
            }
        };
        let listing = store::Listing {
            keys: &readable,
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
    rows.into_iter()
        .map(|row| {
            let ty = types
                .get(&row.type_version_id)
                .cloned()
                .ok_or_else(|| internal(format!("akt {} has no akt type", row.number)))?;
            Ok(Akt { row, ty })
        })
        .collect()
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

    /// Submits an akt type document, creating the next version of its key.
    async fn define_akt_type(
        &self,
        ctx: &Context<'_>,
        document: Json<JsonValue>,
    ) -> async_graphql::Result<AktType> {
        let caller = Caller {
            actor: principal(ctx)?.actor(),
            auth: authorizer(ctx)?,
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
        };
        let fields = field_values(input.fields)?;
        let akter = akter(ctx)?;
        let row = akter
            .create(caller, &input.key, &fields, input.reason, &input.inside)
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
        };
        let fields = field_values(Some(input.fields))?;
        let akter = akter(ctx)?;
        let row = akter
            .edit(caller, &input.number, input.revision, &fields, input.reason)
            .await
            .map_err(error)?;
        reload(ctx, akter, row).await
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
            })
            .collect()
    }

    /// The document this version was defined by.
    async fn document(&self) -> Json<JsonValue> {
        Json(self.0.row.definition.clone())
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.0.row.created_at
    }

    /// How many akter of this type the tenant has, to a subject who may
    /// read them; `null` to anyone else.
    // r[impl authz.read]
    async fn akter_count(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<i64>> {
        let key = self.0.ty.key();
        if !reads(ctx, key).await? {
            return Ok(None);
        }
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        Ok(Some(
            store::count_akter(&mut conn, tenant, key)
                .await
                .map_err(internal)?,
        ))
    }

    async fn created_by(&self) -> ID {
        ID(self.0.row.created_by.to_string())
    }
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
        }
    }
}

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
}

#[derive(SimpleObject)]
pub struct Transition {
    key: String,
    name: String,
    from: Vec<String>,
    to: String,
    requires: Vec<String>,
}

/// A link between this akt and another, as this akt reads it.
pub struct Link {
    kind: String,
    name: String,
    outgoing: bool,
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
    /// Up to `limit` members that the subject may read, of the akt type
    /// `key` if given, in rank order, in a fixed number of queries.
    async fn readable_members(
        &self,
        ctx: &Context<'_>,
        key: Option<&str>,
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

/// An akt, with the akt type version it was created with.
pub struct Akt {
    row: AktRow,
    ty: StoredType,
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
            .filter_map(|def| values.get(&def.key).map(|v| FieldValue::new(def, v)))
            .collect())
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
    ) -> async_graphql::Result<AktConnection> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let size = page_size(first);
        let container = self.row.id.to_string();
        let conditions = conditions.unwrap_or_default();
        let where_key = serde_json::to_string(&conditions).map_err(internal)?;
        let list = List::new(
            "members",
            &[Some(&container), key.as_deref(), Some(&where_key)],
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
        let conditions = match key.as_deref() {
            _ if conditions.is_empty() => Vec::new(),
            Some(key) => matching(ctx, &mut conn, tenant, key, &conditions).await?,
            None => {
                return Err(coded(
                    "filtering by fields needs `type`".to_owned(),
                    "BAD_REQUEST",
                ));
            }
        };
        let rows = containers::member_page(
            &mut conn,
            tenant,
            self.row.id,
            &readable,
            &conditions,
            after.as_deref(),
            size + 1,
        )
        .await
        .map_err(internal)?;
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
    async fn board(&self, ctx: &Context<'_>, first: Option<i32>) -> async_graphql::Result<Board> {
        let limit = first.unwrap_or(500).clamp(1, MAX_BOARD);
        let members = self.readable_members(ctx, None, limit).await?;
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
                links.push(Link {
                    kind: row.kind,
                    name,
                    outgoing,
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
    /// Who gave it: the assignee, unless given for them.
    by: User,
    at: DateTime<Utc>,
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
    fn new(def: &FieldDef, value: &Value) -> Self {
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
            }),
            Value::Verdicts(given) => Self::Verdict(VerdictValue {
                field,
                verdicts: given
                    .iter()
                    .map(|g| GivenVerdict {
                        assignee: g.assignee.into(),
                        verdict: match g.verdict {
                            kuria_engine::verdicts::Verdict::Pass => VerdictKind::Pass,
                            kuria_engine::verdicts::Verdict::Fail => VerdictKind::Fail,
                            kuria_engine::verdicts::Verdict::Skip => VerdictKind::Skip,
                        },
                        by: User(g.by),
                        at: g.at,
                    })
                    .collect(),
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
