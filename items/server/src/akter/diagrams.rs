//! Diagram blocks, rendered when a `markdown` field that allows them is
//! written (`akt.field.markdown.diagrams`): each SVG an attachment the akt
//! holds, by the field and the hash of the source as rendered.

use chrono::Utc;
use kuria_config::{AktType, FieldKind};
use kuria_engine::{Outcome, Value};
use kuria_store::akter::AktRow;
use kuria_store::attachments::{self, AttachmentRow, DiagramRow};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{AktError, Caller};

/// The hash a diagram block is kept by: of its source and of what is
/// imported for it, so that new bundled packages render it again.
#[must_use]
pub fn source_hash(source: &str) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(kuria_typst::IMPORTS.as_bytes());
    hash.update(b"\n");
    hash.update(source.as_bytes());
    hash.finalize().to_vec()
}

/// `source` rendered to SVG by `renderer`, in a process of its own under
/// its limits (`render.limits`), or why it wasn't: typst's message, or
/// the limit it went past.
pub async fn render(
    renderer: Option<&kuria_typst::Renderer>,
    source: String,
) -> Result<Result<String, String>, AktError> {
    let renderer =
        renderer.ok_or_else(|| AktError::Internal("this Kuria renders no diagrams".to_owned()))?;
    match renderer.render(kuria_typst::Job::Diagram { source }).await {
        Ok(bytes) => Ok(Ok(String::from_utf8_lossy(&bytes).into_owned())),
        Err(kuria_typst::RenderError::Refused(message)) => Ok(Err(message)),
        Err(e @ kuria_typst::RenderError::Limit(_)) => Ok(Err(e.to_string())),
        Err(kuria_typst::RenderError::Failed(e)) => {
            Err(AktError::Internal(format!("rendering a diagram: {e}")))
        }
    }
}

/// Renders each diagram block `outcome` writes in a field of `ty` that
/// allows them, unless `akt` already holds its rendering, and keeps each
/// SVG as an attachment the akt holds; refuses the write, naming the
/// field and the block, at one that doesn't compile.
// r[impl akt.field.markdown.diagrams]
pub(super) async fn render_written(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    ty: &AktType,
    akt: &AktRow,
    outcome: &Outcome,
) -> Result<(), AktError> {
    render_each(conn, caller, ty, akt, outcome, true).await
}

/// Renders each diagram block `outcome` writes that compiles, as
/// [`render_written`] does, passing over those that don't: a live draft's
/// edit, taken as it is typed, isn't refused for a block half written
/// (`live.sync`).
pub(super) async fn render_typed(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    ty: &AktType,
    akt: &AktRow,
    outcome: &Outcome,
) -> Result<(), AktError> {
    render_each(conn, caller, ty, akt, outcome, false).await
}

async fn render_each(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    ty: &AktType,
    akt: &AktRow,
    outcome: &Outcome,
    strict: bool,
) -> Result<(), AktError> {
    let held = attachments::diagrams_of(conn, &[akt.id]).await?;
    let mut rendered = Vec::new();
    for field in &ty.document().fields {
        if field.kind != FieldKind::Markdown || !field.diagrams {
            continue;
        }
        let Some(Some(Value::Markdown(text))) = outcome.changes.get(&field.key) else {
            continue;
        };
        for (i, source) in kuria_engine::markdown::diagrams(text)
            .into_iter()
            .enumerate()
        {
            let hash = source_hash(&source);
            let kept = |r: &DiagramRow| r.field == field.key && r.source_sha256 == hash;
            if held.iter().any(kept) || rendered.iter().any(kept) {
                continue;
            }
            let svg = match render(caller.renderer, source).await? {
                Ok(svg) => svg,
                Err(_) if !strict => continue,
                Err(message) => {
                    return Err(AktError::InvalidDiagram {
                        field: field.key.clone(),
                        block: i + 1,
                        message,
                    });
                }
            };
            let storage = caller
                .storage
                .ok_or_else(|| AktError::Internal("this Kuria keeps no diagrams".to_owned()))?;
            let bytes = svg.into_bytes();
            let sha256 = Sha256::digest(&bytes).to_vec();
            storage
                .put(akt.tenant_id, &sha256, bytes.clone())
                .await
                .map_err(|e| AktError::Internal(format!("storing a diagram: {e}")))?;
            let row = AttachmentRow {
                id: Uuid::now_v7(),
                tenant_id: akt.tenant_id,
                name: format!("{}-{}-diagram-{}.svg", akt.number, field.key, i + 1),
                media_type: "image/svg+xml".to_owned(),
                size: i64::try_from(bytes.len()).unwrap_or(i64::MAX),
                sha256,
                uploaded_by: caller.actor.subject.into(),
                uploaded_at: Utc::now(),
                sbom: None,
            };
            attachments::insert(conn, &row).await?;
            rendered.push(DiagramRow {
                tenant_id: akt.tenant_id,
                akt_id: akt.id,
                field: field.key.clone(),
                source_sha256: hash,
                attachment_id: row.id,
            });
        }
    }
    attachments::insert_diagrams(conn, &rendered).await?;
    Ok(())
}
