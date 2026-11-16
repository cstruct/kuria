//! Bundles (`bundle.*`): exporting selected akt types, with or without
//! their akter, or a whole tenant, and importing them into the caller's
//! tenant with their numbers and history.
//!
//! - `format`: `bundle.json`.
//! - `archive`: the tar archive a bundle travels as.
//! - `export`: one consistent snapshot.
//! - `import`: checked first, then written all at once.
//! - `bootstrap`: a bundle imported into a fresh tenant as the server
//!   starts.

pub mod archive;
pub mod bootstrap;
pub mod export;
pub mod format;
pub mod import;

use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use konklav_axum::{RequestDecider, TenantToken};
use kuria_store::Pool;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::akter::AktError;
use crate::attachments::Storage;
use crate::authorize::{Decider, Permission};
use crate::http::caller;

pub use bootstrap::{Bootstrap, bootstrap};
pub use export::Selection;

/// The largest bundle an import takes, in bytes.
pub const IMPORT_LIMIT: usize = 1 << 30;

/// Why an export or an import was refused.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("not permitted")]
    Forbidden,
    #[error("there is no akt type `{0}`")]
    UnknownType(String),
    #[error("akter of the selected types name akter of these, which the selection leaves out: {}", .0.join(", "))]
    IncompleteSelection(Vec<String>),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
    #[error(
        "tenant {0} already holds akt types, akter or contacts, and a bootstrap bundle goes only into a fresh tenant: unset KURIA_BOOTSTRAP_BUNDLE to start with what it holds"
    )]
    NotFresh(Uuid),
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    Internal(String),
}

impl From<diesel::result::Error> for BundleError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<diesel_async::pooled_connection::deadpool::PoolError> for BundleError {
    fn from(error: diesel_async::pooled_connection::deadpool::PoolError) -> Self {
        Self::Unavailable(format!("the database: {error}"))
    }
}

impl From<AktError> for BundleError {
    fn from(error: AktError) -> Self {
        match error {
            AktError::Forbidden => Self::Forbidden,
            AktError::InvalidType(problems) => Self::Invalid(format!(
                "an akt type isn't valid: {}",
                problems
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            )),
            AktError::UnreadableType(reason) => {
                Self::Invalid(format!("an akt type can't be read: {reason}"))
            }
            AktError::Unavailable(e) => Self::Unavailable(e.to_string()),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl BundleError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            Self::UnknownType(_) => (StatusCode::NOT_FOUND, "UNKNOWN_AKT_TYPE"),
            Self::IncompleteSelection(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "INCOMPLETE_SELECTION")
            }
            Self::Invalid(_) => (StatusCode::BAD_REQUEST, "INVALID_BUNDLE"),
            Self::Conflict(_) | Self::NotFresh(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Self::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
        }
    }
}

impl IntoResponse for BundleError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        let message = match &self {
            Self::Internal(detail) => {
                tracing::error!("a bundle failed: {detail}");
                "an internal error".to_owned()
            }
            other => other.to_string(),
        };
        let mut body = json!({ "error": { "code": code, "message": message } });
        if let Self::IncompleteSelection(missing) = &self {
            body["error"]["missing"] = json!(missing);
        }
        (status, axum::Json(body)).into_response()
    }
}

/// What the bundle routes need.
#[derive(Clone)]
pub(crate) struct BundleState {
    pub pool: Pool,
    pub storage: Storage,
    pub decider: Arc<dyn Decider>,
}

#[derive(Deserialize)]
pub(crate) struct ExportParams {
    /// Comma-separated akt type keys.
    types: Option<String>,
    /// With the selected types' akter.
    #[serde(default)]
    akter: bool,
    /// The whole tenant.
    #[serde(default)]
    tenant: bool,
}

/// Who asks, if they may `permission`.
async fn permitted(
    state: &BundleState,
    token: &TenantToken,
    headers: &HeaderMap,
    decider: RequestDecider,
    permission: Permission,
) -> Result<(Uuid, Uuid), Box<Response>> {
    let (principal, auth) = caller(token, headers, &state.decider, Some(decider))?;
    // r[impl authz.bundles]
    match auth.allows(&permission).await {
        Ok(true) => Ok((principal.tenant.into(), principal.subject.into())),
        Ok(false) => Err(Box::new(BundleError::Forbidden.into_response())),
        Err(e) => Err(Box::new(
            BundleError::Unavailable(e.to_string()).into_response(),
        )),
    }
}

/// `GET /export?types=a,b[&akter=true]` or `?tenant=true`: the tenant's
/// bundle as a tar archive.
pub(crate) async fn export_route(
    State(state): State<BundleState>,
    token: TenantToken,
    decider: RequestDecider,
    headers: HeaderMap,
    Query(params): Query<ExportParams>,
) -> Response {
    let selection = match (&params.types, params.tenant) {
        (None, true) => Selection::Tenant,
        (Some(types), false) => Selection::Types {
            keys: types
                .split(',')
                .map(str::trim)
                .filter(|k| !k.is_empty())
                .map(str::to_owned)
                .collect(),
            akter: params.akter,
        },
        _ => {
            return BundleError::Invalid(
                "export either `types=<key>,…` or `tenant=true`".to_owned(),
            )
            .into_response();
        }
    };
    if matches!(&selection, Selection::Types { keys, .. } if keys.is_empty()) {
        return BundleError::Invalid("name the akt types to export".to_owned()).into_response();
    }
    let tenant = match permitted(&state, &token, &headers, decider, Permission::Export).await {
        Ok((tenant, _)) => tenant,
        Err(refused) => return *refused,
    };
    match export::export(&state.pool, &state.storage, tenant, &selection).await {
        Ok(archive) => (
            [
                (header::CONTENT_TYPE, "application/x-tar"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            Body::from(archive),
        )
            .into_response(),
        Err(error) => error.into_response(),
    }
}

/// `POST /import` with a bundle's tar archive: writes it into the tenant.
pub(crate) async fn import_route(
    State(state): State<BundleState>,
    token: TenantToken,
    decider: RequestDecider,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (tenant, subject) =
        match permitted(&state, &token, &headers, decider, Permission::Import).await {
            Ok(caller) => caller,
            Err(refused) => return *refused,
        };
    match import::import(&state.pool, &state.storage, tenant, subject, &body).await {
        Ok(imported) => (StatusCode::OK, axum::Json(imported)).into_response(),
        Err(error) => error.into_response(),
    }
}
