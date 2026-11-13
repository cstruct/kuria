//! Contacts: people a tenant names before, or without, them being users.

use std::borrow::Cow;
use std::sync::Arc;

use async_graphql::{Context, ErrorExtensions, ID, InputObject, Object};
use chrono::{DateTime, Utc};
use kuria_store::contacts::{self, Contact as Row, ContactError};
use kuria_store::people;
use uuid::Uuid;

use super::Principal;
use super::people::{User, contact_name};
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

/// A person a tenant names before, or without, them being a user: loaded,
/// or named by a value and loaded when a field needs it.
pub enum Contact {
    Loaded(Row),
    Named(Uuid),
}

impl Contact {
    fn id(&self) -> Uuid {
        match self {
            Self::Loaded(row) => row.id,
            Self::Named(id) => *id,
        }
    }

    /// The contact's row; `None` if the tenant has no such contact.
    async fn row(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Cow<'_, Row>>> {
        match self {
            Self::Loaded(row) => Ok(Some(Cow::Borrowed(row))),
            Self::Named(id) => {
                let tenant = tenant(ctx)?;
                let mut conn = connection(ctx).await?;
                Ok(contacts::find(&mut conn, tenant, &[*id])
                    .await
                    .map_err(internal)?
                    .pop()
                    .map(Cow::Owned))
            }
        }
    }

    async fn loaded(&self, ctx: &Context<'_>) -> async_graphql::Result<Cow<'_, Row>> {
        self.row(ctx)
            .await?
            .ok_or_else(|| coded(format!("there is no contact `{}`", self.id()), "NOT_FOUND"))
    }
}

#[Object]
impl Contact {
    #[graphql(name = "id")]
    async fn graphql_id(&self) -> ID {
        ID(self.id().to_string())
    }

    /// How the contact is shown: as the user it is linked to, else by its
    /// name, else, once erased, by its identifier.
    // r[impl contact.show]
    async fn display_name(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        let Some(row) = self.row(ctx).await? else {
            return Ok(self.id().to_string());
        };
        let mut conn = connection(ctx).await?;
        contact_name(&mut conn, &row).await.map_err(internal)
    }

    /// Its own name; `null` once erased.
    async fn name(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<String>> {
        Ok(self.row(ctx).await?.and_then(|r| r.name.clone()))
    }

    /// Its email, to those who may list contacts; `null` to anyone else.
    // r[impl contact.show]
    async fn email(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<String>> {
        let reads = authorizer(ctx)?
            .reads_contacts()
            .await
            .map_err(|e| unavailable(&e))?;
        if !reads {
            return Ok(None);
        }
        Ok(self.row(ctx).await?.and_then(|r| r.email.clone()))
    }

    /// The user it is linked to, once they signed in.
    async fn user(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<User>> {
        Ok(self
            .row(ctx)
            .await?
            .and_then(|r| r.linked_subject)
            .map(User))
    }

    async fn linked_at(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<DateTime<Utc>>> {
        Ok(self.row(ctx).await?.and_then(|r| r.linked_at))
    }

    /// Who linked it by hand; `null` if a verified email at sign-in did, or
    /// it isn't linked.
    async fn linked_by(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<User>> {
        Ok(self.row(ctx).await?.and_then(|r| r.linked_by).map(User))
    }

    async fn created_by(&self, ctx: &Context<'_>) -> async_graphql::Result<User> {
        Ok(User(self.loaded(ctx).await?.created_by))
    }

    async fn created_at(&self, ctx: &Context<'_>) -> async_graphql::Result<DateTime<Utc>> {
        Ok(self.loaded(ctx).await?.created_at)
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
            .map(Contact::Loaded)
            .collect())
    }

    /// The users who signed in to the tenant, most recently seen first.
    // r[impl person.list]
    async fn users(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<User>> {
        if !authorizer(ctx)?
            .reads_contacts()
            .await
            .map_err(|e| unavailable(&e))?
        {
            return Err(coded("not permitted", "FORBIDDEN"));
        }
        let tenant = tenant(ctx)?;
        let mut conn = connection(ctx).await?;
        Ok(people::in_tenant(&mut conn, tenant)
            .await
            .map_err(internal)?
            .into_iter()
            .map(|(subject, _)| User(subject))
            .collect())
    }
}

#[derive(InputObject)]
pub struct LinkContactInput {
    /// The contact to link.
    contact: ID,
    /// The user it is, who has signed in to the tenant.
    user: ID,
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
            linked_by: None,
        };
        let mut conn = connection(ctx).await?;
        contacts::insert(&mut conn, &row).await.map_err(refused)?;
        Ok(Contact::Loaded(row))
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
        Ok(Contact::Loaded(row))
    }

    /// Links a contact to the user it is, by hand: for people whose email
    /// Konklav never verified, such as those who sign in with a password.
    // r[impl contact.link-by-hand]
    async fn link_contact(
        &self,
        ctx: &Context<'_>,
        input: LinkContactInput,
    ) -> async_graphql::Result<Contact> {
        require(ctx, &Permission::ManageContacts).await?;
        let principal = ctx.data::<Principal>()?;
        let tenant: Uuid = principal.tenant.into();
        let id = contact_id(&input.contact)?;
        let user = Uuid::parse_str(&input.user).map_err(|_| unknown_user(&input.user))?;
        let mut conn = connection(ctx).await?;
        let row = contacts::find(&mut conn, tenant, &[id])
            .await
            .map_err(internal)?
            .pop()
            .ok_or_else(|| coded(format!("there is no contact `{id}`"), "NOT_FOUND"))?;
        if row.linked_subject.is_some() {
            return Err(coded("the contact is already linked to a user", "LINKED"));
        }
        if !people::is_in(&mut conn, tenant, user)
            .await
            .map_err(internal)?
        {
            return Err(unknown_user(&input.user));
        }
        let by = principal.subject.into();
        if !contacts::link_by_hand(&mut conn, tenant, id, user, by, Utc::now())
            .await
            .map_err(internal)?
        {
            // Linked by someone else in the meantime.
            return Err(coded("the contact is already linked to a user", "LINKED"));
        }
        let row = contacts::find(&mut conn, tenant, &[id])
            .await
            .map_err(internal)?
            .pop()
            .ok_or_else(|| internal("a linked contact is gone"))?;
        Ok(Contact::Loaded(row))
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
        Ok(Contact::Loaded(row))
    }
}

fn unknown_user(id: &ID) -> async_graphql::Error {
    coded(
        format!("`{}` hasn't signed in to this tenant", id.0),
        "UNKNOWN_USER",
    )
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
