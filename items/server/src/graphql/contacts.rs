//! Contacts: people a tenant names before, or without, them being users.

use std::sync::Arc;

use async_graphql::{Context, ErrorExtensions, ID, InputObject, Object};
use chrono::{DateTime, Utc};
use kuria_store::contacts::{self, Contact as Row, ContactError};
use kuria_store::people;
use uuid::Uuid;

use super::Principal;
use super::people::{User, display_name};
use crate::akter::Akter;
use crate::authorize::{Authorizer, Permission, Unavailable};

/// The longest name a contact may have, in characters.
const NAME_LIMIT: usize = 200;
/// The longest email a contact may have, in bytes.
const EMAIL_LIMIT: usize = 254;

fn coded(message: impl Into<String>, code: &'static str) -> async_graphql::Error {
    async_graphql::Error::new(message.into()).extend_with(|_, e| e.set("code", code))
}

fn internal(e: impl std::fmt::Display) -> async_graphql::Error {
    tracing::error!("contacts: {e}");
    coded("internal error", "INTERNAL")
}

fn unavailable(e: &Unavailable) -> async_graphql::Error {
    tracing::warn!("contacts: {e}");
    coded("authorization is unavailable; try again", "UNAVAILABLE")
}

fn tenant(ctx: &Context<'_>) -> async_graphql::Result<Uuid> {
    Ok(ctx.data::<Principal>()?.tenant.into())
}

fn authorizer<'a>(ctx: &Context<'a>) -> async_graphql::Result<&'a Authorizer> {
    ctx.data::<Arc<Authorizer>>().map(AsRef::as_ref)
}

async fn connection(
    ctx: &Context<'_>,
) -> async_graphql::Result<diesel_async::pooled_connection::deadpool::Object<kuria_store::Connection>>
{
    ctx.data::<Akter>()?.pool().get().await.map_err(internal)
}

/// Refuses a caller without `permission`.
async fn require(ctx: &Context<'_>, permission: &Permission) -> async_graphql::Result<()> {
    if authorizer(ctx)?
        .allows(permission)
        .await
        .map_err(|e| unavailable(&e))?
    {
        Ok(())
    } else {
        Err(coded("not permitted", "FORBIDDEN"))
    }
}

/// A person a tenant names before, or without, them being a user.
pub struct Contact(pub Row);

#[Object]
impl Contact {
    async fn id(&self) -> ID {
        ID(self.0.id.to_string())
    }

    /// How the contact is shown: as the user it is linked to, else by its
    /// name, else, once erased, by its identifier.
    // r[impl contact.show]
    async fn display_name(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        if let Some(subject) = self.0.linked_subject {
            let mut conn = connection(ctx).await?;
            let person = people::find(&mut conn, &[subject])
                .await
                .map_err(internal)?
                .pop();
            if person
                .as_ref()
                .is_some_and(|p| p.name.is_some() || p.email.is_some())
            {
                return Ok(display_name(subject, person.as_ref()));
            }
        }
        Ok(self.0.name.clone().unwrap_or_else(|| self.0.id.to_string()))
    }

    /// Its own name; `null` once erased.
    async fn name(&self) -> Option<&str> {
        self.0.name.as_deref()
    }

    /// Its email, to those who may list contacts; `null` to anyone else.
    // r[impl contact.show]
    async fn email(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<&str>> {
        let reads = authorizer(ctx)?
            .reads_contacts()
            .await
            .map_err(|e| unavailable(&e))?;
        Ok(if reads { self.0.email.as_deref() } else { None })
    }

    /// The user it is linked to, once they signed in.
    async fn user(&self) -> Option<User> {
        self.0.linked_subject.map(User)
    }

    async fn linked_at(&self) -> Option<DateTime<Utc>> {
        self.0.linked_at
    }

    async fn created_by(&self) -> User {
        User(self.0.created_by)
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at
    }
}

#[derive(Default)]
pub struct ContactsQuery;

#[Object]
impl ContactsQuery {
    /// The tenant's contacts, by name.
    // r[impl contact.list]
    async fn contacts(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Contact>> {
        if !authorizer(ctx)?
            .reads_contacts()
            .await
            .map_err(|e| unavailable(&e))?
        {
            return Err(coded("not permitted", "FORBIDDEN"));
        }
        let tenant = tenant(ctx)?;
        let mut conn = connection(ctx).await?;
        Ok(contacts::list(&mut conn, tenant)
            .await
            .map_err(internal)?
            .into_iter()
            .map(Contact)
            .collect())
    }
}

#[derive(InputObject)]
pub struct AddContactInput {
    name: String,
    email: Option<String>,
}

#[derive(InputObject)]
pub struct EditContactInput {
    id: ID,
    name: String,
    email: Option<String>,
}

#[derive(Default)]
pub struct ContactsMutation;

#[Object]
impl ContactsMutation {
    /// Adds a contact to the tenant.
    // r[impl contact.add]
    // r[impl contact.unique-email]
    async fn add_contact(
        &self,
        ctx: &Context<'_>,
        input: AddContactInput,
    ) -> async_graphql::Result<Contact> {
        require(ctx, &Permission::ManageContacts).await?;
        let principal = ctx.data::<Principal>()?;
        let (name, email) = checked(&input.name, input.email.as_deref())?;
        let row = Row {
            id: Uuid::now_v7(),
            tenant_id: principal.tenant.into(),
            name: Some(name),
            email,
            linked_subject: None,
            linked_at: None,
            created_by: principal.subject.into(),
            created_at: Utc::now(),
        };
        let mut conn = connection(ctx).await?;
        contacts::insert(&mut conn, &row).await.map_err(refused)?;
        Ok(Contact(row))
    }

    /// Changes an unlinked contact's name and email.
    // r[impl contact.edit]
    // r[impl contact.unique-email]
    async fn edit_contact(
        &self,
        ctx: &Context<'_>,
        input: EditContactInput,
    ) -> async_graphql::Result<Contact> {
        require(ctx, &Permission::ManageContacts).await?;
        let tenant = tenant(ctx)?;
        let id = contact_id(&input.id)?;
        let (name, email) = checked(&input.name, input.email.as_deref())?;
        let mut conn = connection(ctx).await?;
        let edited = contacts::edit_unlinked(&mut conn, tenant, id, &name, email.as_deref())
            .await
            .map_err(refused)?;
        let row = contacts::find(&mut conn, tenant, &[id])
            .await
            .map_err(internal)?
            .pop()
            .ok_or_else(|| coded(format!("there is no contact `{id}`"), "NOT_FOUND"))?;
        if !edited {
            return Err(coded(
                "the contact is linked to a user, so it can't be edited",
                "LINKED",
            ));
        }
        Ok(Contact(row))
    }

    /// Deletes a contact's name and email, keeping the contact so that the
    /// values naming it still read.
    // r[impl contact.erase]
    async fn erase_contact(&self, ctx: &Context<'_>, id: ID) -> async_graphql::Result<Contact> {
        require(ctx, &Permission::ManageContacts).await?;
        let tenant = tenant(ctx)?;
        let id = contact_id(&id)?;
        let mut conn = connection(ctx).await?;
        if !contacts::erase(&mut conn, tenant, id)
            .await
            .map_err(internal)?
        {
            return Err(coded(format!("there is no contact `{id}`"), "NOT_FOUND"));
        }
        let row = contacts::find(&mut conn, tenant, &[id])
            .await
            .map_err(internal)?
            .pop()
            .ok_or_else(|| internal("an erased contact is gone"))?;
        Ok(Contact(row))
    }
}

fn contact_id(id: &ID) -> async_graphql::Result<Uuid> {
    Uuid::parse_str(id).map_err(|_| coded(format!("there is no contact `{}`", id.0), "NOT_FOUND"))
}

fn refused(error: ContactError) -> async_graphql::Error {
    match error {
        ContactError::DuplicateEmail => coded(error.to_string(), "DUPLICATE_EMAIL"),
        ContactError::Database(e) => internal(e),
    }
}

/// A contact's name and email, trimmed, or why they can't be a contact's.
fn checked(name: &str, email: Option<&str>) -> async_graphql::Result<(String, Option<String>)> {
    let invalid = |message: &str| coded(message, "INVALID_CONTACT");
    let name = name.trim();
    if name.is_empty() {
        return Err(invalid("a contact needs a name"));
    }
    if name.chars().count() > NAME_LIMIT {
        return Err(invalid("a contact's name is at most 200 characters"));
    }
    if name.chars().any(char::is_control) {
        return Err(invalid("a contact's name can't hold control characters"));
    }
    let email = email.map(str::trim).filter(|e| !e.is_empty());
    if let Some(email) = email {
        let valid = email.len() <= EMAIL_LIMIT
            && !email.chars().any(|c| c.is_whitespace() || c.is_control())
            && email.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty() && !domain.is_empty() && !domain.contains('@')
            });
        if !valid {
            return Err(invalid("that is not an email address"));
        }
    }
    Ok((name.to_owned(), email.map(str::to_owned)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_emails_are_trimmed_and_checked() {
        assert_eq!(
            checked(" Bob ", Some(" bob@x.example ")).unwrap(),
            ("Bob".to_owned(), Some("bob@x.example".to_owned()))
        );
        assert_eq!(checked("Bob", Some("  ")).unwrap().1, None);
        for (name, email) in [
            ("  ", None),
            ("Bo\nb", None),
            ("Bob", Some("bob")),
            ("Bob", Some("@x.example")),
            ("Bob", Some("bob@")),
            ("Bob", Some("b ob@x.example")),
            ("Bob", Some("a@b@c")),
        ] {
            assert!(checked(name, email).is_err(), "{name:?} {email:?}");
        }
        assert!(checked(&"x".repeat(201), None).is_err());
    }
}
