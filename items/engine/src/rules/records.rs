//! What membership and link revisions record of the other akt
//! (`container.revisions`, `link.revisions`).

use kuria_config::LinkDef;
use serde_json::{Value as Json, json};

/// One side of a membership: the other akt's identifier and number.
// r[impl container.revisions]
#[must_use]
pub fn membership_entry(id: &str, number: &str) -> Json {
    json!({ "akt": id, "number": number })
}

/// One side of a link: the other akt, the kind, the link's name as this
/// akt reads it, and any pinned revision.
// r[impl link.revisions]
#[must_use]
pub fn link_entry(id: &str, number: &str, def: &LinkDef, name: &str, pin: Option<i32>) -> Json {
    let mut entry = json!({ "akt": id, "number": number, "kind": def.key, "name": name });
    if let Some(pin) = pin {
        entry["revision"] = json!(pin);
    }
    entry
}
