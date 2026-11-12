//! A stand-in for Konklav's OpenID provider: discovery, the token endpoint
//! (authorization codes and refresh tokens, rotated) and revocation, served
//! on an ephemeral port.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde_json::json;
use sha2::{Digest, Sha256};

pub const CLIENT_ID: &str = "kuria";
pub const CLIENT_SECRET: &str = "client-secret";

#[derive(Default)]
pub struct ProviderState {
    /// Authorization codes issued, with their PKCE challenge and the access
    /// token the code is exchanged for.
    codes: HashMap<String, (String, String)>,
    /// The refresh token that is currently valid, and how many times it has
    /// been rotated.
    current_refresh: Option<String>,
    rotations: u32,
    /// Refresh grants received.
    pub refreshes: u32,
    /// Tokens revoked.
    pub revoked: Vec<String>,
    /// Answer every token request with 500.
    pub failing: bool,
    /// Answer userinfo requests with 500.
    pub userinfo_failing: bool,
    /// The email userinfo answers, if not `alice@example.com`, and whether
    /// it says it is verified.
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    /// How long a refresh takes.
    pub refresh_delay: Duration,
}

/// A running fake provider.
#[derive(Clone)]
pub struct FakeProvider {
    pub issuer: String,
    pub state: Arc<Mutex<ProviderState>>,
}

impl FakeProvider {
    pub async fn start() -> Self {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new()
            .route(
                "/.well-known/openid-configuration",
                get({
                    let issuer = issuer.clone();
                    move || async move {
                        Json(json!({
                            "issuer": issuer,
                            "authorization_endpoint": format!("{issuer}/authorize"),
                            "token_endpoint": format!("{issuer}/token"),
                            "revocation_endpoint": format!("{issuer}/revoke"),
                            "userinfo_endpoint": format!("{issuer}/userinfo"),
                        }))
                    }
                }),
            )
            .route("/token", post(token))
            .route("/revoke", post(revoke))
            .route("/userinfo", get(userinfo))
            .with_state(state.clone());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self { issuer, state }
    }

    /// Issues `code` for the PKCE `challenge`, to be exchanged for
    /// `access_token`.
    pub fn issue_code(&self, code: &str, challenge: &str, access_token: &str) {
        self.state.lock().unwrap().codes.insert(
            code.to_owned(),
            (challenge.to_owned(), access_token.to_owned()),
        );
    }

    /// Revokes the current refresh token, as Konklav does when a session
    /// ends.
    pub fn end_session(&self) {
        self.state.lock().unwrap().current_refresh = None;
    }

    pub fn refreshes(&self) -> u32 {
        self.state.lock().unwrap().refreshes
    }

    pub fn revoked(&self) -> Vec<String> {
        self.state.lock().unwrap().revoked.clone()
    }

    pub fn set_failing(&self, failing: bool) {
        self.state.lock().unwrap().failing = failing;
    }

    pub fn set_userinfo_failing(&self, failing: bool) {
        self.state.lock().unwrap().userinfo_failing = failing;
    }

    /// Has userinfo answer `email`, verified or not, or not saying.
    pub fn set_email(&self, email: &str, verified: Option<bool>) {
        let mut state = self.state.lock().unwrap();
        state.email = Some(email.to_owned());
        state.email_verified = verified;
    }

    pub fn set_refresh_delay(&self, delay: Duration) {
        self.state.lock().unwrap().refresh_delay = delay;
    }
}

fn client_authenticated(headers: &HeaderMap) -> bool {
    let expected = format!(
        "Basic {}",
        STANDARD.encode(format!("{CLIENT_ID}:{CLIENT_SECRET}"))
    );
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        == Some(expected.as_str())
}

fn refused(error: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
}

async fn token(
    State(state): State<Arc<Mutex<ProviderState>>>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    if state.lock().unwrap().failing {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if !client_authenticated(&headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "invalid_client" })),
        )
            .into_response();
    }
    match form.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            let mut state = state.lock().unwrap();
            let Some((challenge, access_token)) =
                form.get("code").and_then(|code| state.codes.remove(code))
            else {
                return refused("invalid_grant");
            };
            let verifier = form.get("code_verifier").cloned().unwrap_or_default();
            if URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != challenge {
                return refused("invalid_grant");
            }
            state.rotations = 1;
            state.current_refresh = Some("refresh-1".to_owned());
            Json(json!({
                "access_token": access_token,
                "refresh_token": "refresh-1",
                "token_type": "Bearer",
                "expires_in": 300,
            }))
            .into_response()
        }
        Some("refresh_token") => {
            let delay = {
                let mut state = state.lock().unwrap();
                state.refreshes += 1;
                state.refresh_delay
            };
            tokio::time::sleep(delay).await;
            let mut state = state.lock().unwrap();
            if form.get("refresh_token") != state.current_refresh.as_ref() {
                // Reuse of a rotated token ends the session, as Konklav does.
                state.current_refresh = None;
                return refused("invalid_grant");
            }
            state.rotations += 1;
            let n = state.rotations;
            state.current_refresh = Some(format!("refresh-{n}"));
            Json(json!({
                "access_token": format!("tenant-{n}"),
                "refresh_token": format!("refresh-{n}"),
                "token_type": "Bearer",
                "expires_in": 300,
            }))
            .into_response()
        }
        _ => refused("unsupported_grant_type"),
    }
}

async fn revoke(
    State(state): State<Arc<Mutex<ProviderState>>>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> StatusCode {
    if !client_authenticated(&headers) {
        return StatusCode::UNAUTHORIZED;
    }
    let mut state = state.lock().unwrap();
    if let Some(token) = form.get("token") {
        state.revoked.push(token.clone());
        if state.current_refresh.as_ref() == Some(token) {
            state.current_refresh = None;
        }
    }
    StatusCode::OK
}

/// Answers for any bearer token: the test subject, Alice.
async fn userinfo(State(state): State<Arc<Mutex<ProviderState>>>, headers: HeaderMap) -> Response {
    let (email, verified) = {
        let state = state.lock().unwrap();
        if state.userinfo_failing {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        (
            state
                .email
                .clone()
                .unwrap_or_else(|| "alice@example.com".to_owned()),
            state.email_verified,
        )
    };
    if !headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("Bearer "))
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut info = json!({
        "sub": super::SUBJECT,
        "name": "Alice Example",
        "email": email,
    });
    if let Some(verified) = verified {
        info["email_verified"] = json!(verified);
    }
    Json(info).into_response()
}
