//! Requests to Kuria and Konklav, each with a fresh DPoP proof.

use std::collections::HashMap;
use std::sync::Mutex;

use reqwest::{Method, Response, StatusCode};
use serde_json::Value;
use url::Url;

use crate::dpop::DpopKey;
use crate::error::CliError;

/// What a request sends.
pub enum Body<'a> {
    Empty,
    Json(&'a Value),
    Form(&'a [(&'a str, &'a str)]),
    /// Bytes of a media type, such as a bundle's tar archive.
    Bytes(&'a [u8], &'a str),
    /// A file to upload as an attachment: its bytes, media type and name
    /// (`attachment.upload`).
    File(&'a [u8], &'a str, &'a str),
}

/// `name` percent-encoded, as `X-Attachment-Name` takes it.
fn encoded(name: &str) -> String {
    name.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// An HTTP client that proves possession of its DPoP key on every request.
pub struct Http {
    client: reqwest::Client,
    pub key: DpopKey,
    /// The last nonce each origin asked for.
    nonces: Mutex<HashMap<String, String>>,
}

impl Http {
    #[must_use]
    pub fn new(key: DpopKey) -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent(concat!("kuria/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("an HTTP client"),
            key,
            nonces: Mutex::new(HashMap::new()),
        }
    }

    /// An unauthenticated request without a proof, such as for discovery.
    pub async fn get_plain(&self, url: &str) -> Result<Response, CliError> {
        Ok(self.client.get(url).send().await?)
    }

    /// `body` to `url` with a DPoP proof, and `access_token` under the
    /// `DPoP` scheme if given. A server asking for a nonce gets one more
    /// try with it.
    // r[impl cli.dpop]
    pub async fn send(
        &self,
        method: Method,
        url: &str,
        access_token: Option<&str>,
        body: &Body<'_>,
    ) -> Result<Response, CliError> {
        let parsed =
            Url::parse(url).map_err(|e| CliError::refused("BAD_URL", format!("{url}: {e}")))?;
        let origin = parsed.origin().ascii_serialization();
        for attempt in 0.. {
            let nonce = self.nonces.lock().unwrap().get(&origin).cloned();
            let mut request = self.client.request(method.clone(), parsed.clone()).header(
                "dpop",
                self.key
                    .proof(method.as_str(), &parsed, access_token, nonce.as_deref()),
            );
            if let Some(token) = access_token {
                request = request.header("authorization", format!("DPoP {token}"));
            }
            request = match body {
                Body::Empty => request,
                Body::Json(value) => request.json(value),
                Body::Form(fields) => request.form(fields),
                Body::Bytes(bytes, media_type) => request
                    .header("content-type", *media_type)
                    .body(bytes.to_vec()),
                Body::File(bytes, media_type, name) => request
                    .header("content-type", *media_type)
                    .header("x-attachment-name", encoded(name))
                    .body(bytes.to_vec()),
            };
            let response = request.send().await?;
            let asked = response
                .headers()
                .get("dpop-nonce")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            if let Some(asked) = &asked {
                self.nonces
                    .lock()
                    .unwrap()
                    .insert(origin.clone(), asked.clone());
            }
            let retry = attempt == 0
                && asked.is_some()
                && asked != nonce
                && matches!(
                    response.status(),
                    StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED
                );
            if !retry {
                return Ok(response);
            }
        }
        unreachable!("the loop returns")
    }
}

/// `response`'s body as JSON, or a failure saying what answered instead.
pub async fn json(response: Response, what: &str) -> Result<Value, CliError> {
    let status = response.status();
    let text = response.text().await?;
    serde_json::from_str(&text).map_err(|_| {
        CliError::unavailable(format!(
            "{what} answered {status} without JSON: {}",
            text.chars().take(200).collect::<String>()
        ))
    })
}
