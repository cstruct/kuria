//! The commands that read (`cli.read`): what a tenant has (`types`,
//! `schema`, `users`, `contacts`), lists of akter (`ls`, `find`), and one
//! akt (`cat`, `show`, `history`).

mod akter;
mod catalog;
mod list;

pub use akter::{cat, history, show};
pub use catalog::{contacts, schema, types, users};
pub use list::{Listing, find, ls};

use serde_json::json;

use crate::error::CliError;
use crate::model::{SHAPE, Shape, read};
use crate::session::Session;
use crate::values::People;

/// The latest version of the akt type `key`, or `UNKNOWN_AKT_TYPE`.
pub async fn shape(session: &Session, key: &str) -> Result<Shape, CliError> {
    let data = session
        .graphql(
            &format!("query($k: String!) {{ aktType(key: $k) {{ ...Shape document }} }} {SHAPE}"),
            json!({ "k": key }),
        )
        .await?;
    if data["aktType"].is_null() {
        return Err(CliError::from_server(
            "UNKNOWN_AKT_TYPE",
            format!("No akt type `{key}`; see `kuria types`"),
            None,
        ));
    }
    read(data["aktType"].clone(), "the akt type")
}

/// The tenant's users, and its contacts if the session may list them.
pub async fn people(session: &Session) -> Result<People, CliError> {
    let users = session
        .graphql("{ users { id name email } }", json!({}))
        .await?;
    let contacts = match session
        .graphql("{ contacts { id displayName name email } }", json!({}))
        .await
    {
        Ok(data) => data["contacts"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| {
                let s = |k: &str| c[k].as_str().map(str::to_owned);
                (
                    s("id").unwrap_or_default(),
                    s("displayName").unwrap_or_default(),
                    s("name"),
                    s("email"),
                )
            })
            .collect(),
        // Without the right to list contacts, only users are named.
        Err(error) if error.code == "FORBIDDEN" => Vec::new(),
        Err(error) => return Err(error),
    };
    Ok(People {
        users: read(users["users"].clone(), "users")?,
        contacts,
    })
}
