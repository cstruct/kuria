//! Serving the built app, and the login page's proxy to Konklav.
//!
//! The app is a static build with one fallback page, which the app's own
//! paths (`/akt/…`, `/login`) load. Its inline bootstrapping scripts are
//! allowed by their hashes, so the Content Security Policy allows no other
//! inline script.

use std::fmt::Write as _;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes, to_bytes};
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use base64::Engine;
use konklav_axum::RoutePolicy;
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use tower_http::services::ServeDir;

/// The inline style SvelteKit's route announcer, a hidden live region,
/// carries; allowed by its hash and nothing else inline. The app's
/// stylesheet hides the announcer too, should SvelteKit change it.
const ANNOUNCER_STYLE: &str = "position: absolute; left: 0; top: 0; clip: rect(0 0 0 0); clip-path: inset(50%); overflow: hidden; white-space: nowrap; width: 1px; height: 1px";

/// The page the app's paths load.
const FALLBACK: &str = "index.html";

/// Where hashed assets are, whose names change with their content.
const IMMUTABLE: &str = "/_app/immutable/";

/// The largest request the proxy passes to Konklav.
const PROXY_LIMIT: usize = 1 << 20;

/// The built app.
#[derive(Clone)]
pub struct Web {
    dir: PathBuf,
    fallback: Bytes,
    csp: HeaderValue,
}

#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("reading {0}: {1}")]
    Read(PathBuf, std::io::Error),
    #[error("the app's Content Security Policy is invalid")]
    Csp,
}

impl Web {
    /// Reads the app built into `dir`, which must hold its fallback page.
    pub fn load(dir: &Path) -> Result<Self, WebError> {
        let path = dir.join(FALLBACK);
        let fallback = std::fs::read(&path).map_err(|e| WebError::Read(path, e))?;
        let page = String::from_utf8_lossy(&fallback);
        let hashes = inline_scripts(&page).fold(String::new(), |mut hashes, script| {
            let hash = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(script));
            let _ = write!(hashes, " 'sha256-{hash}'");
            hashes
        });
        // r[impl app.csp]
        // r[impl login.csp]
        let announcer =
            base64::engine::general_purpose::STANDARD.encode(Sha256::digest(ANNOUNCER_STYLE));
        let csp = format!(
            "default-src 'self'; script-src 'self'{hashes}; style-src 'self'; style-src-attr 'unsafe-hashes' 'sha256-{announcer}'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
        );
        Ok(Self {
            dir: dir.to_owned(),
            fallback: Bytes::from(fallback),
            csp: HeaderValue::from_str(&csp).map_err(|_| WebError::Csp)?,
        })
    }

    /// Routes serving the app for every `GET` the server's other routes
    /// don't take.
    pub(crate) fn router(self) -> Router {
        let web = Arc::new(self);
        Router::new()
            .route("/", get(serve).route_layer(RoutePolicy::public()))
            .route("/{*path}", get(serve).route_layer(RoutePolicy::public()))
            .with_state(web)
    }
}

/// The bodies of `page`'s `<script>` elements that have no `src`.
fn inline_scripts(page: &str) -> impl Iterator<Item = &str> {
    page.split("<script").skip(1).filter_map(|rest| {
        let (tag, body) = rest.split_once('>')?;
        if tag.contains("src=") {
            return None;
        }
        body.split_once("</script>").map(|(script, _)| script)
    })
}

/// Serves a file of the app, or its fallback page.
// r[impl app.serve]
async fn serve(State(web): State<Arc<Web>>, request: Request) -> Response {
    let path = request.uri().path().to_owned();
    let found = ServeDir::new(&web.dir)
        .append_index_html_on_directories(false)
        .oneshot(request)
        .await;
    let mut response = match found {
        Ok(response) if response.status().is_success() => {
            let mut response = response.map(Body::new);
            let cache = if path.starts_with(IMMUTABLE) {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
            response
        }
        _ => {
            let mut response = Response::new(Body::from(web.fallback.clone()));
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            );
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
    };
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, web.csp.clone());
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

/// Konklav's subject-facing surface, which the login page reaches on
/// Kuria's own origin.
#[derive(Clone)]
pub struct LoginProxy {
    http: reqwest::Client,
    konklav: String,
}

impl LoginProxy {
    /// A proxy to Konklav's subject-facing surface at `konklav`.
    pub fn new(konklav: &str) -> Result<Self, reqwest::Error> {
        Ok(Self {
            // Redirects go back to the browser, never followed here.
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            konklav: konklav.trim_end_matches('/').to_owned(),
        })
    }

    pub(crate) fn router(self) -> Router {
        Router::new()
            .route("/v1/{*path}", any(proxy))
            .route("/.well-known/{*path}", any(proxy))
            .route_layer(RoutePolicy::public())
            .with_state(self)
    }
}

/// Hop-by-hop headers, and ones the HTTP client recomputes.
const SKIPPED: [HeaderName; 6] = [
    header::HOST,
    header::CONNECTION,
    header::TRANSFER_ENCODING,
    header::CONTENT_LENGTH,
    header::CONTENT_ENCODING,
    header::UPGRADE,
];

/// Passes a request to Konklav as it came, without Kuria's own cookies, and
/// Konklav's answer back, cookies and redirects included.
// r[impl login.proxy]
async fn proxy(
    State(proxy): State<LoginProxy>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    let Ok(body) = to_bytes(body, PROXY_LIMIT).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };
    let path = parts.uri.path_and_query().map_or("/", |p| p.as_str());
    let mut headers = parts.headers;
    for name in &SKIPPED {
        headers.remove(name);
    }
    without_kuria_cookies(&mut headers);
    if let Ok(address) = HeaderValue::from_str(&client.ip().to_string()) {
        headers.insert("x-forwarded-for", address);
    }
    let method = Method::from_bytes(parts.method.as_str().as_bytes()).unwrap_or(Method::GET);
    let upstream = proxy
        .http
        .request(method, format!("{}{path}", proxy.konklav))
        .headers(headers)
        .body(body)
        .send()
        .await;
    let upstream = match upstream {
        Ok(upstream) => upstream,
        Err(e) => {
            tracing::warn!("reaching Konklav for the login page: {e}");
            return (StatusCode::BAD_GATEWAY, "Konklav is unreachable").into_response();
        }
    };
    let status = upstream.status();
    let mut headers = upstream.headers().clone();
    for name in &SKIPPED {
        headers.remove(name);
    }
    let body = upstream.bytes().await.unwrap_or_default();
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    *response.headers_mut() = headers;
    response
}

/// Removes Kuria's own cookies, which are no business of Konklav's.
fn without_kuria_cookies(headers: &mut HeaderMap) {
    let kept: Vec<String> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .map(str::trim)
        .filter(|c| !c.is_empty() && !is_kuria_cookie(c))
        .map(str::to_owned)
        .collect();
    headers.remove(header::COOKIE);
    if !kept.is_empty()
        && let Ok(value) = HeaderValue::from_str(&kept.join("; "))
    {
        headers.insert(header::COOKIE, value);
    }
}

fn is_kuria_cookie(cookie: &str) -> bool {
    let name = cookie.split_once('=').map_or(cookie, |(name, _)| name);
    name.trim_start_matches("__Host-").starts_with("kuria_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_inline_scripts_only() {
        let page = r#"<script src="/a.js"></script><script>start()</script><script type="module">go()</script>"#;
        assert_eq!(
            inline_scripts(page).collect::<Vec<_>>(),
            ["start()", "go()"]
        );
    }

    #[test]
    fn keeps_only_other_cookies() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("__Host-kuria_session=s; konklav_refresh=r; kuria_login=l"),
        );
        without_kuria_cookies(&mut headers);
        assert_eq!(headers[header::COOKIE], "konklav_refresh=r");
        let mut only_ours = HeaderMap::new();
        only_ours.insert(
            header::COOKIE,
            HeaderValue::from_static("__Host-kuria_session=s"),
        );
        without_kuria_cookies(&mut only_ours);
        assert!(only_ours.get(header::COOKIE).is_none());
    }
}
