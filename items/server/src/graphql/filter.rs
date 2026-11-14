//! Conditions on field values (`akt.filter`), read by each field's kind
//! into the JSON a matching value stores.

use async_graphql::{ErrorExtensions, InputObject};
use chrono::NaiveDate;
use kuria_config::{AktType, FieldDef, FieldKind};
use kuria_store::akter::{Condition, Match};
use serde_json::{Value as Json, json};
use uuid::Uuid;

/// A condition on one field's value.
#[derive(InputObject, Clone, serde::Serialize)]
pub struct FieldCondition {
    /// The field's key.
    pub field: String,
    /// The value it must hold: a choice's option key; a date as
    /// `YYYY-MM-DD`; a single-line text whole; for a `user` field or a
    /// checkbox's `for`, `me`, a subject identifier, or `contact:<id>`.
    pub equals: String,
}

/// Whom a condition on a user field or a checkbox names, as the JSON forms
/// a value stores them in: a subject, and the contacts linked to them, or a
/// contact.
pub struct Named {
    pub users: Vec<Uuid>,
    pub contacts: Vec<Uuid>,
}

fn refused(message: String) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, e| e.set("code", "BAD_REQUEST"))
}

/// Who `equals` names in a condition on `field`: `me` is `me`; a subject
/// identifier, that subject; `contact:<id>`, that contact. The caller adds
/// the contacts linked to a named user.
pub fn named(
    field: &str,
    equals: &str,
    me: Uuid,
) -> async_graphql::Result<(Option<Uuid>, Option<Uuid>)> {
    if equals == "me" {
        return Ok((Some(me), None));
    }
    if let Some(contact) = equals.strip_prefix("contact:") {
        return Uuid::parse_str(contact)
            .map(|id| (None, Some(id)))
            .map_err(|_| refused(format!("`{equals}` names no contact, for `{field}`")));
    }
    Uuid::parse_str(equals)
        .map(|subject| (Some(subject), None))
        .map_err(|_| {
            refused(format!(
                "`{field}` is matched by `me`, a subject identifier or `contact:<id>`, not `{equals}`"
            ))
        })
}

/// Whether conditions may be put on fields of `kind`.
pub fn filterable(field: &FieldDef) -> bool {
    match field.kind {
        FieldKind::Choice | FieldKind::Choices | FieldKind::Checkbox | FieldKind::Date => true,
        FieldKind::User => !field.multiple,
        FieldKind::Text => !field.multiline,
        _ => false,
    }
}

/// The field a condition is on, if `ty` has it and it is filterable.
pub fn field_of<'a>(
    ty: &'a AktType,
    condition: &FieldCondition,
) -> async_graphql::Result<&'a FieldDef> {
    let field = ty
        .document()
        .fields
        .iter()
        .find(|f| f.key == condition.field)
        .ok_or_else(|| refused(format!("`{}` has no field `{}`", ty.key(), condition.field)))?;
    if !filterable(field) {
        return Err(refused(format!(
            "`{}` can't be filtered by: only choices, users, checkboxes, dates and single-line text can",
            field.key
        )));
    }
    Ok(field)
}

/// A condition as the store matches it. `named` is whom it names, for a
/// user field or a checkbox.
// r[impl akt.filter]
pub fn condition(
    field: &FieldDef,
    equals: &str,
    named: Option<&Named>,
) -> async_graphql::Result<Condition> {
    let key = &field.key;
    let contains = |value: Json| Match::Contains(json!({ key.clone(): value }));
    let someone = |named: &Named| -> Vec<Json> {
        named
            .users
            .iter()
            .map(|u| json!(u.to_string()))
            .chain(
                named
                    .contacts
                    .iter()
                    .map(|c| json!({ "contact": c.to_string() })),
            )
            .collect()
    };
    let matches = match field.kind {
        FieldKind::Choice | FieldKind::Choices => {
            if !field.options.iter().any(|o| o.key == equals) {
                return Err(refused(format!("`{key}` has no option `{equals}`")));
            }
            if field.kind == FieldKind::Choice {
                vec![contains(json!(equals))]
            } else {
                vec![contains(json!([equals]))]
            }
        }
        FieldKind::Date => {
            NaiveDate::parse_from_str(equals, "%Y-%m-%d")
                .map_err(|_| refused(format!("`{key}` is matched by a date, YYYY-MM-DD")))?;
            vec![contains(json!(equals))]
        }
        FieldKind::Text => vec![contains(json!(equals))],
        FieldKind::User => {
            someone(named.ok_or_else(|| refused(format!("say whom `{key}` names")))?)
                .into_iter()
                .map(contains)
                .collect()
        }
        FieldKind::Checkbox => {
            let named =
                named.ok_or_else(|| refused(format!("say whom `{key}` was checked for")))?;
            someone(named)
                .into_iter()
                .map(|who| contains(json!({ "for": who })))
                .chain(named.users.iter().map(|u| Match::CheckedForSelf {
                    field: key.clone(),
                    subject: u.to_string(),
                }))
                .collect()
        }
        _ => return Err(refused(format!("`{key}` can't be filtered by"))),
    };
    Ok(Condition(matches))
}
