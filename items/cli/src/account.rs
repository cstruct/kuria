//! `login`, `logout` and `whoami`.

use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::dpop::DpopKey;
use crate::error::{CliError, Kind};
use crate::http::{Body, Http, json as read_json};
use crate::konklav;
use crate::output::{Answer, note};
use crate::profile::{Config, Profile};
use crate::secret::Secrets;
use crate::session::{Context, Session, SignedIn};

/// How long `login` waits for the link to be authorized: a little longer
/// than the relay keeps a code.
const WAIT: Duration = Duration::from_secs(130);

/// `kuria login`: signs in by a link authorized in a browser, anywhere.
// r[impl cli.login]
pub async fn login(context: &Context, open_browser: bool) -> Result<Answer, CliError> {
    let url = context
        .url()
        .map_err(|_| CliError::refused("NO_URL", "Name Kuria: `kuria login --url <Kuria>`"))?
        .to_owned();
    let http = Http::new(DpopKey::generate());
    let discovery = konklav::discover(&http, &url).await?;
    let tenant = context.target.tenant.or(discovery.tenant).ok_or_else(|| {
        CliError::refused(
            "NO_TENANT",
            "Name the tenant: `kuria login --tenant <tenant id>`",
        )
    })?;
    let endpoints = konklav::endpoints(&http, &discovery.issuer).await?;

    let verifier = random(32);
    let state = random(32);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let scope = format!("openid email tenant:{tenant}");
    let mut link = url::Url::parse(&endpoints.authorization_endpoint)
        .map_err(|e| CliError::unavailable(format!("Konklav's authorization endpoint: {e}")))?;
    link.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &discovery.client_id)
        .append_pair("redirect_uri", &discovery.redirect_uri)
        .append_pair("scope", &scope)
        .append_pair("state", &state)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("dpop_jkt", &http.key.thumbprint());
    note(&format!(
        "Open this link to sign in to {url}, in a browser on any machine:\n\n  {link}\n\nWaiting for it to be authorized…"
    ));
    if open_browser {
        // Where there's no browser, the link is still printed.
        drop(webbrowser::open(link.as_str()));
    }

    let code = collect(&http, &url, &state).await?;
    let tokens = konklav::grant(
        &http,
        &endpoints,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &discovery.client_id),
            ("code", &code),
            ("redirect_uri", &discovery.redirect_uri),
            ("code_verifier", &verifier),
        ],
    )
    .await?;

    let profile = context.target.profile.clone();
    context.store.save(
        &profile,
        &Secrets {
            dpop_key: http.key.export(),
            refresh_token: tokens.refresh_token.clone(),
            access_token: Some(tokens.access_token.clone()),
            expires_at: Some(tokens.expires_at),
        },
    )?;
    let mut config = Config::load(&context.dir)?;
    config.profiles.insert(
        profile.clone(),
        Profile {
            url: url.clone(),
            tenant: Some(tenant),
        },
    );
    config.default.get_or_insert_with(|| profile.clone());
    config.save(&context.dir)?;

    let session = Session::started(http, url, profile, &tokens);
    whoami_with(&session).await
}

/// The code Konklav redirected to Kuria's relay with, once the person
/// authorizes the link.
async fn collect(http: &Http, url: &str, state: &str) -> Result<String, CliError> {
    let deadline = Instant::now() + WAIT;
    let body = json!({ "state": state });
    let collect_url = format!("{url}/cli/authorized/collect");
    loop {
        let response = http
            .send(Method::POST, &collect_url, None, &Body::Json(&body))
            .await?;
        let wait = match response.status() {
            StatusCode::OK => {
                let held = read_json(response, "Kuria's relay").await?;
                return match held["code"].as_str() {
                    Some(code) => Ok(code.to_owned()),
                    None => Err(CliError::new(
                        Kind::NotPermitted,
                        &held["error"]
                            .as_str()
                            .unwrap_or("refused")
                            .to_ascii_uppercase(),
                        format!(
                            "The sign-in wasn't authorized: {}",
                            held["error_description"]
                                .as_str()
                                .or_else(|| held["error"].as_str())
                                .unwrap_or("refused")
                        ),
                    )),
                };
            }
            StatusCode::NO_CONTENT => Duration::from_secs(1),
            StatusCode::TOO_MANY_REQUESTS => Duration::from_secs(5),
            status => {
                return Err(CliError::unavailable(format!(
                    "Kuria's relay answered {status}"
                )));
            }
        };
        if Instant::now() + wait > deadline {
            return Err(CliError::refused(
                "EXPIRED",
                "The link wasn't authorized in time; run `kuria login` again",
            ));
        }
        tokio::time::sleep(wait).await;
    }
}

/// `kuria logout`: ends the profile's session at Konklav, and forgets its
/// key and tokens.
// r[impl cli.secret]
pub async fn logout(context: &Context) -> Result<Answer, CliError> {
    let profile = &context.target.profile;
    let Some(secrets) = context.store.load(profile)? else {
        return Ok(Answer {
            markdown: format!("Not signed in (profile `{profile}`)."),
            json: json!({ "signedOut": false, "profile": profile }),
        });
    };
    let revoked = match (&secrets.refresh_token, context.target.url.as_deref()) {
        (Some(refresh_token), Some(url)) => {
            let http = Http::new(DpopKey::import(&secrets.dpop_key)?);
            let ended = async {
                let discovery = konklav::discover(&http, url).await?;
                let endpoints = konklav::endpoints(&http, &discovery.issuer).await?;
                konklav::revoke(&http, &endpoints, &discovery.client_id, refresh_token).await
            };
            ended.await
        }
        _ => Ok(()),
    };
    // Forgotten here even if Konklav couldn't be told.
    context.store.forget(profile)?;
    revoked.map_err(|error| CliError {
        message: format!(
            "Forgot the session here, but Konklav couldn't end it: {}",
            error.message
        ),
        ..error
    })?;
    Ok(Answer {
        markdown: format!("Signed out (profile `{profile}`)."),
        json: json!({ "signedOut": true, "profile": profile }),
    })
}

/// `kuria whoami`: who the session acts as, and where.
pub async fn whoami(context: &Context) -> Result<Answer, CliError> {
    let session = Session::open(context).await?;
    let answer = whoami_with(&session).await;
    session.close().await;
    answer
}

async fn whoami_with(session: &Session) -> Result<Answer, CliError> {
    let data = session
        .graphql(
            "{ viewer { subject tenant user { name email } } }",
            Value::Null,
        )
        .await?;
    let viewer = &data["viewer"];
    let (with, profile) = match &session.signed_in {
        SignedIn::Profile(name) => ("link", Some(name.as_str())),
        SignedIn::SecretKey { .. } => ("secret-key", None),
    };
    let name = viewer["user"]["name"].as_str().unwrap_or_default();
    let email = viewer["user"]["email"].as_str();
    let who = match email {
        Some(email) if email != name => format!("{name} <{email}>"),
        _ => name.to_owned(),
    };
    let how = match profile {
        Some(profile) => format!("a link (profile `{profile}`)"),
        None => "a secret key (KURIA_KEY)".to_owned(),
    };
    Ok(Answer {
        markdown: format!(
            "Signed in to {} as {who}, with {how}.\n\n- Tenant: {}\n- Subject: {}\n",
            session.url,
            viewer["tenant"].as_str().unwrap_or_default(),
            viewer["subject"].as_str().unwrap_or_default(),
        ),
        json: json!({
            "url": session.url,
            "tenant": viewer["tenant"],
            "subject": viewer["subject"],
            "name": name,
            "email": email,
            "signedInWith": with,
            "profile": profile,
        }),
    })
}

/// `bytes` random bytes, base64url-encoded.
fn random(bytes: usize) -> String {
    let mut buffer = vec![0; bytes];
    getrandom::fill(&mut buffer).expect("system randomness");
    URL_SAFE_NO_PAD.encode(buffer)
}
