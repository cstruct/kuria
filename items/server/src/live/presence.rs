//! Awareness, stamped with who each client is (`live.presence`).
//!
//! A client's awareness state is its own JSON, such as where its cursor is.
//! The server sets its `user` to who the client is, as Kuria names them
//! (`person.name`), whatever the client said, so that no client can pose
//! as someone else.

use std::sync::Arc;

use serde_json::{Value as Json, json};
use uuid::Uuid;
use yrs::sync::AwarenessUpdate;

use crate::akter::{AktError, Akter};

/// How Kuria names `subject` in `tenant` (`person.name`), and, when
/// `acting` really acts as them, as `acting` for `subject`.
// r[impl live.presence]
pub(super) async fn name_of(
    akter: &Akter,
    tenant: Uuid,
    subject: Uuid,
    acting: Option<Uuid>,
) -> Result<String, AktError> {
    use crate::graphql::people::someone_name;
    use kuria_engine::Someone::User;
    let mut conn = akter.pool().get().await?;
    let subject_name = someone_name(&mut conn, tenant, User(subject)).await?;
    Ok(match acting {
        Some(actor) => format!(
            "{} for {subject_name}",
            someone_name(&mut conn, tenant, User(actor)).await?
        ),
        None => subject_name,
    })
}

/// `update` with every state in it saying it is `subject`'s, named
/// `name`; a state saying its client left stays as it is.
// r[impl live.presence]
pub(super) fn stamp(mut update: AwarenessUpdate, subject: Uuid, name: &str) -> AwarenessUpdate {
    for entry in update.clients.values_mut() {
        let state = match serde_json::from_str::<Json>(&entry.json) {
            Ok(Json::Null) => continue,
            Ok(Json::Object(state)) => state,
            _ => serde_json::Map::new(),
        };
        let mut state = state;
        state.insert(
            "user".to_owned(),
            json!({ "id": subject.to_string(), "name": name }),
        );
        entry.json = Arc::from(Json::Object(state).to_string());
    }
    update
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use yrs::ClientID;
    use yrs::sync::awareness::AwarenessUpdateEntry;

    use super::*;

    // r[verify live.presence]
    #[test]
    fn a_client_is_who_its_token_says() {
        let entry = |json: &str| AwarenessUpdateEntry {
            clock: 1,
            json: Arc::from(json),
        };
        let update = AwarenessUpdate {
            clients: HashMap::from([
                (
                    ClientID::new(1),
                    entry(r#"{"user":{"name":"Someone else"},"cursor":{"anchor":3}}"#),
                ),
                (ClientID::new(2), entry("null")),
                (ClientID::new(3), entry("[]")),
            ]),
        };
        let subject = Uuid::from_u128(5);
        let stamped = stamp(update, subject, "Ada");
        let json = |id: u64| -> Json {
            serde_json::from_str(&stamped.clients[&ClientID::new(id)].json).unwrap()
        };
        assert_eq!(
            json(1),
            json!({ "user": { "id": subject.to_string(), "name": "Ada" }, "cursor": { "anchor": 3 } })
        );
        assert_eq!(json(2), Json::Null);
        assert_eq!(json(3)["user"]["name"], "Ada");
    }
}
