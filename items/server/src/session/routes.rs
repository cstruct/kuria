//! `/auth/login`, `/auth/callback` and `/auth/logout`.

use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use base64::Engine;
use chrono::{DateTime, TimeDelta, Utc};
use konklav_axum::{RoutePolicy, RouteRateLimit};
use konklav_relying_client::{Presented, Verified};
use konklav_token::TokenKind;
use kuria_store::sessions::{self, BrowserSession, NewLoginAttempt};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::cookies::LOGIN_MAX_AGE_SECONDS;
use super::resolve::expires_at;
use super::{SessionState, id_hash, random_token};
use crate::oidc::Tokens;

/// The sign-in routes. Each is public to `konklav-axum`: they authenticate
/// browsers rather than tokens.
pub(crate) fn router(state: SessionState) -> Result<Router, crate::AppError> {
    // r[impl session.login.rate-limit]
    let limited = RoutePolicy::public().rate_limit(RouteRateLimit::new(
        state.sessions.sign_in_limit.clone(),
        "{client:address}",
    )?)?;
    Ok(Router::new()
        .route("/auth/login", get(login).route_layer(limited))
        .route(
            "/auth/callback",
            get(callback).route_layer(RoutePolicy::public()),
        )
        .route(
            "/auth/logout",
            post(logout).route_layer(RoutePolicy::public()),
        )
        .with_state(state))
}

#[derive(Deserialize)]
struct LoginParams {
    tenant: Option<Uuid>,
    return_to: Option<String>,
}

/// Starts a sign-in at Konklav.
// r[impl session.login.start]
async fn login(State(state): State<SessionState>, Query(params): Query<LoginParams>) -> Response {
    let sessions = &state.sessions;
    let state_value = random_token(32);
    let code_verifier = random_token(32);
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(code_verifier.as_bytes()));
    let scope = match params.tenant {
        // r[impl person.from-sign-in]
        Some(tenant) => format!("openid profile email tenant:{tenant}"),
        None => "openid profile email".to_owned(),
    };
    let attempt = NewLoginAttempt {
        state: state_value.clone(),
        code_verifier,
        return_to: return_to(params.return_to.as_deref()),
        expires_at: Utc::now() + TimeDelta::seconds(LOGIN_MAX_AGE_SECONDS),
    };
    let stored = async {
        let mut conn = sessions.pool.get().await.map_err(|e| e.to_string())?;
        sessions::insert_login_attempt(&mut conn, &attempt)
            .await
            .map_err(|e| e.to_string())
    };
    if let Err(e) = stored.await {
        tracing::error!("recording a login attempt: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let location = sessions
        .provider
        .authorization_url(&state_value, &code_challenge, &scope);
    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, location),
            (
                header::SET_COOKIE,
                sessions
                    .cookies
                    .set_login(&state_value)
                    .to_str()
                    .unwrap_or_default()
                    .to_owned(),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
    )
        .into_response()
}

/// Where to go after signing in: a path on Kuria's own origin, or `/`.
// r[impl session.login.return-to]
fn return_to(requested: Option<&str>) -> String {
    match requested {
        Some(path)
            if path.starts_with('/')
                && !path.starts_with("//")
                && !path.contains('\\')
                && !path.chars().any(char::is_control) =>
        {
            path.to_owned()
        }
        _ => "/".to_owned(),
    }
}

#[derive(Deserialize)]
struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    iss: Option<String>,
    error: Option<String>,
}

/// Completes a sign-in: checks it belongs to this browser, exchanges the
/// code, verifies the token and starts a session.
// r[impl session.login.callback]
async fn callback(
    State(state): State<SessionState>,
    headers: HeaderMap,
    Query(params): Query<CallbackParams>,
) -> Response {
    let sessions = &state.sessions;
    let failed = |reason: &str| {
        tracing::info!(reason, "a sign-in failed");
        (
            StatusCode::BAD_REQUEST,
            [(
                header::SET_COOKIE,
                sessions
                    .cookies
                    .clear_login()
                    .to_str()
                    .unwrap_or_default()
                    .to_owned(),
            )],
            "The sign-in failed. Start it again.",
        )
            .into_response()
    };
    if let Some(error) = &params.error {
        return failed(&format!("Konklav answered {error}"));
    }
    let (Some(code), Some(state_value)) = (&params.code, &params.state) else {
        return failed("no code or state");
    };
    if sessions.cookies.login(&headers).as_ref() != Some(state_value) {
        return failed("the state is not this browser's");
    }
    if params
        .iss
        .as_deref()
        .is_some_and(|iss| iss != sessions.provider.issuer())
    {
        return failed("the response names another issuer");
    }

    let mut conn = match sessions.pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("checking out a database connection: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let attempt = match sessions::take_login_attempt(&mut conn, state_value, Utc::now()).await {
        Ok(Some(attempt)) => attempt,
        Ok(None) => return failed("no such login attempt, or it expired"),
        Err(e) => {
            tracing::error!("reading a login attempt: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let signed_in = match exchange(&state, code, &attempt.code_verifier).await {
        Ok(signed_in) => signed_in,
        Err(reason) => return failed(&reason),
    };

    record_person(&state, &mut conn, &signed_in).await;

    // r[impl session.login.replace]
    if let Some(previous) = sessions.cookies.session(&headers)
        && let Err(e) = sessions::delete_session(&mut conn, &id_hash(&previous)).await
    {
        tracing::error!("ending a browser's previous session: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let cookie = random_token(32);
    let hash = id_hash(&cookie);
    let session = BrowserSession {
        subject_id: signed_in.subject,
        tenant_id: signed_in.tenant,
        access_token: sessions.cipher.seal(&hash, &signed_in.tokens.access_token),
        refresh_token: sessions.cipher.seal(&hash, &signed_in.refresh_token),
        access_expires_at: expires_at(&signed_in.tokens, signed_in.received_at),
        id_hash: hash,
    };
    if let Err(e) = sessions::insert_session(&mut conn, &session).await {
        tracing::error!("storing a browser session: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let mut response = (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, attempt.return_to),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
    )
        .into_response();
    let set = response.headers_mut();
    set.append(header::SET_COOKIE, sessions.cookies.set_session(&cookie));
    set.append(header::SET_COOKIE, sessions.cookies.clear_login());
    response
}

/// What a completed code exchange gives a new session.
struct SignedIn {
    tokens: Tokens,
    refresh_token: String,
    subject: Uuid,
    tenant: Uuid,
    received_at: DateTime<Utc>,
}

/// Exchanges the code, and verifies the access token through the relying
/// SDK like any other, requiring a tenant token.
async fn exchange(state: &SessionState, code: &str, verifier: &str) -> Result<SignedIn, String> {
    let tokens = state
        .sessions
        .provider
        .exchange(code, verifier)
        .await
        .map_err(|e| e.to_string())?;
    let received_at = Utc::now();
    let refresh_token = tokens
        .refresh_token
        .clone()
        .ok_or("Konklav issued no refresh token")?;
    let presented = Presented {
        authorization: format!("Bearer {}", tokens.access_token),
        dpop: None,
        request: None,
    };
    let claims = match state.host.verify_request(&presented).await {
        Ok(Verified::Valid(claims)) => claims,
        Ok(_) => return Err("the access token did not verify".to_owned()),
        Err(e) => return Err(format!("the access token could not be verified: {e}")),
    };
    let tenant = claims
        .tid
        .filter(|_| claims.knd == TokenKind::Tenant)
        .ok_or("the access token is not a tenant token")?;
    Ok(SignedIn {
        tokens,
        refresh_token,
        subject: claims.sub.into(),
        tenant: tenant.into(),
        received_at,
    })
}

/// Records the signed-in subject's name and email from Konklav's userinfo
/// endpoint, and links the tenant's contact with that email if Konklav
/// verified it. Best effort: a sign-in doesn't fail for want of a name.
// r[impl person.from-sign-in]
async fn record_person(
    state: &SessionState,
    conn: &mut kuria_store::Connection,
    signed_in: &SignedIn,
) {
    let info = match state
        .sessions
        .provider
        .userinfo(&signed_in.tokens.access_token)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            tracing::warn!("reading the signed-in subject's name: {e}");
            return;
        }
    };
    if info.sub != signed_in.subject.to_string() {
        tracing::warn!("the userinfo endpoint answered for another subject");
        return;
    }
    let email = info.email.filter(|e| !e.trim().is_empty());
    let person = kuria_store::people::Person {
        subject_id: signed_in.subject,
        name: info.name.filter(|n| !n.trim().is_empty()),
        email: email.clone(),
        updated_at: Utc::now(),
    };
    if let Err(e) = kuria_store::people::record(conn, &person).await {
        tracing::warn!("recording the signed-in subject's name: {e}");
    }
    // r[impl contact.link]
    if let Some(email) = email.filter(|_| info.email_verified == Some(true)) {
        match kuria_store::contacts::link(
            conn,
            signed_in.tenant,
            email.trim(),
            signed_in.subject,
            Utc::now(),
        )
        .await
        {
            Ok(true) => tracing::info!("linked a contact to the signed-in subject"),
            Ok(false) => {}
            Err(e) => tracing::warn!("linking a contact to the signed-in subject: {e}"),
        }
    }
}

/// Ends the browser's session here and at Konklav.
// r[impl session.logout]
async fn logout(State(state): State<SessionState>, headers: HeaderMap) -> Response {
    let sessions = &state.sessions;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(header::SET_COOKIE, sessions.cookies.clear_session());
    let Some(cookie) = sessions.cookies.session(&headers) else {
        return response;
    };
    let hash = id_hash(&cookie);
    let deleted = async {
        let mut conn = sessions.pool.get().await.map_err(|e| e.to_string())?;
        sessions::delete_session(&mut conn, &hash)
            .await
            .map_err(|e| e.to_string())
    };
    let session = match deleted.await {
        Ok(session) => session,
        Err(e) => {
            tracing::error!("ending a browser session: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    if let Some(refresh_token) =
        session.and_then(|s| sessions.cipher.open(&hash, &s.refresh_token).ok())
        && let Err(e) = sessions.provider.revoke(&refresh_token).await
    {
        // The session is already gone here; Konklav ends its own when the
        // refresh token expires.
        tracing::warn!("revoking a signed-out session's refresh token: {e}");
    }
    response
}

#[cfg(test)]
mod tests {
    use super::return_to;

    // r[verify session.login.return-to]
    #[test]
    fn keeps_only_paths_on_the_same_origin() {
        assert_eq!(
            return_to(Some("/akt/CAPA-42?tab=history")),
            "/akt/CAPA-42?tab=history"
        );
        assert_eq!(return_to(None), "/");
        for unsafe_target in [
            "https://evil.example/",
            "//evil.example/",
            "/\\evil.example",
            "akt/1",
            "/a\nb",
        ] {
            assert_eq!(return_to(Some(unsafe_target)), "/", "{unsafe_target}");
        }
    }
}
