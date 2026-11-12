//! Drafting a field from the akter around its akt.

use std::collections::{BTreeMap, HashMap, HashSet};

use kuria_engine::draft::{Listed, render, select};
use kuria_engine::{Value, fields_from_json};
use kuria_store::akter::{self as store, AktRow};
use kuria_store::containers;
use serde_json::{Map, Value as Json};
use uuid::Uuid;

use super::{AktError, Akter, Caller, StoredType, require_read, type_of};
use crate::authorize::{Authorizer, Permission};

/// More members than any recipe should ever meet in one container.
const PER_CONTAINER: i64 = 10_000;

impl Akter {
    /// Drafts the field `field` of the akt `number` by its recipe, as an
    /// edit based on `expected_revision`.
    // r[impl draft.edit]
    pub async fn draft_field(
        &self,
        caller: Caller<'_>,
        number: &str,
        expected_revision: i32,
        field: &str,
        reason: Option<String>,
    ) -> Result<AktRow, AktError> {
        let text = self.draft(caller, number, field).await?;
        let mut fields = Map::new();
        fields.insert(field.to_owned(), Json::String(text));
        let reason = reason.or_else(|| Some(format!("Drafted {field}")));
        // An ordinary edit: the same permission, revision and final-state
        // rules, and an ordinary revision.
        self.edit(caller, number, expected_revision, &fields, reason)
            .await
    }

    /// The draft text of the field `field` of the akt `number`.
    // r[impl draft.sources]
    async fn draft(
        &self,
        caller: Caller<'_>,
        number: &str,
        field: &str,
    ) -> Result<String, AktError> {
        let auth = caller.auth;
        let tenant: Uuid = caller.actor.tenant.into();
        let mut conn = self.pool.get().await?;
        let akt = store::akt_by_number(&mut conn, tenant, number)
            .await?
            .ok_or_else(|| AktError::NotFound(number.to_owned()))?;
        let ty = type_of(&mut conn, tenant, &akt).await?;
        require_read(auth, &ty, number).await?;
        let Some(recipe) = ty.ty.field(field).and_then(|f| f.draft.clone()) else {
            return Err(AktError::NotDraftable(field.to_owned()));
        };

        // Every akt type the subject may read: what may be looked inside
        // and listed at all.
        let mut readable = Vec::new();
        for version in store::latest_type_versions(&mut conn, tenant).await? {
            if reads(auth, &version.key).await? {
                readable.push(version.key);
            }
        }

        let mut seen: HashSet<Uuid> = HashSet::from([akt.id]);
        let mut found: Vec<(AktRow, String)> = Vec::new();
        let mut frontier: Vec<Uuid> = Vec::new();
        for (container, key) in containers::containers_of(&mut conn, tenant, akt.id).await? {
            if readable.contains(&key)
                && recipe.counts(&key, &container.state)
                && seen.insert(container.id)
            {
                frontier.push(container.id);
            }
        }
        for _ in 0..recipe.depth {
            if frontier.is_empty() {
                break;
            }
            let members =
                containers::members_of(&mut conn, tenant, &frontier, &readable, PER_CONTAINER)
                    .await?;
            let keys = type_keys(&mut conn, tenant, &members).await?;
            frontier = Vec::new();
            for (_, member) in members {
                if !seen.insert(member.id) {
                    continue;
                }
                let key = keys[&member.type_version_id].clone();
                // r[impl draft.states]
                if !recipe.counts(&key, &member.state) {
                    continue;
                }
                if recipe.through.is_empty() || recipe.through.contains(&key) {
                    frontier.push(member.id);
                }
                found.push((member, key));
            }
        }

        let listed_rows: Vec<AktRow> = found
            .into_iter()
            .filter(|(_, key)| recipe.types.contains(key))
            .map(|(row, _)| row)
            .collect();
        // r[impl draft.latest]
        let mut listed_rows = select(
            &recipe,
            listed_rows,
            |row| (row.created_at, row.id),
            |row, key| row.fields.get(key).cloned(),
        );
        listed_rows.sort_by(|a, b| a.rank.cmp(&b.rank));
        let listed = describe(&mut conn, tenant, &listed_rows).await?;
        Ok(render(&recipe, &listed))
    }
}

/// How the user `subject` is shown, remembered for the rest of the draft.
async fn user_name(
    conn: &mut kuria_store::Connection,
    names: &mut HashMap<Uuid, String>,
    subject: Uuid,
) -> Result<String, AktError> {
    if let Some(name) = names.get(&subject) {
        return Ok(name.clone());
    }
    let person = kuria_store::people::find(conn, &[subject]).await?.pop();
    let name = crate::graphql::display_name(subject, person.as_ref());
    names.insert(subject, name.clone());
    Ok(name)
}

async fn reads(auth: &Authorizer, key: &str) -> Result<bool, AktError> {
    Ok(auth
        .allows(&Permission::Read {
            key: key.to_owned(),
        })
        .await?)
}

/// The akt type key of each type version `members` use.
async fn type_keys(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    members: &[(Uuid, AktRow)],
) -> Result<HashMap<Uuid, String>, AktError> {
    let mut ids: Vec<Uuid> = members.iter().map(|(_, m)| m.type_version_id).collect();
    ids.sort_unstable();
    ids.dedup();
    Ok(store::type_versions_by_id(conn, tenant, &ids)
        .await?
        .into_iter()
        .map(|v| (v.id, v.key))
        .collect())
}

/// What each line of the draft may show about `rows`.
async fn describe(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    rows: &[AktRow],
) -> Result<Vec<Listed>, AktError> {
    let mut types: HashMap<Uuid, StoredType> = HashMap::new();
    let mut names: HashMap<Uuid, String> = HashMap::new();
    let mut listed = Vec::with_capacity(rows.len());
    for row in rows {
        if let std::collections::hash_map::Entry::Vacant(slot) = types.entry(row.type_version_id) {
            slot.insert(type_of(conn, tenant, row).await?);
        }
        let ty = &types[&row.type_version_id];
        let values = fields_from_json(&ty.ty, row.fields.as_object().unwrap_or(&Map::new()))
            .map_err(|e| AktError::Internal(format!("akt {}: {e}", row.number)))?;
        let mut fields = BTreeMap::new();
        for (key, value) in &values {
            let Some(def) = ty.ty.field(key) else {
                continue;
            };
            let text = match value {
                // r[impl draft.render]
                Value::User(subject) => user_name(conn, &mut names, *subject).await?,
                Value::Attachment(id) => kuria_store::attachments::find(conn, tenant, &[*id])
                    .await?
                    .pop()
                    .map_or_else(|| id.to_string(), |a| a.name),
                other => other.display(def),
            };
            fields.insert(key.clone(), text);
        }
        listed.push(Listed {
            number: row.number.clone(),
            akt_type: ty.ty.key().to_owned(),
            type_name: ty.ty.document().name.clone(),
            state: ty
                .ty
                .state(&row.state)
                .map_or_else(|| row.state.clone(), |s| s.name.clone()),
            fields,
        });
    }
    Ok(listed)
}
