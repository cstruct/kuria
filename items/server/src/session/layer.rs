//! The middleware that authenticates requests by their session cookie.
//!
//! It runs before `konklav-axum`'s layer, and turns a session cookie into
//! the `Authorization: Bearer` header that layer verifies. Requests with an
//! `Authorization` header of their own pass through untouched.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::resolve::Resolved;
use super::{BrowserSessions, SessionState};

/// Authenticates a request by its session cookie.
pub(crate) async fn authenticate(
    State(state): State<SessionState>,
    mut request: Request,
    next: Next,
) -> Response {
    let sessions = &state.sessions;
    // Only the API and the sign-in routes read the cookie: not the app's
    // files, and not the requests the login page sends to Konklav.
    if !is_api(request.uri().path()) {
        return next.run(request).await;
    }
    // r[impl session.request]
    if request.headers().contains_key(header::AUTHORIZATION) {
        return next.run(request).await;
    }
    let Some(cookie) = sessions.cookies.session(request.headers()) else {
        return next.run(request).await;
    };
    // r[impl session.origin]
    if needs_origin(&request) && !sessions.origin_allowed(request.headers()) {
        return error(StatusCode::FORBIDDEN, "origin");
    }
    // The sign-in routes read the cookie themselves.
    if request.uri().path().starts_with("/auth/") {
        return next.run(request).await;
    }
    match sessions.access_token(&cookie).await {
        Ok(Resolved::Token(token)) => {
            let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}")) else {
                return error(StatusCode::INTERNAL_SERVER_ERROR, "internal");
            };
            request.headers_mut().insert(header::AUTHORIZATION, value);
            next.run(request).await
        }
        // r[impl session.unknown]
        // r[impl session.refresh.failure]
        Ok(Resolved::NoSession) => {
            let mut response = next.run(request).await;
            response
                .headers_mut()
                .append(header::SET_COOKIE, sessions.cookies.clear_session());
            response
        }
        Ok(Resolved::Unavailable) => error(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        Err(e) => {
            tracing::error!("resolving a browser session: {e}");
            error(StatusCode::INTERNAL_SERVER_ERROR, "internal")
        }
    }
}

/// Whether `path` is one of the routes a session cookie authenticates.
fn is_api(path: &str) -> bool {
    ["/graphql", "/attachments", "/auth/"]
        .iter()
        .any(|prefix| path == prefix.trim_end_matches('/') || path.starts_with(prefix))
}

/// Whether a cookie request must name an allowed origin: anything that may
/// change something, and every WebSocket upgrade, which browsers don't
/// subject to CORS.
fn needs_origin(request: &Request) -> bool {
    let safe = matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    let upgrade = request
        .headers()
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    !safe || upgrade
}

impl BrowserSessions {
    fn origin_allowed(&self, headers: &HeaderMap) -> bool {
        headers
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|origin| self.allowed_origins.iter().any(|o| o == origin))
    }
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, axum::Json(json!({ "error": { "code": code } }))).into_response()
}
