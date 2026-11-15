//! `markdown` fields as PDFs (`pdf.*`): of the living version or of a
//! version, rendered with typst and its bundled fonts.

use std::sync::{Arc, OnceLock};

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
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use uuid::Uuid;

use crate::akter::AktError;
use crate::authorize::{Decider, Permission};
use crate::http::caller;

#[derive(Clone)]
pub(crate) struct PdfState {
    pub pool: Pool,
    pub decider: Arc<dyn Decider>,
    /// Kuria's public URL, that references link to.
    pub public_url: String,
}

/// The PDF route's state.
pub(crate) fn state(pool: Pool, decider: Arc<dyn Decider>, public_url: String) -> PdfState {
    PdfState {
        pool,
        decider,
        public_url,
    }
}

#[derive(Deserialize)]
pub(crate) struct Asked {
    field: Option<String>,
    version: Option<i32>,
}

/// typst's fonts, read once.
fn fonts() -> &'static (LazyHash<FontBook>, Vec<Font>) {
    static FONTS: OnceLock<(LazyHash<FontBook>, Vec<Font>)> = OnceLock::new();
    FONTS.get_or_init(|| {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();
        (LazyHash::new(FontBook::from_fonts(&fonts)), fonts)
    })
}

/// One typst document, with nothing to read but itself.
struct Doc {
    library: LazyHash<Library>,
    main: Source,
}

impl World for Doc {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &fonts().0
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            Err(FileError::AccessDenied)
        }
    }

    fn file(&self, _id: FileId) -> FileResult<Bytes> {
        Err(FileError::AccessDenied)
    }

    fn font(&self, index: usize) -> Option<Font> {
        fonts().1.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

/// `markup`, typst, as a PDF.
// r[impl pdf.repeatable]
pub fn render(markup: String) -> Result<Vec<u8>, String> {
    let doc = Doc {
        library: LazyHash::new(Library::default()),
        main: Source::new(FileId::new(None, VirtualPath::new("/main.typ")), markup),
    };
    let said = |errors: &[typst::diag::SourceDiagnostic]| {
        errors
            .iter()
            .map(|e| e.message.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    };
    let document: PagedDocument = typst::compile(&doc).output.map_err(|e| said(&e))?;
    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|e| said(&e))
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
    let (text, pins) = match found.await {
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
    let markup = kuria_engine::typst::markup(&text, &link_of);
    let rendered = tokio::task::spawn_blocking(move || render(markup)).await;
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

/// The field's Markdown and the pins it is read with, if the subject may
/// read it: `None` for an akt, field or version that isn't there.
async fn wanted(
    state: &PdfState,
    authorizer: &crate::authorize::Authorizer,
    tenant: Uuid,
    number: &str,
    asked: &Asked,
) -> Result<Option<(String, Vec<Pin>)>, AktError> {
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
    Ok(Some((text, pins)))
}
