//! Outcomes, as the API serves them: derived from verdicts and members each
//! time they are read, never stored (`akt.outcome.derived`).

use std::collections::HashMap;

use kuria_config::AktType;
use kuria_engine::outcomes::{self, Outcome};
use kuria_store::akter::{self as store, AktRow};
use kuria_store::containers;
use uuid::Uuid;

use super::{AktError, StoredType, type_of};

/// The tenant's akt types, by key, as their latest versions: what decides
/// whether a member's type has outcomes.
pub struct Types(HashMap<String, StoredType>);

impl Types {
    pub async fn of(conn: &mut kuria_store::Connection, tenant: Uuid) -> Result<Self, AktError> {
        let mut types = HashMap::new();
        for row in store::latest_type_versions(conn, tenant).await? {
            let stored = StoredType::read(row)?;
            types.insert(stored.ty.key().to_owned(), stored);
        }
        Ok(Self(types))
    }

    fn has_outcomes(&self, ty: &AktType) -> bool {
        // The akt's own version first: it is what the akt is held to.
        let lookup = |key: &str| {
            if key == ty.key() {
                Some(ty)
            } else {
                self.0.get(key).map(|t| &t.ty)
            }
        };
        outcomes::has_outcomes(ty.key(), &lookup)
    }
}

/// `akt`'s outcome, `None` if its type has none: from its own verdicts if
/// it has a `verdict` field, else from its members' outcomes, recursively.
// r[impl akt.outcome]
// r[impl akt.outcome.derived]
pub async fn of(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    types: &Types,
    akt: &AktRow,
) -> Result<Option<Outcome>, AktError> {
    let ty = type_of(conn, tenant, akt).await?;
    if !types.has_outcomes(&ty.ty) {
        return Ok(None);
    }
    let fields = kuria_engine::fields_from_json(
        &ty.ty,
        akt.fields.as_object().unwrap_or(&serde_json::Map::new()),
    )
    .map_err(|e| AktError::Internal(format!("akt {}: {e}", akt.number)))?;
    if let Some(own) = outcomes::of_fields(&ty.ty, &fields) {
        return Ok(Some(own));
    }
    let mut held = Vec::new();
    for (member, _) in containers::members_with_types(conn, tenant, akt.id).await? {
        // Containment has no cycles, so this ends.
        if let Some(outcome) = Box::pin(of(conn, tenant, types, &member)).await? {
            held.push(outcome);
        }
    }
    Ok(Some(outcomes::of_all(held)))
}

/// The outcome of the akt `number`, if it exists and has outcomes.
pub async fn of_number(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    types: &Types,
    number: &str,
) -> Result<Option<Outcome>, AktError> {
    match store::akt_by_number(conn, tenant, number).await? {
        Some(akt) => of(conn, tenant, types, &akt).await,
        None => Ok(None),
    }
}
