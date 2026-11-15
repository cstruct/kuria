//! Git repositories Kuria hosts (`git.repo.*`): a container of a type that
//! says `"git": true` is a repository of it and its members, served over
//! git's smart HTTP protocol at `/git/<number>.git` by the `git` binary.
//!
//! - [`cache`]: each repository's local cache, a bare repository filled
//!   from object storage, where its packfiles are kept, and with its refs
//!   written from the database before each request.
//! - [`history`]: the remote helper's history of the container, built in
//!   the server by the helper's own code, answered by the server's GraphQL
//!   schema in-process: what a repository starts from (`git.repo.seed`);
//!   and a push to `main` made into batches by the same code.
//! - [`receive`]: pushes, checked before any of them is kept, their pack
//!   stored and their refs moved by compare-and-swap in the transaction
//!   that writes what they change, merging into live drafts
//!   (`git.repo.push`, `git.repo.main`).
//! - [`commits`]: Kuria's own changes as commits on `main`
//!   (`git.repo.commits`).
//! - [`pkt`]: git's pkt-lines.

mod cache;
mod commits;
mod history;
mod pkt;
mod receive;

pub use commits::Timing;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use base64::Engine as _;
use konklav_axum::{Policy, RequestDecider, RouterPolicy, TenantToken};
use kuria_store::Pool;
use serde::Deserialize;
use uuid::Uuid;

use crate::akter::{AktError, Akter};
use crate::attachments::Storage;
use crate::authorize::{Authorizer, Decider, Permission};
use crate::graphql::{KuriaSchema, Principal};
use crate::http::{AppError, caller};

/// How large a push's request may be.
const PUSH_LIMIT: usize = 256 << 20;

/// What the repositories' routes need.
#[derive(Clone)]
pub(crate) struct GitState {
    pub pool: Pool,
    pub storage: Storage,
    pub decider: Arc<dyn Decider>,
    /// The schema the remote helper's code is answered by, in-process.
    pub schema: KuriaSchema,
    /// The write path a push's batches run on.
    pub akter: Akter,
    pub caches: Arc<cache::Caches>,
    /// Kuria's public URL, as the helper's code names Kuria.
    pub public_url: String,
    /// When Kuria's changes become commits.
    pub timing: Timing,
}

impl GitState {
    pub(crate) fn new(
        pool: Pool,
        storage: Storage,
        decider: Arc<dyn Decider>,
        schema: KuriaSchema,
        akter: Akter,
    ) -> Self {
        Self {
            pool,
            storage,
            decider,
            schema,
            akter,
            caches: Arc::new(cache::Caches::new(std::env::temp_dir().join("kuria-git"))),
            public_url: String::new(),
            timing: Timing::default(),
        }
    }

    /// The same, committing Kuria's changes as `timing` says.
    pub(crate) fn timed(mut self, timing: Timing) -> Self {
        self.timing = timing;
        self
    }

    /// The same, caching repositories under `root`, if given.
    pub(crate) fn caching_in(mut self, root: Option<PathBuf>) -> Self {
        if let Some(root) = root {
            self.caches = Arc::new(cache::Caches::new(root));
        }
        self
    }

    /// The same, for Kuria at `public_url`.
    pub(crate) fn named(mut self, public_url: String) -> Self {
        self.public_url = public_url;
        self
    }
}

/// The repositories' routes, authenticated as `/graphql` is
/// (`git.repo.serve`).
// r[impl git.repo.serve]
pub(crate) fn router(api: &RouterPolicy, state: GitState) -> Result<Router, AppError> {
    Ok(Router::new()
        .route(
            "/git/{repo}/info/refs",
            get(info_refs).route_layer(api.route(&Policy::new())?),
        )
        .route(
            "/git/{repo}/git-upload-pack",
            post(upload_pack)
                .layer(DefaultBodyLimit::max(PUSH_LIMIT))
                .route_layer(api.route(&Policy::new())?),
        )
        .route(
            "/git/{repo}/git-receive-pack",
            post(receive_pack)
                .layer(DefaultBodyLimit::max(PUSH_LIMIT))
                .route_layer(api.route(&Policy::new())?),
        )
        .with_state(state))
}

/// git's credentials as the API takes them: the access token given as the
/// password of HTTP Basic authentication, as `kuria git-credential` gives
/// it, presented as a bearer token for Konklav's layer to verify; and a
/// refusal for a missing one that asks git for credentials.
// r[impl git.repo.serve]
pub(crate) async fn basic_credentials(mut request: Request, next: Next) -> Response {
    let git = request.uri().path().starts_with("/git/");
    if git
        && let Some(token) = basic_password(request.headers())
        && let Ok(bearer) = HeaderValue::from_str(&format!("Bearer {token}"))
    {
        request.headers_mut().insert(header::AUTHORIZATION, bearer);
    }
    let mut response = next.run(request).await;
    if git && response.status() == StatusCode::UNAUTHORIZED {
        response.headers_mut().append(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"Kuria\""),
        );
    }
    response
}

/// The password of a request's HTTP Basic authentication, if it has one.
fn basic_password(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, encoded) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    let (_, password) = std::str::from_utf8(&decoded).ok()?.split_once(':')?;
    (!password.is_empty()).then(|| password.to_owned())
}

/// A repository a request is for, and who asks.
pub(crate) struct Opened {
    pub principal: Principal,
    pub authorizer: Arc<Authorizer>,
    pub tenant: Uuid,
    pub akt: Uuid,
    pub number: String,
    /// Its local cache.
    pub dir: PathBuf,
}

/// What a request does to a repository.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wants {
    Fetch,
    Push,
}

impl Wants {
    fn of(service: &str) -> Option<Self> {
        match service {
            "git-upload-pack" => Some(Self::Fetch),
            "git-receive-pack" => Some(Self::Push),
            _ => None,
        }
    }
}

/// The repository `repo`, `<number>.git`, if it is one and the caller may
/// do what it `wants`: fetching to one who may read its akt, pushing to one
/// who may edit it. One the caller may not read isn't there.
async fn open(
    state: &GitState,
    principal: Principal,
    authorizer: Arc<Authorizer>,
    repo: &str,
    wants: Wants,
) -> Result<Opened, Box<Response>> {
    let not_found = || Box::new((StatusCode::NOT_FOUND, "no such repository").into_response());
    let number = repo.strip_suffix(".git").ok_or_else(not_found)?;
    let tenant: Uuid = principal.tenant.into();
    let found = async {
        let mut conn = state.pool.get().await?;
        let Some(akt) = kuria_store::akter::akt_by_number(&mut conn, tenant, number).await? else {
            return Ok(None);
        };
        let ty = crate::akter::versions::type_of_akt(&mut conn, tenant, &akt).await?;
        let key = ty.ty.key().to_owned();
        if !ty.ty.git()
            || !authorizer
                .allows(&Permission::Read { key: key.clone() })
                .await?
        {
            return Ok(None);
        }
        let may_push =
            wants == Wants::Fetch || authorizer.allows(&Permission::Edit { key }).await?;
        Ok::<_, AktError>(Some((akt, may_push)))
    };
    match found.await {
        Ok(Some((akt, true))) => Ok(Opened {
            principal,
            authorizer,
            tenant,
            akt: akt.id,
            number: akt.number,
            dir: state.caches.dir(tenant, akt.id),
        }),
        Ok(Some((_, false))) => Err(Box::new(
            (StatusCode::FORBIDDEN, "not permitted to push").into_response(),
        )),
        Ok(None) => Err(not_found()),
        Err(AktError::Unavailable(e)) => {
            tracing::warn!("{e}");
            Err(Box::new(
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "authorization is unavailable",
                )
                    .into_response(),
            ))
        }
        Err(e) => Err(Box::new(failed(&e.to_string()))),
    }
}

/// An internal failure, logged.
fn failed(what: &str) -> Response {
    tracing::error!("a repository: {what}");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

/// Looks at every repository every `every`, committing Kuria's changes that
/// are due (`git.repo.commits`).
pub(crate) fn commit_every(state: GitState, every: std::time::Duration) {
    commits::spawn(state, every);
}

/// Who asks, and the repository, ready: its cache filled and its refs
/// written, seeded first if it has none (`git.repo.seed`), and Kuria's
/// changes that are due committed on `main`, so that a fetch has them and
/// a push must build on them (`git.repo.commits`). Answers it with its
/// cache locked, for the request.
async fn ready(
    state: &GitState,
    token: &TenantToken,
    decider: RequestDecider,
    headers: &HeaderMap,
    repo: &str,
    wants: Wants,
) -> Result<(Opened, tokio::sync::OwnedMutexGuard<()>), Box<Response>> {
    let (principal, authorizer) = caller(token, headers, &state.decider, Some(decider))?;
    let opened = open(state, principal, authorizer, repo, wants).await?;
    let guard = state.caches.lock(&opened.dir).await;
    let failed = |e: String| Box::new(failed(&e));
    let refs = cache::fill(state, &opened).await.map_err(failed)?;
    if refs.is_empty() {
        history::seed(state, &opened).await.map_err(failed)?;
        cache::fill(state, &opened).await.map_err(failed)?;
    }
    let kuria = commits::as_kuria(state, opened.tenant, opened.akt, &opened.number);
    // Kuria's changes not committed yet are committed later: the request is
    // answered by the repository as it is.
    if let Err(e) = commits::commit_pending(state, &kuria).await {
        tracing::warn!("committing to {}: {e}", opened.number);
    }
    Ok((opened, guard))
}

#[derive(Deserialize)]
pub(crate) struct Service {
    service: Option<String>,
}

/// `GET /git/<number>.git/info/refs?service=…`: the refs, as the service
/// advertises them.
async fn info_refs(
    State(state): State<GitState>,
    token: TenantToken,
    decider: RequestDecider,
    headers: HeaderMap,
    Path(repo): Path<String>,
    Query(service): Query<Service>,
) -> Response {
    let Some(service) = service.service else {
        return (StatusCode::FORBIDDEN, "git's dumb protocol isn't served").into_response();
    };
    let Some(wants) = Wants::of(&service) else {
        return (StatusCode::FORBIDDEN, "no such service").into_response();
    };
    let (opened, _guard) = match ready(&state, &token, decider, &headers, &repo, wants).await {
        Ok(ready) => ready,
        Err(refused) => return *refused,
    };
    let name = service.trim_start_matches("git-");
    let advertised = cache::git(
        &opened.dir,
        &[name, "--stateless-rpc", "--advertise-refs", "."],
        &[],
        None,
    )
    .await;
    match advertised {
        Ok(refs) => {
            let mut body = pkt::line(format!("# service={service}\n").as_bytes());
            body.extend_from_slice(pkt::FLUSH);
            body.extend_from_slice(&refs);
            answer(&format!("application/x-{service}-advertisement"), body)
        }
        Err(e) => failed(&e),
    }
}

/// `POST /git/<number>.git/git-upload-pack`: a fetch.
async fn upload_pack(
    State(state): State<GitState>,
    token: TenantToken,
    decider: RequestDecider,
    headers: HeaderMap,
    Path(repo): Path<String>,
    body: Bytes,
) -> Response {
    let (opened, _guard) = match ready(&state, &token, decider, &headers, &repo, Wants::Fetch).await
    {
        Ok(ready) => ready,
        Err(refused) => return *refused,
    };
    let body = match inflated(&headers, &body) {
        Ok(body) => body,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };
    match cache::git(
        &opened.dir,
        &["upload-pack", "--stateless-rpc", "."],
        &[],
        Some(body),
    )
    .await
    {
        Ok(out) => answer("application/x-git-upload-pack-result", out),
        Err(e) => failed(&e),
    }
}

/// `POST /git/<number>.git/git-receive-pack`: a push.
async fn receive_pack(
    State(state): State<GitState>,
    token: TenantToken,
    decider: RequestDecider,
    headers: HeaderMap,
    Path(repo): Path<String>,
    body: Bytes,
) -> Response {
    let (opened, _guard) = match ready(&state, &token, decider, &headers, &repo, Wants::Push).await
    {
        Ok(ready) => ready,
        Err(refused) => return *refused,
    };
    let body = match inflated(&headers, &body) {
        Ok(body) => body,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };
    match receive::receive(&state, &opened, &body).await {
        Ok(out) => answer("application/x-git-receive-pack-result", out),
        Err(e) => failed(&e),
    }
}

/// A request's body, gunzipped if git sent it so.
fn inflated(headers: &HeaderMap, body: &Bytes) -> Result<Vec<u8>, String> {
    let gzipped = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("gzip") || v.eq_ignore_ascii_case("x-gzip"));
    if !gzipped {
        return Ok(body.to_vec());
    }
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&body[..]), &mut out)
        .map_err(|e| format!("the request's gzip: {e}"))?;
    Ok(out)
}

/// `body` as `content_type`, never cached.
fn answer(content_type: &str, body: Vec<u8>) -> Response {
    let mut response = Body::from(body).into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(content_type) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify git.repo.serve]
    #[test]
    fn reads_the_token_from_basic_credentials() {
        let mut headers = HeaderMap::new();
        let encoded = base64::engine::general_purpose::STANDARD.encode("kuria:t0k:en");
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {encoded}")).unwrap(),
        );
        assert_eq!(basic_password(&headers).as_deref(), Some("t0k:en"));
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer x"));
        assert_eq!(basic_password(&headers), None);
    }
}
