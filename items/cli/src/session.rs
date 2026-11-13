//! The signed-in session a command uses: the profile's, refreshed before
//! its access token expires, or one started with `KURIA_KEY`.

use std::path::PathBuf;

use uuid::Uuid;

use crate::dpop::{DpopKey, now};
use crate::error::CliError;
use crate::http::Http;
use crate::konklav::{self, Tokens};
use crate::profile::Target;
use crate::secret::{Secrets, Store};

/// How long before it expires an access token is refreshed, in seconds.
const REFRESH_AHEAD: i64 = 60;

/// Where a command runs: what it is for, and where the client keeps what
/// it saves.
pub struct Context {
    pub target: Target,
    pub dir: PathBuf,
    pub store: Store,
    /// `KURIA_KEY`, to sign in as software instead of with the profile.
    pub secret_key: Option<String>,
}

/// How a session was started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignedIn {
    /// By `kuria login`, saved in a profile.
    Profile(String),
    /// With `KURIA_KEY`, for this command only, at this issuer.
    SecretKey { issuer: String },
}

/// A DPoP-bound session with Kuria.
pub struct Session {
    pub http: Http,
    /// Kuria, without a trailing slash.
    pub url: String,
    pub signed_in: SignedIn,
    access_token: String,
}

impl Context {
    /// The Kuria URL the command is for.
    pub fn url(&self) -> Result<&str, CliError> {
        self.target.url.as_deref().ok_or_else(|| {
            CliError::not_signed_in(
                "Name Kuria with --url or KURIA_URL, or sign in with `kuria login --url <Kuria>`",
            )
        })
    }
}

impl Session {
    /// The session the command signs in with.
    pub async fn open(context: &Context) -> Result<Self, CliError> {
        let url = context.url()?.to_owned();
        if let Some(key) = &context.secret_key {
            return Self::with_secret_key(&url, context.target.tenant, key).await;
        }
        let profile = &context.target.profile;
        let secrets = context.store.load(profile)?.ok_or_else(|| {
            CliError::not_signed_in(format!(
                "Not signed in (profile `{profile}`); run `kuria login`, or set KURIA_KEY"
            ))
        })?;
        let http = Http::new(DpopKey::import(&secrets.dpop_key)?);
        let access_token = match (&secrets.access_token, secrets.expires_at) {
            (Some(token), Some(expires_at)) if expires_at - now() > REFRESH_AHEAD => token.clone(),
            _ => refresh(&http, &url, context, &secrets).await?,
        };
        Ok(Self {
            http,
            url,
            signed_in: SignedIn::Profile(profile.clone()),
            access_token,
        })
    }

    /// A session started from `tokens` that `kuria login` was just given.
    #[must_use]
    pub fn started(http: Http, url: String, profile: String, tokens: &Tokens) -> Self {
        Self {
            http,
            url,
            signed_in: SignedIn::Profile(profile),
            access_token: tokens.access_token.clone(),
        }
    }

    // r[impl cli.secret-key]
    async fn with_secret_key(url: &str, tenant: Option<Uuid>, key: &str) -> Result<Self, CliError> {
        // A key of its own, never saved.
        let http = Http::new(DpopKey::generate());
        let discovery = konklav::discover(&http, url).await?;
        let tenant = tenant.or(discovery.tenant).ok_or_else(|| {
            CliError::refused("NO_TENANT", "Name the tenant with --tenant or KURIA_TENANT")
        })?;
        let access_token = konklav::secret_key_login(&http, &discovery.issuer, key, tenant).await?;
        Ok(Self {
            http,
            url: url.to_owned(),
            signed_in: SignedIn::SecretKey {
                issuer: discovery.issuer,
            },
            access_token,
        })
    }

    /// The access token, for requests carrying a proof.
    #[must_use]
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Ends a session started with a secret key; a saved one stays.
    pub async fn close(self) {
        if let SignedIn::SecretKey { issuer } = &self.signed_in {
            // It expires soon anyway: a failure here isn't the command's.
            drop(konklav::sign_out(&self.http, issuer, &self.access_token).await);
        }
    }
}

/// A fresh access token for the saved session, with the refresh token
/// rotated in the store.
// r[impl cli.secret]
async fn refresh(
    http: &Http,
    url: &str,
    context: &Context,
    secrets: &Secrets,
) -> Result<String, CliError> {
    let Some(refresh_token) = &secrets.refresh_token else {
        return Err(CliError::not_signed_in(
            "The session has ended; run `kuria login`",
        ));
    };
    let discovery = konklav::discover(http, url).await?;
    let endpoints = konklav::endpoints(http, &discovery.issuer).await?;
    let refreshed = konklav::grant(
        http,
        &endpoints,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &discovery.client_id),
            ("refresh_token", refresh_token),
        ],
    )
    .await;
    let tokens = match refreshed {
        Ok(tokens) => tokens,
        Err(error) => {
            if error.code == "NOT_SIGNED_IN" {
                // Konklav ended it: nothing saved is any use.
                context.store.forget(&context.target.profile)?;
            }
            return Err(error);
        }
    };
    context.store.save(
        &context.target.profile,
        &Secrets {
            dpop_key: secrets.dpop_key.clone(),
            refresh_token: tokens
                .refresh_token
                .clone()
                .or_else(|| secrets.refresh_token.clone()),
            access_token: Some(tokens.access_token.clone()),
            expires_at: Some(tokens.expires_at),
        },
    )?;
    Ok(tokens.access_token)
}
