//! The commands that write (`cli.write`, `cli.apply`): creating and
//! editing akter, from files or `--set`; transitions and drafts; arranging
//! containers, order and links; and akt type documents.

mod arrange;
mod bundles;
mod edit;
mod types;

pub use arrange::{Membership, link, membership, move_akt, transfer, unlink};
pub use bundles::{Exported, export as export_bundle, import as import_bundle};
pub use edit::{Created, apply, create, draft, perform, set};
pub use types::{define, export};

use serde_json::{Map, Value as Json, json};

use crate::error::CliError;
use crate::file::{AKT, Akt};
use crate::model::{SHAPE, SOMEONE, Shape, VALUES, read};
use crate::session::Session;
use crate::values::{People, input_of, set_value};

/// An akt as it is now, with its type.
pub(crate) async fn akt(session: &Session, number: &str) -> Result<Akt, CliError> {
    let data = session
        .graphql(
            &format!(
                "query($n: String!) {{ akt(number: $n) {{ {AKT} }} }} {SHAPE} {VALUES} {SOMEONE}"
            ),
            json!({ "n": number }),
        )
        .await?;
    if data["akt"].is_null() {
        return Err(CliError::from_server(
            "NOT_FOUND",
            format!("No akt `{number}`"),
            None,
        ));
    }
    read(data["akt"].clone(), "the akt")
}

/// Whom values can name, fetched only when `shape`'s `keys` need them.
pub(crate) async fn people_for<'a>(
    session: &Session,
    shape: &Shape,
    keys: impl IntoIterator<Item = &'a String>,
) -> Result<People, CliError> {
    let needs = keys
        .into_iter()
        .filter_map(|k| shape.field(k))
        .any(|f| matches!(f.kind.as_str(), "USER" | "CHECKBOX"));
    if needs {
        crate::reads::people(crate::reads::Source::Server(session)).await
    } else {
        Ok(People::default())
    }
}

/// `--set <field>=<value>`s as the fields input takes them.
pub(crate) fn split_sets(shape: &Shape, sets: &[String]) -> Result<Map<String, Json>, CliError> {
    let mut values = Map::new();
    for set in sets {
        let Some((key, value)) = set.split_once('=') else {
            return Err(CliError::refused(
                "BAD_REQUEST",
                format!("`{set}` isn't `<field>=<value>`"),
            ));
        };
        let key = key.trim();
        let field = known_field(shape, key)?;
        values.insert(key.to_owned(), set_value(field, value)?);
    }
    Ok(values)
}

/// `shape`'s field `key`, or a refusal naming it.
pub(crate) fn known_field<'a>(
    shape: &'a Shape,
    key: &str,
) -> Result<&'a crate::model::FieldDef, CliError> {
    shape.field(key).ok_or_else(|| CliError {
        field: Some(key.to_owned()),
        ..CliError::refused(
            "UNKNOWN_FIELD",
            format!(
                "{} has no field `{key}`; see `kuria schema {}`",
                shape.name, shape.key
            ),
        )
    })
}

/// Given values as the server's inputs.
pub(crate) fn inputs(
    shape: &Shape,
    given: &Map<String, Json>,
    people: &People,
) -> Result<Map<String, Json>, CliError> {
    given
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                input_of(known_field(shape, key)?, value, people)?,
            ))
        })
        .collect()
}
