//! The CLI's login relay: `/cli/authorized`, the redirect URI Konklav sends
//! `kuria-cli`'s authorization codes to, keeps each code for its request's
//! `state` until the CLI collects it from `/cli/authorized/collect`.
//!
//! The code is useless to the relay: it is bound to the CLI's PKCE verifier
//! and DPoP key, which never leave the CLI. The relay still never exchanges
//! or logs one.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use konklav_axum::{RoutePolicy, RouteRateLimit};
use konklav_relying_client::rate_limit::{RateLimit, RateLimitWindow};
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// How long a code waits to be collected.
pub const HOLD: Duration = Duration::from_secs(120);

/// The windows of the relay's limit, per client address, shared by both of
/// its routes: enough for a CLI polling every second for the two minutes a
/// code is held, and the redirect.
pub const RELAY_WINDOWS: [RateLimitWindow; 2] = [
    RateLimitWindow {
        size: 120,
        period_ms: 60_000,
    },
    RateLimitWindow {
        size: 1_000,
        period_ms: 3_600_000,
    },
];

/// What the relay needs.
pub struct CliRelay {
    /// Where codes wait, so any instance can answer the CLI.
    pub valkey: ConnectionManager,
    /// How often one client address may use the relay.
    pub limit: RateLimit,
    /// How long a code waits: [`HOLD`], unless a test says otherwise.
    pub hold: Duration,
}

/// What Konklav redirected with, as kept and as collected.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Held {
    Code {
        code: String,
    },
    Error {
        error: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_description: Option<String>,
    },
}

#[derive(Deserialize)]
struct Redirect {
    state: Option<String>,
    code: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct Collect {
    state: String,
}

/// The relay's routes. They are public: the code and `state` are what the
/// CLI holds, not a token.
// r[impl cli.relay]
pub(crate) fn router(relay: Arc<CliRelay>) -> Result<Router, crate::AppError> {
    let limited = || -> Result<RoutePolicy, crate::AppError> {
        Ok(RoutePolicy::public().rate_limit(RouteRateLimit::new(
            relay.limit.clone(),
            "{client:address}",
        )?)?)
    };
    Ok(Router::new()
        .route("/cli/authorized", get(authorized).route_layer(limited()?))
        .route(
            "/cli/authorized/collect",
            post(collect).route_layer(limited()?),
        )
        .with_state(relay))
}

/// Valkey's key for `state`: its hash, so the key names no `state` a CLI
/// could collect with.
fn key(state: &str) -> String {
    format!("kuria.cli-relay:{:x}", Sha256::digest(state.as_bytes()))
}

/// A `state` as `kuria login` makes one: 16 to 256 base64url characters.
fn valid_state(state: &str) -> bool {
    (16..=256).contains(&state.len())
        && state
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// What to keep for a redirect, or `None` when it carries neither a code
/// nor an OAuth error, or something no authorization server sends.
fn held(redirect: Redirect) -> Option<Held> {
    match (redirect.code, redirect.error) {
        (Some(code), None) if !code.is_empty() && code.len() <= 2048 => Some(Held::Code { code }),
        // RFC 6749's error codes are ASCII; these are only ever shown escaped.
        (None, Some(error))
            if !error.is_empty()
                && error.len() <= 64
                && error.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') =>
        {
            Some(Held::Error {
                error,
                error_description: redirect
                    .error_description
                    .map(|d| d.chars().filter(|c| !c.is_control()).take(512).collect()),
            })
        }
        _ => None,
    }
}

/// Konklav's redirect: keeps the code, or the error, for its `state`, and
/// tells the person to return to their terminal.
async fn authorized(
    State(relay): State<Arc<CliRelay>>,
    Query(redirect): Query<Redirect>,
) -> Response {
    let Some(state) = redirect.state.clone().filter(|s| valid_state(s)) else {
        return page(StatusCode::BAD_REQUEST, Page::Invalid);
    };
    let Some(held) = held(redirect) else {
        return page(StatusCode::BAD_REQUEST, Page::Invalid);
    };
    let shown = match &held {
        Held::Code { .. } => Page::Authorized,
        Held::Error { error, .. } => Page::Refused(error.clone()),
    };
    let value = serde_json::to_string(&held).expect("serializable");
    let mut valkey = relay.valkey.clone();
    // The first redirect for a `state` is the one kept.
    let kept: Result<Option<String>, _> = redis::cmd("SET")
        .arg(key(&state))
        .arg(value)
        .arg("NX")
        .arg("PX")
        .arg(u64::try_from(relay.hold.as_millis()).unwrap_or(u64::MAX))
        .query_async(&mut valkey)
        .await;
    match kept {
        Ok(Some(_)) => page(StatusCode::OK, shown),
        Ok(None) => page(StatusCode::CONFLICT, Page::Invalid),
        Err(e) => {
            tracing::error!("keeping a CLI sign-in for collection: {e}");
            page(StatusCode::SERVICE_UNAVAILABLE, Page::Unavailable)
        }
    }
}

/// The CLI collecting what was kept for its `state`, once: the code or the
/// error, or `204 No Content` while nothing is.
async fn collect(State(relay): State<Arc<CliRelay>>, Json(asked): Json<Collect>) -> Response {
    let no_store = [(header::CACHE_CONTROL, "no-store")];
    if !valid_state(&asked.state) {
        return (StatusCode::BAD_REQUEST, no_store).into_response();
    }
    let mut valkey = relay.valkey.clone();
    let taken: Result<Option<String>, _> = redis::cmd("GETDEL")
        .arg(key(&asked.state))
        .query_async(&mut valkey)
        .await;
    match taken {
        Ok(Some(value)) => match serde_json::from_str::<Held>(&value) {
            Ok(held) => (StatusCode::OK, no_store, Json(held)).into_response(),
            Err(e) => {
                tracing::error!("a kept CLI sign-in isn't readable: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, no_store).into_response()
            }
        },
        Ok(None) => (StatusCode::NO_CONTENT, no_store).into_response(),
        Err(e) => {
            tracing::error!("collecting a CLI sign-in: {e}");
            (StatusCode::SERVICE_UNAVAILABLE, no_store).into_response()
        }
    }
}

/// What the page the person's browser lands on says.
enum Page {
    Authorized,
    Refused(String),
    Invalid,
    Unavailable,
}

fn page(status: StatusCode, shown: Page) -> Response {
    let (title, text) = match shown {
        Page::Authorized => (
            "The CLI is signed in".to_owned(),
            "You can return to your terminal.".to_owned(),
        ),
        Page::Refused(error) => (
            "The CLI wasn't signed in".to_owned(),
            format!(
                "Konklav answered <code>{}</code>. Return to your terminal to try again.",
                escape(&error)
            ),
        ),
        Page::Invalid => (
            "This sign-in link has been used".to_owned(),
            "Run <code>kuria login</code> again for a new link.".to_owned(),
        ),
        Page::Unavailable => (
            "Kuria couldn't keep the sign-in".to_owned(),
            "Run <code>kuria login</code> again in a moment.".to_owned(),
        ),
    };
    let html = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{title} · Kuria</title><style>{STYLE}</style></head>\
<body><main><h1>{title}</h1><p>{text}</p></main></body></html>\n"
    );
    let mut response = (status, html).into_response();
    let headers = response.headers_mut();
    for (name, value) in [
        (header::CONTENT_TYPE, "text/html; charset=utf-8"),
        (header::CACHE_CONTROL, "no-store"),
        // The address bar holds the code: it goes nowhere else.
        (header::REFERRER_POLICY, "no-referrer"),
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'",
        ),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
    response
}

const STYLE: &str = "body{margin:0;font-family:system-ui,sans-serif;color:#1c1c1e;background:#f7f7f8}\
main{max-width:32rem;margin:15vh auto;padding:0 16px}h1{font-size:1.5rem}\
code{font-size:.95em}\
@media(prefers-color-scheme:dark){body{color:#ececee;background:#161618}}";

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_is_base64url_and_long_enough() {
        assert!(valid_state("abcdefghijklmnop_-09"));
        assert!(!valid_state("short"));
        assert!(!valid_state("abcdefghijklmnop.q"));
        assert!(!valid_state(&"a".repeat(257)));
    }

    #[test]
    fn keeps_a_code_or_an_oauth_error_only() {
        let redirect = |code: Option<&str>, error: Option<&str>| Redirect {
            state: None,
            code: code.map(str::to_owned),
            error: error.map(str::to_owned),
            error_description: Some("Denied\nby you".to_owned()),
        };
        assert!(matches!(
            held(redirect(Some("c"), None)),
            Some(Held::Code { .. })
        ));
        assert!(matches!(
            held(redirect(None, Some("access_denied"))),
            Some(Held::Error { error_description: Some(d), .. }) if d == "Deniedby you"
        ));
        assert!(held(redirect(None, Some("<script>"))).is_none());
        assert!(held(redirect(Some("c"), Some("access_denied"))).is_none());
        assert!(held(redirect(None, None)).is_none());
    }

    #[test]
    fn keys_by_the_state_s_hash() {
        let state = "abcdefghijklmnop";
        assert!(!key(state).contains(state));
        assert_eq!(key(state), key(state));
    }
}
