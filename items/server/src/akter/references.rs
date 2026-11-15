//! The links an akt's `markdown` fields keep: one, unpinned, to each akt
//! their references name that exists and fits the kind
//! (`akt.field.markdown.references`, `link.referenced`).

use chrono::{DateTime, Utc};
use kuria_config::AktType;
use kuria_engine::Value;
use kuria_engine::markdown;
use kuria_engine::rules::records;
use kuria_store::akter::{self as store, AktRow};
use kuria_store::links::{self as stored, LinkRow};
use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{AktError, type_of};

/// Makes `akt`'s links of each kind its `markdown` fields keep their
/// references as those `fields` reference, and answers what changed, as a
/// revision records it (`{"linked": […], "unlinked": […]}`), or `None` if
/// nothing did. The targets get no revision: they may be final.
// r[impl akt.field.markdown.references]
// r[impl link.referenced]
pub(crate) async fn keep(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    ty: &AktType,
    akt: &AktRow,
    fields: &std::collections::BTreeMap<String, Value>,
    by: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<Json>, AktError> {
    let kept = markdown::referenced(ty, fields);
    if kept.is_empty() {
        return Ok(None);
    }
    let mut linked = Vec::new();
    let mut unlinked = Vec::new();
    let existing = stored::from(conn, tenant, akt.id).await?;
    for (kind, numbers) in kept {
        let Some(def) = ty.link(&kind) else { continue };
        let mut wanted: Vec<AktRow> = Vec::new();
        for number in &numbers {
            if *number == akt.number {
                continue;
            }
            let Some(target) = store::akt_by_number(conn, tenant, number).await? else {
                continue;
            };
            let target_type = type_of(conn, tenant, &target).await?;
            if def.to.iter().any(|k| k == target_type.ty.key()) {
                wanted.push(target);
            }
        }
        for (link, target) in existing.iter().filter(|(l, _)| l.kind == kind) {
            if !wanted.iter().any(|w| w.id == link.target_id) {
                stored::delete(conn, tenant, akt.id, &kind, link.target_id).await?;
                unlinked.push(records::link_entry(
                    &target.id.to_string(),
                    &target.number,
                    def,
                    &def.name,
                    None,
                ));
            }
        }
        for target in wanted {
            let had = existing
                .iter()
                .any(|(l, _)| l.kind == kind && l.target_id == target.id);
            if !had {
                stored::insert(
                    conn,
                    &LinkRow {
                        tenant_id: tenant,
                        source_id: akt.id,
                        kind: kind.clone(),
                        target_id: target.id,
                        pinned_revision: None,
                        added_at: now,
                        added_by: by,
                    },
                )
                .await?;
                linked.push(records::link_entry(
                    &target.id.to_string(),
                    &target.number,
                    def,
                    &def.name,
                    None,
                ));
            }
        }
    }
    if linked.is_empty() && unlinked.is_empty() {
        return Ok(None);
    }
    let mut change = serde_json::Map::new();
    if !linked.is_empty() {
        change.insert("linked".to_owned(), Json::Array(linked));
    }
    if !unlinked.is_empty() {
        change.insert("unlinked".to_owned(), Json::Array(unlinked));
    }
    Ok(Some(json!(change)))
}

/// Keeps `akt`'s reference links as `fields` say them, recording what
/// changed in `revision`, beside any membership it records.
pub(crate) async fn record(
    conn: &mut kuria_store::Connection,
    ty: &AktType,
    akt: &AktRow,
    fields: &std::collections::BTreeMap<String, Value>,
    revision: &mut kuria_store::akter::RevisionRow,
) -> Result<(), AktError> {
    let links = keep(
        conn,
        akt.tenant_id,
        ty,
        akt,
        fields,
        revision.author,
        revision.created_at,
    )
    .await?;
    revision.membership = merged(revision.membership.take(), links);
    Ok(())
}

/// `membership` with `links` merged in, as one revision records both.
fn merged(membership: Option<Json>, links: Option<Json>) -> Option<Json> {
    match (membership, links) {
        (Some(Json::Object(mut a)), Some(Json::Object(b))) => {
            a.extend(b);
            Some(Json::Object(a))
        }
        (a, None) => a,
        (None, b) => b,
        (a, _) => a,
    }
}
