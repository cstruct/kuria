//! Browser sessions: signing browsers in through Konklav, and authenticating
//! their requests with a session cookie.
//!
//! - [`cipher`]: encrypting a session's tokens at rest.
//! - [`cookies`]: the session and login cookies.
//! - [`resolve`]: finding a session's access token, refreshing it if needed.
//! - [`layer`]: the middleware that authenticates cookie requests.
//! - [`routes`]: `/auth/login`, `/auth/callback` and `/auth/logout`.

mod cipher;
mod cookies;
mod layer;
mod resolve;
mod routes;

use std::sync::Arc;

use konklav_axum::RelyingHost;
use konklav_relying_client::rate_limit::{
    RateLimit, RateLimitConfig, RateLimitError, RateLimitWindow, StoreSecret,
};
use kuria_store::Pool;

use crate::oidc::Provider;

pub use cipher::{InvalidKey, SessionCipher};
pub use cookies::Cookies;
pub(crate) use layer::authenticate;
pub(crate) use routes::router;

/// What browser sessions need.
pub struct BrowserSessions {
    pub pool: Pool,
    pub provider: Provider,
    pub cipher: SessionCipher,
    pub cookies: Cookies,
    /// The origins a cookie request may come from, such as
    /// `https://kuria.example`.
    pub allowed_origins: Vec<String>,
    /// How often one client address may start a sign-in.
    pub sign_in_limit: RateLimit,
    /// The tenant a sign-in that names none is for.
    pub default_tenant: Option<uuid::Uuid>,
}

/// The windows of [`sign_in_limit`]: ten sign-ins a minute, a hundred an
/// hour, per client address.
pub const SIGN_IN_WINDOWS: [RateLimitWindow; 2] = [
    RateLimitWindow {
        size: 10,
        period_ms: 60_000,
    },
    RateLimitWindow {
        size: 100,
        period_ms: 3_600_000,
    },
];

/// The limit on starting sign-ins, named `name`, shared through Valkey by
/// every instance that uses the same name and `secret`. While Valkey can't
/// be reached, each instance counts in memory.
// r[impl session.login.rate-limit]
pub fn sign_in_limit(
    name: &str,
    windows: Vec<RateLimitWindow>,
    valkey: redis::aio::ConnectionManager,
    secret: &str,
) -> Result<RateLimit, RateLimitError> {
    RateLimit::with_redis(
        RateLimitConfig::new(name, windows),
        valkey,
        StoreSecret::new(secret.as_bytes())?,
        RateLimit::DEFAULT_TIMEOUT,
    )
}

/// What the `/auth` routes and the session layer share.
#[derive(Clone)]
pub(crate) struct SessionState {
    pub sessions: Arc<BrowserSessions>,
    pub host: Arc<dyn RelyingHost>,
}

/// `bytes` random bytes, base64url-encoded without padding.
fn random_token(bytes: usize) -> String {
    use base64::Engine;
    let mut buffer = vec![0; bytes];
    getrandom::fill(&mut buffer).expect("system randomness");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buffer)
}

/// The SHA-256 hash under which a session cookie's session is stored.
fn id_hash(cookie: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(cookie.as_bytes()).to_vec()
}
