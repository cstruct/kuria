//! Kuria as a confidential OIDC client of Konklav's OpenID provider.
//!
//! Only what browser sessions need: the authorization request, the code
//! exchange, refreshing and revocation. Access tokens are verified by the
//! relying SDK, not here, and ID tokens are not used.

use std::time::Duration;

use serde::Deserialize;

/// The OpenID provider's endpoints and Kuria's client registration.
#[derive(Clone, Debug)]
pub struct Provider {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    revocation_endpoint: Option<String>,
    userinfo_endpoint: Option<String>,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    http: reqwest::Client,
}

/// How Kuria is registered with the OpenID provider.
#[derive(Clone, Debug)]
pub struct Client {
    pub id: String,
    pub secret: String,
    /// Kuria's callback, `<public URL>/auth/callback`, registered exactly.
    pub redirect_uri: String,
}

/// Tokens from the token endpoint.
#[derive(Clone, Debug, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Seconds until the access token expires.
    pub expires_in: Option<i64>,
}

/// What the userinfo endpoint says about a subject.
#[derive(Clone, Debug, Deserialize)]
pub struct UserInfo {
    pub sub: String,
    pub name: Option<String>,
    pub email: Option<String>,
    /// Whether the provider verified `email`.
    #[serde(default)]
    pub email_verified: Option<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The provider answered, and refused: the grant is invalid, expired or
    /// revoked, or the client is not allowed it.
    #[error("the OpenID provider refused: {0}")]
    Refused(String),
    /// The provider could not be reached, or failed.
    #[error("the OpenID provider is unavailable: {0}")]
    Unavailable(String),
}

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    revocation_endpoint: Option<String>,
    userinfo_endpoint: Option<String>,
}

#[derive(Deserialize)]
struct TokenErrorBody {
    error: String,
}

const TIMEOUT: Duration = Duration::from_secs(10);

impl Provider {
    /// Reads the provider's endpoints from `<issuer>/.well-known/openid-configuration`.
    pub async fn discover(issuer: &str, client: Client) -> Result<Self, ProviderError> {
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        let url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        let response = http
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        if !response.status().is_success() {
            return Err(ProviderError::Unavailable(format!(
                "{url} answered {}",
                response.status()
            )));
        }
        let discovery: Discovery = response
            .json()
            .await
            .map_err(|e| ProviderError::Unavailable(format!("{url}: {e}")))?;
        if discovery.issuer != issuer {
            return Err(ProviderError::Unavailable(format!(
                "{url} names the issuer {}, not {issuer}",
                discovery.issuer
            )));
        }
        Ok(Self {
            issuer: discovery.issuer,
            authorization_endpoint: discovery.authorization_endpoint,
            token_endpoint: discovery.token_endpoint,
            revocation_endpoint: discovery.revocation_endpoint,
            userinfo_endpoint: discovery.userinfo_endpoint,
            client_id: client.id,
            client_secret: client.secret,
            redirect_uri: client.redirect_uri,
            http,
        })
    }

    #[must_use]
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// The URL that starts an authorization code flow with `state` and the
    /// PKCE `code_challenge` (`S256`), for `scope`.
    #[must_use]
    pub fn authorization_url(&self, state: &str, code_challenge: &str, scope: &str) -> String {
        let query = [
            ("response_type", "code"),
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("scope", scope),
            ("state", state),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
        ];
        let separator = if self.authorization_endpoint.contains('?') {
            '&'
        } else {
            '?'
        };
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(query)
            .finish();
        format!("{}{separator}{encoded}", self.authorization_endpoint)
    }

    /// Exchanges an authorization code for tokens.
    pub async fn exchange(&self, code: &str, code_verifier: &str) -> Result<Tokens, ProviderError> {
        self.token_request(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("code_verifier", code_verifier),
        ])
        .await
    }

    /// Exchanges a refresh token for new tokens.
    pub async fn refresh(&self, refresh_token: &str) -> Result<Tokens, ProviderError> {
        self.token_request(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .await
    }

    /// The claims about the signed-in subject the userinfo endpoint answers
    /// for `access_token`.
    pub async fn userinfo(&self, access_token: &str) -> Result<UserInfo, ProviderError> {
        let Some(endpoint) = &self.userinfo_endpoint else {
            return Err(ProviderError::Unavailable(
                "the provider has no userinfo endpoint".to_owned(),
            ));
        };
        let response = self
            .http
            .get(endpoint)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        if !response.status().is_success() {
            return Err(ProviderError::Refused(format!(
                "the userinfo endpoint answered {}",
                response.status()
            )));
        }
        response
            .json()
            .await
            .map_err(|e| ProviderError::Unavailable(format!("the userinfo response: {e}")))
    }

    /// Revokes a refresh token, and with it the session at Konklav.
    pub async fn revoke(&self, refresh_token: &str) -> Result<(), ProviderError> {
        let Some(endpoint) = &self.revocation_endpoint else {
            return Err(ProviderError::Unavailable(
                "the provider has no revocation endpoint".to_owned(),
            ));
        };
        let response = self
            .http
            .post(endpoint)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&[
                ("token", refresh_token),
                ("token_type_hint", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ProviderError::Unavailable(format!(
                "the revocation endpoint answered {}",
                response.status()
            )))
        }
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<Tokens, ProviderError> {
        let response = self
            .http
            .post(&self.token_endpoint)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(form)
            .send()
            .await
            .map_err(|e| ProviderError::Unavailable(e.to_string()))?;
        let status = response.status();
        if status.is_success() {
            return response
                .json()
                .await
                .map_err(|e| ProviderError::Unavailable(format!("the token response: {e}")));
        }
        if status.is_client_error() {
            let error = response
                .json::<TokenErrorBody>()
                .await
                .map_or_else(|_| status.to_string(), |body| body.error);
            return Err(ProviderError::Refused(error));
        }
        Err(ProviderError::Unavailable(format!(
            "the token endpoint answered {status}"
        )))
    }
}
