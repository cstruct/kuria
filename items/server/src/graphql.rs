//! The GraphQL schema.
//!
//! - [`akter`]: akt types, akter and their revisions, and the mutations
//!   that change them.

// async-graphql requires every resolver to be async, also those that don't
// await anything.
#![allow(clippy::unused_async, clippy::unused_async_trait_impl)]

mod akter;
mod attachments;
mod changes;
mod people;

pub(crate) use people::display_name;

use async_graphql::{Context, ID, Object, Schema, SimpleObject};
use konklav_core::{SubjectId, TenantId};

use crate::akter::{Actor, Akter};
use crate::changes::Hub;

/// The schema the GraphQL endpoint serves.
pub type KuriaSchema = Schema<Query, Mutation, changes::ChangesSubscription>;

/// Builds the schema, storing akter through `akter`, and serving the
/// `changes` subscription from `hub` if there is one.
#[must_use]
pub fn schema(akter: Akter, hub: Option<Hub>) -> KuriaSchema {
    Schema::build(
        Query::default(),
        Mutation::default(),
        changes::ChangesSubscription,
    )
    .data(akter)
    .data(hub)
    .limit_depth(12)
    .limit_complexity(2_000)
    .finish()
}

/// Who is making a request, from its verified token. Every request's data
/// holds one; resolvers read the tenant from here and nowhere else.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Principal {
    pub subject: SubjectId,
    pub tenant: TenantId,
    /// When the access token expires, in Unix seconds.
    pub expires_at: i64,
}

impl Principal {
    fn actor(self) -> Actor {
        Actor {
            subject: self.subject,
            tenant: self.tenant,
        }
    }
}

#[derive(async_graphql::MergedObject, Default)]
pub struct Query(ViewerQuery, akter::AkterQuery);

#[derive(async_graphql::MergedObject, Default)]
pub struct Mutation(akter::AkterMutation, akter::ContainmentMutation);

#[derive(Default)]
pub struct ViewerQuery;

#[Object]
impl ViewerQuery {
    /// The subject making this request, in the tenant of its token.
    // r[impl auth.viewer]
    async fn viewer(&self, ctx: &Context<'_>) -> async_graphql::Result<Viewer> {
        let principal = ctx.data::<Principal>()?;
        Ok(Viewer {
            subject: ID(principal.subject.to_string()),
            tenant: ID(principal.tenant.to_string()),
        })
    }
}

/// The subject making a request, in the tenant of its token.
#[derive(SimpleObject)]
pub struct Viewer {
    /// The subject's Konklav identifier.
    subject: ID,
    /// The tenant the request acts in.
    tenant: ID,
}
