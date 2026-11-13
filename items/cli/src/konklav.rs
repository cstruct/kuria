//! Where Kuria signs in, and Konklav's endpoints the client uses: the token
//! endpoint's grants, revocation, and software's secret-key login.

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::dpop::now;
use crate::error::{CliError, Kind};
use crate::http::{Body, Http, json as read_json};

/// What Kuria's `/cli/config` answers.
#[derive(Clone, Debug, Deserialize)]
pub struct Discovery {
    pub issuer: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub tenant: Option<Uuid>,
}

/// Konklav's endpoints, from its discovery document.
#[derive(Clone, Debug, Deserialize)]
pub struct Endpoints {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub revocation_endpoint: Option<String>,
}

/// Tokens a grant answered.
#[derive(Clone, Debug)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// In Unix seconds.
    pub expires_at: i64,
}

/// Where the Kuria at `url` signs in.
// r[impl cli.discovery]
pub async fn discover(http: &Http, url: &str) -> Result<Discovery, CliError> {
    let response = http.get_plain(&format!("{url}/cli/config")).await?;
    if response.status() == StatusCode::NOT_FOUND {
        return Err(CliError::refused(
            "NO_CLI_CONFIG",
            format!(
                "{url} doesn't say where its CLI signs in; is it Kuria, and does it run the CLI's relay?"
            ),
        ));
    }
    let value = read_json(response, "Kuria's /cli/config").await?;
    serde_json::from_value(value)
        .map_err(|e| CliError::unavailable(format!("Kuria's /cli/config: {e}")))
}

/// Konklav's endpoints, from `issuer`'s discovery document.
pub async fn endpoints(http: &Http, issuer: &str) -> Result<Endpoints, CliError> {
    let url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    let response = http.get_plain(&url).await?;
    let value = read_json(response, "Konklav's discovery document").await?;
    serde_json::from_value(value)
        .map_err(|e| CliError::unavailable(format!("Konklav's discovery document: {e}")))
}

#[derive(Deserialize)]
struct Granted {
    access_token: String,
    token_type: String,
    expires_in: i64,
    refresh_token: Option<String>,
}

/// A grant at the token endpoint, with a DPoP proof.
pub async fn grant(
    http: &Http,
    endpoints: &Endpoints,
    form: &[(&str, &str)],
) -> Result<Tokens, CliError> {
    let response = http
        .send(
            Method::POST,
            &endpoints.token_endpoint,
            None,
            &Body::Form(form),
        )
        .await?;
    let status = response.status();
    let value = read_json(response, "Konklav's token endpoint").await?;
    if !status.is_success() {
        return Err(oauth_error(status, &value));
    }
    let granted: Granted = serde_json::from_value(value)
        .map_err(|e| CliError::unavailable(format!("Konklav's token endpoint: {e}")))?;
    if !granted.token_type.eq_ignore_ascii_case("dpop") {
        return Err(CliError::refused(
            "NOT_BOUND",
            "Konklav answered a token that isn't bound to the client's DPoP key",
        ));
    }
    Ok(Tokens {
        access_token: granted.access_token,
        refresh_token: granted.refresh_token,
        expires_at: now() + granted.expires_in,
    })
}

/// A refused OAuth request, as the client says it.
fn oauth_error(status: StatusCode, value: &Value) -> CliError {
    let error = value["error"].as_str().unwrap_or("server_error");
    let message = value["error_description"]
        .as_str()
        .unwrap_or(error)
        .to_owned();
    match error {
        // A refresh token the session no longer has.
        "invalid_grant" => CliError::not_signed_in(format!(
            "Konklav refused the session ({message}); run `kuria login` again"
        )),
        _ if status.is_server_error() => {
            CliError::unavailable(format!("Konklav failed: {message}"))
        }
        _ => CliError::new(Kind::Refused, &error.to_ascii_uppercase(), message),
    }
}

/// Revokes `refresh_token`, ending its session (RFC 7009).
pub async fn revoke(
    http: &Http,
    endpoints: &Endpoints,
    client_id: &str,
    refresh_token: &str,
) -> Result<(), CliError> {
    let Some(endpoint) = &endpoints.revocation_endpoint else {
        return Err(CliError::unavailable(
            "Konklav names no revocation endpoint",
        ));
    };
    let form = [
        ("token", refresh_token),
        ("token_type_hint", "refresh_token"),
        ("client_id", client_id),
    ];
    let response = http
        .send(Method::POST, endpoint, None, &Body::Form(&form))
        .await?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let value = read_json(response, "Konklav's revocation endpoint")
        .await
        .unwrap_or(Value::Null);
    Err(oauth_error(status, &value))
}

/// Software's sign-in with a Konklav secret key (`<key id>.<secret>`),
/// moved into `tenant`, all bound to the client's DPoP key. Answers the
/// tenant token.
// r[impl cli.secret-key]
pub async fn secret_key_login(
    http: &Http,
    issuer: &str,
    key: &str,
    tenant: Uuid,
) -> Result<String, CliError> {
    let issuer = issuer.trim_end_matches('/');
    let Some((key_id, secret)) = key.split_once('.') else {
        return Err(CliError::refused(
            "INVALID_KEY",
            "KURIA_KEY isn't a Konklav secret key (`<key id>.<secret>`)",
        ));
    };
    let body = json!({ "key_id": key_id, "secret": secret });
    let signed_in =
        subject_api(http, &format!("{issuer}/v1/login/secret-key"), None, &body).await?;
    let token = signed_in["access_token"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let body = json!({ "tenant": tenant });
    let moved = subject_api(
        http,
        &format!("{issuer}/v1/login/tenant"),
        Some(&token),
        &body,
    )
    .await?;
    moved["access_token"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| CliError::unavailable("Konklav answered no tenant token"))
}

/// Ends a session started with a secret key.
pub async fn sign_out(http: &Http, issuer: &str, token: &str) -> Result<(), CliError> {
    let url = format!("{}/v1/sign-out", issuer.trim_end_matches('/'));
    http.send(Method::POST, &url, Some(token), &Body::Empty)
        .await?;
    Ok(())
}

async fn subject_api(
    http: &Http,
    url: &str,
    token: Option<&str>,
    body: &Value,
) -> Result<Value, CliError> {
    let response = http
        .send(Method::POST, url, token, &Body::Json(body))
        .await?;
    let status = response.status();
    let value = read_json(response, "Konklav").await?;
    if status.is_success() {
        return Ok(value);
    }
    let code = value["error"]["code"]
        .as_str()
        .or_else(|| value["error"].as_str())
        .unwrap_or("refused");
    let message = value["error"]["message"]
        .as_str()
        .unwrap_or(code)
        .to_owned();
    Err(match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => CliError::new(
            Kind::NotPermitted,
            &code.to_ascii_uppercase(),
            format!("Konklav refused the secret key: {message}"),
        ),
        s if s.is_server_error() => CliError::unavailable(format!("Konklav failed: {message}")),
        _ => CliError::refused(&code.to_ascii_uppercase(), message),
    })
}
