//! `markdown` fields as PDFs (`pdf.*`): of the living version or of a
//! version, rendered by `kuria-typst`.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use konklav_axum::{RequestDecider, TenantToken};
use kuria_config::FieldKind;
use kuria_engine::markdown::Reference;
use kuria_engine::versions::{Pin, as_pinned};
use kuria_engine::{Value, fields_from_json};
use kuria_store::Pool;
use kuria_store::akter as store;
use serde::Deserialize;
use serde_json::Map;
use uuid::Uuid;

use crate::akter::AktError;
use crate::attachments::Storage;
use crate::authorize::{Decider, Permission};
use crate::http::caller;

#[derive(Clone)]
pub(crate) struct PdfState {
    pub pool: Pool,
    /// Where embedded images' pictures are read from.
    pub storage: Storage,
    pub decider: Arc<dyn Decider>,
    /// Kuria's public URL, that references link to.
    pub public_url: String,
}

/// The PDF route's state.
pub(crate) fn state(
    pool: Pool,
    storage: Storage,
    decider: Arc<dyn Decider>,
    public_url: String,
) -> PdfState {
    PdfState {
        pool,
        storage,
        decider,
        public_url,
    }
}

/// An embedded image's picture, for typst: the reference's destination,
/// the file it is read from, and its bytes.
type Picture = (String, String, Vec<u8>);

#[derive(Deserialize)]
pub(crate) struct Asked {
    field: Option<String>,
    version: Option<i32>,
}

/// `GET /akter/{number}/pdf?field=&version=`: a `markdown` field as a PDF,
/// to a subject who may read the akt; 404 otherwise, as if it didn't exist.
// r[impl pdf.route]
pub(crate) async fn pdf(
    State(state): State<PdfState>,
    token: TenantToken,
    decider: RequestDecider,
    headers: HeaderMap,
    Path(number): Path<String>,
    Query(asked): Query<Asked>,
) -> Response {
    let (principal, authorizer) = match caller(&token, &headers, &state.decider, Some(decider)) {
        Ok(caller) => caller,
        Err(refused) => return *refused,
    };
    let tenant: Uuid = principal.tenant.into();
    let found = wanted(&state, &authorizer, tenant, &number, &asked);
    let (text, pins, pictures) = match found.await {
        Ok(Some(found)) => found,
        Ok(None) => return (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(AktError::Unavailable(e)) => {
            tracing::warn!("{e}");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "authorization is unavailable",
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("reading an akt for a PDF: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    let base = state.public_url.trim_end_matches('/').to_owned();
    let link_of = move |r: &Reference| {
        let (version, _) = as_pinned(r, &pins);
        format!(
            "{base}/akt/{}{}{}",
            r.number,
            version.map(|v| format!("?version={v}")).unwrap_or_default(),
            r.heading
                .as_ref()
                .map(|h| format!("#{h}"))
                .unwrap_or_default()
        )
    };
    let files: HashMap<String, String> = pictures
        .iter()
        .map(|(destination, path, _)| (destination.clone(), path.clone()))
        .collect();
    let picture_of = move |r: &Reference| files.get(&r.to_string()).cloned();
    let markup = kuria_engine::typst::markup(&text, &link_of, &picture_of);
    let files = pictures
        .into_iter()
        .map(|(_, path, bytes)| (path, bytes))
        .collect();
    let rendered = tokio::task::spawn_blocking(move || kuria_typst::pdf(markup, files)).await;
    let bytes = match rendered {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => {
            tracing::error!("rendering {number}'s PDF: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "the PDF couldn't be made",
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("rendering {number}'s PDF: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    let name = match asked.version {
        Some(v) => format!("{number}-v{v}.pdf"),
        None => format!("{number}.pdf"),
    };
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    if let Ok(disposition) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, disposition);
    }
    response
}

/// The field's Markdown, the pins it is read with, and the pictures of
/// the images it embeds, if the subject may read it: `None` for an akt,
/// field or version that isn't there.
async fn wanted(
    state: &PdfState,
    authorizer: &crate::authorize::Authorizer,
    tenant: Uuid,
    number: &str,
    asked: &Asked,
) -> Result<Option<(String, Vec<Pin>, Vec<Picture>)>, AktError> {
    let mut conn = state.pool.get().await?;
    let Some(akt) = store::akt_by_number(&mut conn, tenant, number).await? else {
        return Ok(None);
    };
    let ty = crate::akter::versions::type_of_akt(&mut conn, tenant, &akt).await?;
    if !authorizer
        .allows(&Permission::Read {
            key: ty.ty.key().to_owned(),
        })
        .await?
    {
        return Ok(None);
    }
    let field = match &asked.field {
        Some(key) => ty.ty.field(key).cloned(),
        None => {
            let mut markdown = ty
                .ty
                .document()
                .fields
                .iter()
                .filter(|f| f.kind == FieldKind::Markdown);
            match (markdown.next(), markdown.next()) {
                (Some(only), None) => Some(only.clone()),
                _ => None,
            }
        }
    };
    let Some(field) = field.filter(|f| f.kind == FieldKind::Markdown) else {
        return Ok(None);
    };
    let (row, pins) = match asked.version {
        Some(n) => match crate::akter::versions::at_version(&mut conn, &ty, &akt, n).await? {
            Some((row, version)) => {
                let pins: Vec<Pin> = serde_json::from_value(version.pins).unwrap_or_default();
                (row, pins)
            }
            None => return Ok(None),
        },
        None => (akt, Vec::new()),
    };
    let values = fields_from_json(&ty.ty, row.fields.as_object().unwrap_or(&Map::new()))
        .map_err(|e| AktError::Internal(format!("akt {number}: {e}")))?;
    let text = match values.get(&field.key) {
        Some(Value::Markdown(text)) => text.clone(),
        _ => String::new(),
    };
    let pictures = pictures_of(state, &mut conn, authorizer, tenant, &text, &pins).await?;
    Ok(Some((text, pins, pictures)))
}

/// The pictures of the images `text` embeds, as read with `pins`: each one
/// that isn't broken, with its bytes (`pdf.render`).
async fn pictures_of(
    state: &PdfState,
    conn: &mut kuria_store::Connection,
    authorizer: &crate::authorize::Authorizer,
    tenant: Uuid,
    text: &str,
    pins: &[Pin],
) -> Result<Vec<Picture>, AktError> {
    let mut pictures = Vec::new();
    for reference in kuria_engine::markdown::images(text) {
        let target =
            crate::akter::versions::target(conn, tenant, authorizer, &reference.number).await?;
        let (pinned_version, pinned_revision) = as_pinned(&reference, pins);
        let version = reference.version.or(pinned_version);
        let as_read = Reference {
            version,
            ..reference.clone()
        };
        if target.is_none()
            || (pinned_revision.is_none()
                && kuria_engine::markdown::broken_image(&as_read, target.as_ref()).is_some())
        {
            continue;
        }
        let Some(id) = crate::akter::versions::picture_at(
            conn,
            tenant,
            &reference.number,
            version,
            pinned_revision,
        )
        .await?
        else {
            continue;
        };
        let Some(row) = kuria_store::attachments::find(conn, tenant, &[id])
            .await?
            .pop()
        else {
            continue;
        };
        let extension = match row.media_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/svg+xml" => "svg",
            "image/webp" => "webp",
            _ => continue,
        };
        let bytes = state
            .storage
            .get(tenant, &row.sha256)
            .await
            .map_err(|e| AktError::Internal(format!("reading attachment {id}: {e}")))?;
        pictures.push((
            reference.to_string(),
            format!("/pictures/{id}.{extension}"),
            bytes,
        ));
    }
    Ok(pictures)
}
