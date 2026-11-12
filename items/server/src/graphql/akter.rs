//! Akt types, akter and akt revisions in GraphQL.

// async-graphql requires every resolver to be async, also those that don't
// await anything.
#![allow(clippy::unused_async, clippy::unused_async_trait_impl)]

use std::collections::HashMap;

use async_graphql::{
    Context, Enum, ErrorExtensions, ID, InputObject, Json, Object, SimpleObject, Union,
};
use chrono::{DateTime, Utc};
use kuria_config::{ChoiceOption as ConfigOption, FieldDef, FieldKind as ConfigKind};
use kuria_engine::board::{UNMAPPED, column_of};
use kuria_engine::{Value, fields_from_json};
use kuria_store::akter::{self as store, AktRow, RevisionRow};
use kuria_store::containers;
use serde_json::{Map, Value as JsonValue};
use uuid::Uuid;

use super::Principal;
use crate::akter::{AktError, Akter, Caller, Place, StoredType};
use crate::authorize::{Authorizer, Permission};

/// The most akter one query lists.
const MAX_LIST: i32 = 200;

/// The most members one board shows.
const MAX_BOARD: i32 = 1_000;

fn principal<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a Principal> {
    ctx.data::<Principal>()
}

fn akter<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a Akter> {
    ctx.data::<Akter>()
}

fn authorizer<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a Authorizer> {
    ctx.data::<Authorizer>()
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
        AktError::NotFound(_) => coded(message, "NOT_FOUND"),
        AktError::Refused(_) | AktError::NotDraftable(_) => coded(message, "REFUSED"),
        AktError::Forbidden => coded(message, "FORBIDDEN"),
        AktError::NotContainable { .. } => coded(message, "NOT_CONTAINABLE"),
        AktError::AlreadyMember { .. } => coded(message, "ALREADY_MEMBER"),
        AktError::NotMember { .. } => coded(message, "NOT_MEMBER"),
        AktError::Exclusive { .. } => coded(message, "EXCLUSIVE"),
        AktError::Cycle { .. } => coded(message, "CYCLE"),
        AktError::Frozen(_) => coded(message, "FROZEN"),
        AktError::CreatedIn { .. } => coded(message, "CREATED_IN"),
        AktError::SelfMove => coded(message, "BAD_REQUEST"),
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

    /// Akter, newest first: of one akt type and in one state, if given, and
    /// at most `first` of them (50 by default, 200 at most).
    async fn akter(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "type")] key: Option<String>,
        state: Option<String>,
        first: Option<i32>,
    ) -> async_graphql::Result<Vec<Akt>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let limit = first.unwrap_or(50).clamp(1, MAX_LIST);
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        // r[impl authz.read]
        // Only the akt types the subject may read are asked for, so the
        // limit counts akter they can see.
        let mut readable = Vec::new();
        for ty in store::latest_type_versions(&mut conn, tenant)
            .await
            .map_err(internal)?
        {
            if key.as_ref().is_none_or(|k| *k == ty.key) && reads(ctx, &ty.key).await? {
                readable.push(ty.key);
            }
        }
        let rows = store::list_akter(
            &mut conn,
            tenant,
            &readable,
            state.as_deref(),
            i64::from(limit),
        )
        .await
        .map_err(internal)?;
        with_types(&mut conn, tenant, rows).await
    }
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

#[Object]
impl AkterMutation {
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
            })
            .collect()
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
    Subject,
    Reference,
}

impl From<ConfigKind> for FieldKind {
    fn from(kind: ConfigKind) -> Self {
        match kind {
            ConfigKind::Text => Self::Text,
            ConfigKind::Choice => Self::Choice,
            ConfigKind::Choices => Self::Choices,
            ConfigKind::Date => Self::Date,
            ConfigKind::Subject => Self::Subject,
            ConfigKind::Reference => Self::Reference,
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
}

#[derive(SimpleObject)]
pub struct Transition {
    key: String,
    name: String,
    from: Vec<String>,
    to: String,
    requires: Vec<String>,
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
    /// type if given, and at most `first` (50 by default, 200 at most).
    // r[impl authz.members]
    async fn members(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "type")] key: Option<String>,
        first: Option<i32>,
    ) -> async_graphql::Result<Vec<Akt>> {
        let limit = first.unwrap_or(50).clamp(1, MAX_LIST);
        self.readable_members(ctx, key.as_deref(), limit).await
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

    /// Every revision, oldest first.
    async fn revisions(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<AktRevision>> {
        let tenant: Uuid = principal(ctx)?.tenant.into();
        let mut conn = akter(ctx)?.pool().get().await.map_err(internal)?;
        Ok(store::revisions(&mut conn, tenant, self.row.id)
            .await
            .map_err(internal)?
            .into_iter()
            .map(AktRevision)
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
    Subject(SubjectValue),
    Reference(ReferenceValue),
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
            Value::Subject(subject) => Self::Subject(SubjectValue {
                field,
                subject: ID(subject.to_string()),
            }),
            Value::Reference { url, label } => Self::Reference(ReferenceValue {
                field,
                url: url.clone(),
                label: label.clone(),
            }),
        }
    }
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
pub struct SubjectValue {
    field: String,
    subject: ID,
}

#[derive(SimpleObject)]
pub struct ReferenceValue {
    field: String,
    url: String,
    label: Option<String>,
}

/// One recorded change to an akt.
pub struct AktRevision(RevisionRow);

#[Object]
impl AktRevision {
    async fn revision(&self) -> i32 {
        self.0.revision
    }

    /// The subject who made the change.
    async fn author(&self) -> ID {
        ID(self.0.author.to_string())
    }

    async fn transition(&self) -> Option<&str> {
        self.0.transition.as_deref()
    }

    async fn reason(&self) -> Option<&str> {
        self.0.reason.as_deref()
    }

    /// The field values the change set, with `null` for those it cleared.
    async fn changes(&self) -> Json<JsonValue> {
        Json(self.0.changes.clone())
    }

    /// The state the akt was left in.
    async fn state(&self) -> &str {
        &self.0.state
    }

    /// For a membership revision, what it added or removed: `added` or
    /// `removed` with the containers on a member, `added_member` or
    /// `removed_member` with the members on a container.
    async fn membership(&self) -> Option<Json<JsonValue>> {
        self.0.membership.clone().map(Json)
    }

    /// SHA-256 of the resulting state and field values, in hex.
    async fn hash(&self) -> String {
        use std::fmt::Write;
        self.0.hash.iter().fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        })
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at
    }
}
