//! Uploading and downloading attachments.
//!
//! The bytes live in object storage under `<tenant>/<SHA-256 in hex>`, and
//! their metadata in Postgres. Which akter hold an attachment is kept by the
//! write path, so reading one is decided from the akt types that hold it.

use std::fmt::Write as _;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use konklav_axum::{RequestDecider, TenantToken};
use kuria_engine::sbom;
use kuria_store::Pool;
use kuria_store::attachments::{self, AttachmentRow};
use object_store::aws::AmazonS3Builder;
use object_store::memory::InMemory;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::StorageConfig;
use crate::akter::AktError;
use crate::authorize::{Authorizer, Decider, Permission};
use crate::http::caller;

/// The header naming an upload, percent-encoded as UTF-8.
pub const NAME_HEADER: &str = "x-attachment-name";

/// The longest name an attachment may have, in characters.
const NAME_LIMIT: usize = 255;

/// Where attachments' bytes are stored, and how large they may be.
#[derive(Clone)]
pub struct Storage {
    store: Arc<dyn ObjectStore>,
    /// The largest upload, in bytes.
    pub limit: usize,
}

impl Storage {
    /// S3-compatible object storage.
    pub fn s3(config: &StorageConfig, limit: usize) -> Result<Self, object_store::Error> {
        let mut builder = AmazonS3Builder::from_env()
            .with_bucket_name(&config.bucket)
            .with_region(&config.region);
        if let Some(endpoint) = &config.endpoint {
            builder = builder
                .with_allow_http(endpoint.starts_with("http://"))
                .with_endpoint(endpoint);
        }
        if let Some((access, secret)) = &config.credentials {
            builder = builder
                .with_access_key_id(access)
                .with_secret_access_key(secret);
        }
        Ok(Self {
            store: Arc::new(builder.build()?),
            limit,
        })
    }

    /// Storage in `store`.
    #[must_use]
    pub fn new(store: Arc<dyn ObjectStore>, limit: usize) -> Self {
        Self { store, limit }
    }

    /// Storage in memory, for tests.
    #[must_use]
    pub fn in_memory(limit: usize) -> Self {
        Self {
            store: Arc::new(InMemory::new()),
            limit,
        }
    }

    fn location(tenant: Uuid, sha256: &[u8]) -> object_store::path::Path {
        object_store::path::Path::from(format!("{tenant}/{}", hex(sha256)))
    }

    /// The bytes `tenant` stored under `sha256`.
    pub(crate) async fn get(
        &self,
        tenant: Uuid,
        sha256: &[u8],
    ) -> Result<Vec<u8>, object_store::Error> {
        let got = self.store.get(&Self::location(tenant, sha256)).await?;
        Ok(got.bytes().await?.to_vec())
    }

    /// Stores `bytes` for `tenant` under their SHA-256, unless it has them.
    pub(crate) async fn put(
        &self,
        tenant: Uuid,
        sha256: &[u8],
        bytes: Vec<u8>,
    ) -> Result<(), object_store::Error> {
        let location = Self::location(tenant, sha256);
        match self.store.head(&location).await {
            Ok(_) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => self
                .store
                .put(&location, PutPayload::from_bytes(bytes.into()))
                .await
                .map(drop),
            Err(e) => Err(e),
        }
    }
}

/// What the attachment routes need.
/// Objects kept by name, beside attachments: repositories' packfiles
/// (`git.repo.storage`), under `git/`.
impl Storage {
    /// The object at `path`, if there is one.
    pub(crate) async fn get_named(&self, path: &str) -> Result<Vec<u8>, object_store::Error> {
        let got = self
            .store
            .get(&object_store::path::Path::from(path))
            .await?;
        Ok(got.bytes().await?.to_vec())
    }

    /// Keeps `bytes` at `path`, unless something is already there: a name
    /// that is its content's hash never changes.
    pub(crate) async fn put_named(
        &self,
        path: &str,
        bytes: Vec<u8>,
    ) -> Result<(), object_store::Error> {
        let location = object_store::path::Path::from(path);
        match self.store.head(&location).await {
            Ok(_) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => self
                .store
                .put(&location, PutPayload::from_bytes(bytes.into()))
                .await
                .map(drop),
            Err(e) => Err(e),
        }
    }

    /// The names of the objects under `prefix`, their last part only.
    pub(crate) async fn list_named(
        &self,
        prefix: &str,
    ) -> Result<Vec<String>, object_store::Error> {
        use futures_util::TryStreamExt as _;
        let prefix = object_store::path::Path::from(prefix);
        let listed: Vec<object_store::ObjectMeta> =
            self.store.list(Some(&prefix)).try_collect().await?;
        Ok(listed
            .into_iter()
            .filter_map(|meta| meta.location.filename().map(str::to_owned))
            .collect())
    }
}

#[derive(Clone)]
pub(crate) struct AttachmentState {
    pub pool: Pool,
    pub storage: Storage,
    pub decider: Arc<dyn Decider>,
}

/// Stores the body as a new attachment of the caller's tenant.
// r[impl attachment.upload]
// r[impl attachment.immutable]
// r[impl attachment.metadata]
pub(crate) async fn upload(
    State(state): State<AttachmentState>,
    token: TenantToken,
    decider: RequestDecider,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (principal, _) = match caller(&token, &headers, &state.decider, Some(decider)) {
        Ok(caller) => caller,
        Err(refused) => return *refused,
    };
    let name = match name(&headers) {
        Ok(name) => name,
        Err(problem) => return error(StatusCode::BAD_REQUEST, "invalid_name", problem),
    };
    let media_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map_or("application/octet-stream", str::trim)
        .to_owned();
    let tenant: Uuid = principal.tenant.into();
    let sha256 = Sha256::digest(&body).to_vec();
    // r[impl attachment.sbom]
    let sbom = sbom::detect(&body).map(|summary| json!(summary));

    let location = Storage::location(tenant, &sha256);
    let stored = match state.storage.store.head(&location).await {
        Ok(_) => Ok(()),
        Err(object_store::Error::NotFound { .. }) => state
            .storage
            .store
            .put(&location, PutPayload::from_bytes(body.clone()))
            .await
            .map(drop),
        Err(e) => Err(e),
    };
    if let Err(e) = stored {
        tracing::error!("storing an attachment: {e}");
        return error(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "storage");
    }

    let row = AttachmentRow {
        id: Uuid::now_v7(),
        tenant_id: tenant,
        name,
        media_type,
        size: i64::try_from(body.len()).unwrap_or(i64::MAX),
        sha256,
        uploaded_by: principal.subject.into(),
        uploaded_at: Utc::now(),
        sbom,
    };
    let inserted = async {
        let mut conn = state.pool.get().await.map_err(|e| e.to_string())?;
        attachments::insert(&mut conn, &row)
            .await
            .map_err(|e| e.to_string())
    };
    if let Err(e) = inserted.await {
        tracing::error!("recording an attachment: {e}");
        return error(StatusCode::INTERNAL_SERVER_ERROR, "internal", "database");
    }
    (StatusCode::CREATED, axum::Json(describe(&row))).into_response()
}

/// Answers an attachment's bytes to a subject who may read it.
// r[impl attachment.download]
pub(crate) async fn download(
    State(state): State<AttachmentState>,
    token: TenantToken,
    decider: RequestDecider,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (principal, authorizer) = match caller(&token, &headers, &state.decider, Some(decider)) {
        Ok(caller) => caller,
        Err(refused) => return *refused,
    };
    let not_found = || error(StatusCode::NOT_FOUND, "not_found", "attachment");
    let Ok(id) = Uuid::parse_str(&id) else {
        return not_found();
    };
    let tenant: Uuid = principal.tenant.into();
    let found = async {
        let mut conn = state.pool.get().await?;
        let Some(row) = attachments::find(&mut conn, tenant, &[id]).await?.pop() else {
            return Ok(None);
        };
        let allowed = may_read(&mut conn, &authorizer, principal.subject.into(), &row).await?;
        Ok::<_, AktError>(allowed.then_some(row))
    };
    let row = match found.await {
        Ok(Some(row)) => row,
        Ok(None) => return not_found(),
        Err(AktError::Unavailable(e)) => {
            tracing::warn!("{e}");
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "authorization",
            );
        }
        Err(e) => {
            tracing::error!("reading an attachment: {e}");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "internal", "database");
        }
    };

    let location = Storage::location(tenant, &row.sha256);
    let stream = match state.storage.store.get(&location).await {
        Ok(result) => result.into_stream(),
        Err(e) => {
            tracing::error!("reading an attachment's bytes: {e}");
            return error(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "storage");
        }
    };
    let mut response = Body::from_stream(stream).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&row.media_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    if let Ok(length) = HeaderValue::from_str(&row.size.to_string()) {
        headers.insert(header::CONTENT_LENGTH, length);
    }
    // Always a download, never rendered on Kuria's origin.
    if let Ok(disposition) = HeaderValue::from_str(&format!(
        "attachment; filename*=UTF-8''{}",
        utf8_percent_encode(&row.name, NON_ALPHANUMERIC)
    )) {
        headers.insert(header::CONTENT_DISPOSITION, disposition);
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("sandbox"),
    );
    response
}

/// Whether `subject` may read the attachment `row`: may read an akt that
/// holds it or held it, or uploaded it and no akt has held it yet.
// r[impl attachment.download]
// r[impl attachment.references]
pub(crate) async fn may_read(
    conn: &mut kuria_store::Connection,
    authorizer: &Authorizer,
    subject: Uuid,
    row: &AttachmentRow,
) -> Result<bool, AktError> {
    let holders = attachments::holder_types(conn, row.tenant_id, row.id).await?;
    if holders.is_empty() {
        return Ok(row.uploaded_by == subject);
    }
    for key in holders {
        if authorizer.allows(&Permission::Read { key }).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The upload's name, from its percent-encoded header.
fn name(headers: &HeaderMap) -> Result<String, &'static str> {
    let value = headers
        .get(NAME_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or("the X-Attachment-Name header is missing")?;
    let name = percent_decode_str(value)
        .decode_utf8()
        .map_err(|_| "the name is not percent-encoded UTF-8")?
        .trim()
        .to_owned();
    if name.is_empty() {
        return Err("the name is empty");
    }
    if name.chars().count() > NAME_LIMIT {
        return Err("the name is longer than 255 characters");
    }
    if name
        .chars()
        .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err("the name has control characters or slashes");
    }
    Ok(name)
}

/// An attachment as the API shows it.
pub(crate) fn describe(row: &AttachmentRow) -> serde_json::Value {
    json!({
        "id": row.id,
        "name": row.name,
        "mediaType": row.media_type,
        "size": row.size,
        "sha256": hex(&row.sha256),
        "sbom": row.sbom,
    })
}

#[must_use]
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

fn error(status: StatusCode, code: &str, detail: &str) -> Response {
    (
        status,
        axum::Json(json!({ "error": { "code": code, "detail": detail } })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(value: &str) -> Result<String, &'static str> {
        let mut headers = HeaderMap::new();
        headers.insert(NAME_HEADER, HeaderValue::from_str(value).unwrap());
        name(&headers)
    }

    #[test]
    fn names_are_percent_decoded_and_checked() {
        assert_eq!(named("sbom.json"), Ok("sbom.json".to_owned()));
        assert_eq!(
            named("r%C3%A4ksm%C3%B6rg%C3%A5s.txt"),
            Ok("räksmörgås.txt".to_owned())
        );
        assert!(named("").is_err());
        assert!(named("a%2Fb").is_err());
        assert!(named("a%0Ab").is_err());
        assert!(named("%FF").is_err());
        assert!(named(&"x".repeat(256)).is_err());
        assert!(name(&HeaderMap::new()).is_err());
    }

    #[test]
    fn hex_is_lowercase() {
        assert_eq!(hex(&[0x00, 0xab, 0x10]), "00ab10");
    }
}
