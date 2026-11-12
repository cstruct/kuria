//! The HTTP routes and their Konklav route policies.

use std::sync::Arc;

use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::Router;
use axum::extract::State;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use konklav_axum::rate_limit::TemplateError;
use konklav_axum::{
    InvalidProxy, Konklav, Policy, RelyingHost, RoutePolicy, RoutePolicyError, RouterPolicy,
    TenantToken,
};
use konklav_token::TokenKind;

use crate::akter::Akter;
use crate::authorize::{Authorizer, Decider};
use crate::graphql::{KuriaSchema, Principal, schema};
use crate::session::{self, BrowserSessions, SessionState};
use kuria_store::Pool;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid route policy: {0}")]
    Policy(#[from] RoutePolicyError),
    #[error("invalid trusted proxy: {0}")]
    Proxy(#[from] InvalidProxy),
    #[error("invalid rate-limit key: {0}")]
    RateLimitKey(#[from] TemplateError),
}

/// How the router is set up.
pub struct Options {
    /// The URL clients address the server by, without a trailing slash.
    pub public_url: String,
    /// The proxies whose `X-Forwarded-For` names the client, as addresses or
    /// CIDR networks.
    pub trusted_proxies: Vec<String>,
    /// Kuria's database, for akt types and akter. Connections are opened
    /// when first needed.
    pub pool: Pool,
    /// What decides permissions for a request's token: the relying client.
    pub decider: Arc<dyn Decider>,
    /// Browser sessions. Without them, only token-authenticated requests are
    /// served.
    pub sessions: Option<Arc<BrowserSessions>>,
}

/// The server's router, deciding every request through `host`. Serve it
/// with connect info (`into_make_service_with_connect_info::<SocketAddr>()`),
/// so client addresses are known.
// r[impl auth.relying-system]
pub fn app(host: Arc<dyn RelyingHost>, options: Options) -> Result<Router, AppError> {
    let Options {
        public_url,
        trusted_proxies,
        pool,
        decider,
        sessions,
    } = options;
    let konklav = Konklav::builder(host.clone())
        // r[impl auth.dpop]
        .public_url(move |parts: &Parts| format!("{public_url}{}", parts.uri.path()))
        // r[impl session.login.rate-limit]
        .trusted_proxies(trusted_proxies)
        .build()?;

    // r[impl auth.graphql.unauthenticated]
    // r[impl auth.graphql.tenant-token]
    // r[impl auth.unavailable]
    let api = RouterPolicy::new(Policy::new().token_kinds([TokenKind::Tenant]));

    let mut router = Router::new()
        // r[impl auth.health]
        .route(
            "/healthz",
            get(|| async { "ok" }).route_layer(RoutePolicy::public()),
        )
        .route(
            "/graphql",
            post(graphql).route_layer(api.route(&Policy::new())?),
        )
        .with_state(GraphqlState {
            schema: schema(Akter::new(pool)),
            decider,
        });

    if let Some(sessions) = sessions {
        let state = SessionState { sessions, host };
        router = router
            .merge(session::router(state.clone())?)
            .layer(konklav.layer())
            // Outside Konklav's layer, so it runs first and hands it the
            // session's token.
            .layer(middleware::from_fn_with_state(state, session::authenticate));
    } else {
        router = router.layer(konklav.layer());
    }
    Ok(router)
}

/// What the GraphQL endpoint needs.
#[derive(Clone)]
struct GraphqlState {
    schema: KuriaSchema,
    decider: Arc<dyn Decider>,
}

/// Executes a GraphQL request as the subject and in the tenant of its
/// verified token.
// r[impl auth.tenant]
async fn graphql(
    State(state): State<GraphqlState>,
    token: TenantToken,
    headers: HeaderMap,
    request: GraphQLRequest,
) -> Response {
    // r[impl authz.sender-constrained]
    if token.cnf.is_some() {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(
                serde_json::json!({ "error": { "code": "sender_constrained_unsupported" } }),
            ),
        )
            .into_response();
    }
    // Verified by Konklav's layer; a bearer token, since it has no `cnf`.
    let Some(bearer) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token.trim().to_owned())
    else {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(
                serde_json::json!({ "error": { "code": "sender_constrained_unsupported" } }),
            ),
        )
            .into_response();
    };
    // r[impl session.graphql-json]
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    // The route admits only tenant tokens, and every tenant token names its
    // tenant.
    let Some(tenant) = token.tid else {
        tracing::error!("a tenant token without a tenant");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let principal = Principal {
        subject: token.sub,
        tenant,
    };
    let authorizer = Authorizer::new(state.decider.clone(), bearer);
    GraphQLResponse::from(
        state
            .schema
            .execute(request.into_inner().data(principal).data(authorizer))
            .await,
    )
    .into_response()
}
