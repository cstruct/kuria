//! The HTTP routes and their Konklav route policies.

use std::sync::Arc;

use async_graphql::http::ALL_WEBSOCKET_PROTOCOLS;
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use konklav_axum::rate_limit::TemplateError;
use konklav_axum::{
    InvalidProxy, Konklav, Policy, RelyingHost, RequestDecider, RoutePolicy, RoutePolicyError,
    RouterPolicy, TenantToken,
};
use konklav_token::TokenKind;

use crate::akter::Akter;
use crate::attachments::{self, AttachmentState, Storage};
use crate::authorize::{Authorizer, Decider};
use crate::changes::Hub;
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
    /// Change events. Without them, the `changes` subscription refuses, and
    /// published events wait for the relay's next pass.
    pub changes: Option<Changes>,
    /// Where attachments are stored, and how large they may be.
    pub storage: Storage,
    /// The built app, served for every `GET` no other route takes.
    pub web: Option<crate::web::Web>,
    /// The proxy to Konklav's subject-facing surface, when the server is
    /// Konklav's login application.
    pub login_proxy: Option<crate::web::LoginProxy>,
}

/// What delivers change events: the hub subscriptions join, and what wakes
/// the relay after a write.
#[derive(Clone)]
pub struct Changes {
    pub hub: Hub,
    pub waker: Arc<tokio::sync::Notify>,
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
        changes,
        storage,
        web,
        login_proxy,
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

    let attachment_routes = Router::new()
        .route(
            "/attachments",
            // r[impl attachment.limit]
            post(attachments::upload)
                .layer(DefaultBodyLimit::max(storage.limit))
                .route_layer(api.route(&Policy::new())?),
        )
        .route(
            "/attachments/{id}",
            get(attachments::download).route_layer(api.route(&Policy::new())?),
        )
        .with_state(AttachmentState {
            pool: pool.clone(),
            storage,
            decider: decider.clone(),
        });

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
        .route(
            "/graphql/ws",
            get(graphql_ws).route_layer(api.route(&Policy::new())?),
        )
        .with_state(GraphqlState {
            schema: schema(
                Akter::new(pool, changes.as_ref().map(|c| c.waker.clone())),
                changes.map(|c| c.hub),
            ),
            decider,
        })
        .merge(attachment_routes);
    if let Some(proxy) = login_proxy {
        router = router.merge(proxy.router());
    }
    if let Some(web) = web {
        router = router.merge(web.router());
    }

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

/// Who a GraphQL request or connection acts as, and what decides its
/// permissions, from its verified token; or the answer refusing it.
// r[impl auth.tenant]
pub(crate) fn caller(
    token: &TenantToken,
    headers: &HeaderMap,
    decider: &Arc<dyn Decider>,
    request: Option<RequestDecider>,
) -> Result<(Principal, Arc<Authorizer>), Box<Response>> {
    let refused = || {
        Box::new(
            (
                StatusCode::FORBIDDEN,
                axum::Json(
                    serde_json::json!({ "error": { "code": "sender_constrained_unsupported" } }),
                ),
            )
                .into_response(),
        )
    };
    // r[impl authz.sender-constrained]
    // A sender-constrained token is decided with the presentation its route
    // policy accepted, which only lasts the request: a WebSocket's later
    // decisions can't use it.
    let authorizer = if token.cnf.is_some() {
        Authorizer::for_request(request.ok_or_else(refused)?)
    } else {
        // Verified by Konklav's layer; a bearer token, since it has no `cnf`.
        let bearer = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map(|(_, token)| token.trim().to_owned())
            .ok_or_else(refused)?;
        Authorizer::new(decider.clone(), bearer)
    };
    // The route admits only tenant tokens, and every tenant token names its
    // tenant.
    let Some(tenant) = token.tid else {
        tracing::error!("a tenant token without a tenant");
        return Err(Box::new(StatusCode::INTERNAL_SERVER_ERROR.into_response()));
    };
    let principal = Principal {
        subject: token.sub,
        tenant,
        expires_at: token.exp,
    };
    Ok((principal, Arc::new(authorizer)))
}

/// Executes a GraphQL request as the subject and in the tenant of its
/// verified token.
async fn graphql(
    State(state): State<GraphqlState>,
    token: TenantToken,
    decider: RequestDecider,
    headers: HeaderMap,
    request: GraphQLRequest,
) -> Response {
    // r[impl session.graphql-json]
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    let (principal, authorizer) = match caller(&token, &headers, &state.decider, Some(decider)) {
        Ok(caller) => caller,
        Err(refused) => return *refused,
    };
    if !json {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    GraphQLResponse::from(
        state
            .schema
            .execute(request.into_inner().data(principal).data(authorizer))
            .await,
    )
    .into_response()
}

/// Serves GraphQL subscriptions over a WebSocket, as the subject and in the
/// tenant of the upgrade request's verified token.
// r[impl changes.websocket]
async fn graphql_ws(
    State(state): State<GraphqlState>,
    token: TenantToken,
    headers: HeaderMap,
    protocol: GraphQLProtocol,
    upgrade: WebSocketUpgrade,
) -> Response {
    // Subscriptions decide long after the upgrade request is answered.
    let (principal, authorizer) = match caller(&token, &headers, &state.decider, None) {
        Ok(caller) => caller,
        Err(refused) => return *refused,
    };
    let mut data = async_graphql::Data::default();
    data.insert(principal);
    data.insert(authorizer);
    let schema = state.schema.clone();
    upgrade
        .protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |socket| {
            GraphQLWebSocket::new(socket, schema, protocol)
                .with_data(data)
                .serve()
        })
}
