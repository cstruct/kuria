//! The GraphQL schema.

use async_graphql::{Context, EmptyMutation, EmptySubscription, ID, Object, Schema, SimpleObject};
use konklav_core::{SubjectId, TenantId};

/// The schema the GraphQL endpoint serves.
pub type KuriaSchema = Schema<Query, EmptyMutation, EmptySubscription>;

/// Builds the schema.
#[must_use]
pub fn schema() -> KuriaSchema {
    Schema::build(Query, EmptyMutation, EmptySubscription).finish()
}

/// Who is making a request, from its verified token. Every request's data
/// holds one; resolvers read the tenant from here and nowhere else.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Principal {
    pub subject: SubjectId,
    pub tenant: TenantId,
}

pub struct Query;

#[Object]
impl Query {
    /// The subject making this request, in the tenant of its token.
    // r[impl auth.viewer]
    #[allow(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "async-graphql requires resolvers to be async"
    )]
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
