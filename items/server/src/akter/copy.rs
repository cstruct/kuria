//! Copying an akt with the part of its member tree chosen (`akt.copy`):
//! each copy a new akt of the same type in its initial state, made through
//! the write path as a create, all in one transaction.

use std::collections::{HashMap, HashSet};

use diesel_async::AsyncConnection;
use kuria_config::FieldKind;
use kuria_store::akter::{self as store, AktRow};
use kuria_store::links::LinkRow;
use kuria_store::{containers, links};
use serde_json::{Map, Value as Json};
use uuid::Uuid;

use super::{AktError, Akter, Caller, StoredType, require_read, type_of};
use crate::authorize::Permission;

/// An akt of the tree being copied.
struct Node {
    row: AktRow,
    ty: StoredType,
    parent: Option<Uuid>,
}

impl Akter {
    /// Copies the akt `number`, keeping only the akter `only` names, and
    /// their trees, if given, into the containers `inside`, else the
    /// copied akt's own, and answers the copy.
    // r[impl akt.copy]
    pub async fn copy(
        &self,
        caller: Caller<'_>,
        number: &str,
        only: Option<&[String]>,
        inside: Option<&[String]>,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        let mut conn = self.pool.get().await?;
        let conn: &mut kuria_store::Connection = &mut conn;
        let result = conn
            .transaction::<_, AktError, _>(async |conn| {
                copy_in(conn, caller, number, only, inside, reason.clone()).await
            })
            .await;
        self.committed(result)
    }
}

/// The akt `number` and every member under it the caller may read,
/// parents before their members, in rank order.
async fn tree(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    tenant: Uuid,
    number: &str,
) -> Result<Vec<Node>, AktError> {
    let root = store::akt_by_number(conn, tenant, number)
        .await?
        .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
    let ty = type_of(conn, tenant, &root).await?;
    require_read(caller.auth, &ty, number).await?;
    let mut nodes = vec![Node {
        row: root,
        ty,
        parent: None,
    }];
    // A member in two containers of the tree is copied once, under the
    // first.
    let mut seen = HashSet::from([nodes[0].row.id]);
    let mut at = 0;
    while at < nodes.len() {
        let id = nodes[at].row.id;
        for (member, key) in containers::members_with_types(conn, tenant, id).await? {
            if !seen.insert(member.id) {
                continue;
            }
            // What the caller can't read isn't there for them to copy.
            if !caller.auth.allows(&Permission::Read { key }).await? {
                continue;
            }
            let ty = type_of(conn, tenant, &member).await?;
            nodes.push(Node {
                row: member,
                ty,
                parent: Some(id),
            });
        }
        at += 1;
    }
    Ok(nodes)
}

/// The akter of `nodes` a copy keeps (`kuria_engine::copying::kept`).
// r[impl akt.copy.only]
fn kept(nodes: &[Node], only: Option<&[String]>) -> Result<HashSet<Uuid>, AktError> {
    let numbers: HashMap<Uuid, &str> = nodes
        .iter()
        .map(|n| (n.row.id, n.row.number.as_str()))
        .collect();
    let tree: Vec<(&str, Option<&str>)> = nodes
        .iter()
        .map(|n| (n.row.number.as_str(), n.parent.map(|p| numbers[&p])))
        .collect();
    let kept = kuria_engine::copying::kept(&tree, only).map_err(AktError::BadRequest)?;
    Ok(nodes
        .iter()
        .filter(|n| kept.contains(&n.row.number))
        .map(|n| n.row.id)
        .collect())
}

/// `fields` as a create takes them, without what a copy doesn't keep
/// (`kuria_engine::copying::keeps`).
fn copied_fields(ty: &StoredType, fields: &Json) -> Map<String, Json> {
    fields
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(key, _)| {
            ty.ty
                .field(key)
                .is_some_and(|f| kuria_engine::copying::keeps(f.kind))
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// The body of [`Akter::copy`], in the transaction `conn` is in.
// r[impl akt.copy]
// r[impl akt.copy.links]
// r[impl akt.copy.record]
pub(super) async fn copy_in(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    number: &str,
    only: Option<&[String]>,
    inside: Option<&[String]>,
    reason: Option<String>,
) -> Result<AktRow, AktError> {
    let tenant: Uuid = caller.actor.tenant.into();
    let nodes = tree(conn, caller, tenant, number).await?;
    let kept = kept(&nodes, only)?;
    let nodes: Vec<&Node> = nodes.iter().filter(|n| kept.contains(&n.row.id)).collect();
    let root = nodes[0];
    let inside: Vec<String> = match inside {
        Some(inside) => inside.to_vec(),
        None => containers::containers_of(conn, tenant, root.row.id)
            .await?
            .into_iter()
            .map(|(c, _)| c.number)
            .collect(),
    };

    // Every link out of the kept akter, before any copy exists.
    let mut out = Vec::new();
    for node in &nodes {
        for (link, target) in links::from(conn, tenant, node.row.id).await? {
            out.push((node.row.id, link, target));
        }
    }

    // The locks every write takes, in their order, before the first:
    // the order lock, then the existing akter the copies join or link to.
    containers::lock_order(conn, tenant).await?;
    let mut existing: Vec<String> = inside.clone();
    existing.extend(
        out.iter()
            .filter(|(_, _, t)| !kept.contains(&t.id))
            .map(|(_, _, t)| t.number.clone()),
    );
    existing.sort();
    existing.dedup();
    let refs: Vec<&str> = existing.iter().map(String::as_str).collect();
    containers::lock_akter(conn, tenant, &refs).await?;

    let said = |what: &str| match &reason {
        Some(reason) => format!("{reason} ({what})"),
        None => what[..1].to_uppercase() + &what[1..],
    };
    // Each copy, parents first, each in its parent's copy.
    let mut copies: HashMap<Uuid, AktRow> = HashMap::new();
    for node in &nodes {
        let into = match node.parent {
            None => inside.clone(),
            Some(parent) => vec![copies[&parent].number.clone()],
        };
        let made = super::create::create_in(
            conn,
            caller,
            node.ty.ty.key(),
            &copied_fields(&node.ty, &node.row.fields),
            Some(said(&format!("copied from {}", node.row.number))),
            &into,
        )
        .await?;
        store::insert_origin(conn, tenant, made.id, node.row.id).await?;
        copies.insert(node.row.id, made);
    }
    copy_sections(conn, caller, tenant, &nodes, &copies, &said).await?;
    copy_links(conn, caller, &out, &copies, &said).await?;

    store::akt_by_number(conn, tenant, &copies[&root.row.id].number)
        .await?
        .ok_or_else(|| AktError::Internal("the copy vanished".to_owned()))
}

/// Sets each copy's sections, once the members they place exist, placing
/// the copies where the originals were.
// r[impl akt.copy]
async fn copy_sections(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    tenant: Uuid,
    nodes: &[&Node],
    copies: &HashMap<Uuid, AktRow>,
    said: &(dyn Fn(&str) -> String + Sync),
) -> Result<(), AktError> {
    let renamed: HashMap<&str, String> = nodes
        .iter()
        .map(|n| (n.row.number.as_str(), copies[&n.row.id].number.clone()))
        .collect();
    for node in nodes {
        let Some(field) = node
            .ty
            .ty
            .document()
            .fields
            .iter()
            .find(|f| f.kind == FieldKind::Sections)
        else {
            continue;
        };
        let Some(raw) = node.row.fields.get(&field.key) else {
            continue;
        };
        let Ok(kuria_engine::Value::Sections(items)) = kuria_engine::Value::from_json(field, raw)
        else {
            continue;
        };
        let copied = kuria_engine::sections::copied(&items, &|n| renamed.contains_key(n), &|n| {
            renamed[n].clone()
        });
        if copied.is_empty() {
            continue;
        }
        let copy = &copies[&node.row.id];
        let now = store::akt_by_number(conn, tenant, &copy.number)
            .await?
            .ok_or_else(|| AktError::Internal(format!("the copy {} vanished", copy.number)))?;
        let mut fields = Map::new();
        fields.insert(
            field.key.clone(),
            kuria_engine::Value::Sections(copied).to_json(),
        );
        super::edit_in(
            conn,
            caller,
            &copy.number,
            now.revision,
            &fields,
            Some(said(&format!("sections copied from {}", node.row.number))),
        )
        .await?;
    }

    Ok(())
}

/// Copies the links out of the copied akter: between the copies within the
/// tree, and to the same akt, with its pin, outside it.
// r[impl akt.copy.links]
async fn copy_links(
    conn: &mut kuria_store::Connection,
    caller: Caller<'_>,
    out: &[(Uuid, LinkRow, AktRow)],
    copies: &HashMap<Uuid, AktRow>,
    said: &(dyn Fn(&str) -> String + Sync),
) -> Result<(), AktError> {
    for (source, link, target) in out {
        let (to, pin) = match copies.get(&target.id) {
            Some(copy) => (copy.number.clone(), None),
            None => (target.number.clone(), link.pinned_revision),
        };
        super::links::add_link_in(
            conn,
            caller,
            &copies[source].number,
            &link.kind,
            &to,
            pin,
            Some(said("link copied")),
        )
        .await?;
    }

    Ok(())
}
