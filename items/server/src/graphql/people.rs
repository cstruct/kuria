//! Users, shown by the names Kuria recorded at sign-in.

use async_graphql::{Context, ID, Object};
use kuria_engine::Someone as Named;
use kuria_store::people::{self, Person};
use uuid::Uuid;

use crate::akter::Akter;

/// A person or piece of software, as Konklav knows them.
pub struct User(pub Uuid);

impl User {
    async fn person(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<Person>> {
        let mut conn = ctx.data::<Akter>()?.pool().get().await?;
        Ok(people::find(&mut conn, &[self.0]).await?.pop())
    }
}

#[Object]
impl User {
    /// The Konklav subject identifier.
    async fn id(&self) -> ID {
        ID(self.0.to_string())
    }

    /// The name recorded at their last browser sign-in, else their email,
    /// else their identifier.
    // r[impl person.name]
    async fn name(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        Ok(display_name(self.0, self.person(ctx).await?.as_ref()))
    }

    /// The email recorded at their last browser sign-in.
    async fn email(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<String>> {
        Ok(self.person(ctx).await?.and_then(|p| p.email))
    }
}

/// A user or a contact, as a value names them.
#[derive(async_graphql::Union)]
pub enum Someone {
    User(User),
    Contact(super::contacts::Contact),
}

impl From<kuria_engine::Someone> for Someone {
    fn from(someone: kuria_engine::Someone) -> Self {
        match someone {
            kuria_engine::Someone::User(subject) => Self::User(User(subject)),
            kuria_engine::Someone::Contact(id) => {
                Self::Contact(super::contacts::Contact::Named(id))
            }
        }
    }
}

/// How the contact `contact` is shown: as the user it is linked to, if
/// Kuria recorded their name or email, else by its own name, else by its
/// identifier.
// r[impl contact.show]
pub async fn contact_name(
    conn: &mut kuria_store::Connection,
    contact: &kuria_store::contacts::Contact,
) -> diesel::QueryResult<String> {
    if let Some(subject) = contact.linked_subject {
        let person = people::find(conn, &[subject]).await?.pop();
        if person
            .as_ref()
            .is_some_and(|p| p.name.is_some() || p.email.is_some())
        {
            return Ok(display_name(subject, person.as_ref()));
        }
    }
    Ok(contact
        .name
        .clone()
        .unwrap_or_else(|| contact.id.to_string()))
}

/// How `someone` is shown: a user as `display_name` shows them, a contact
/// as `contact_name` does, and a contact the tenant doesn't have by its
/// identifier. Values keep naming a contact once it is linked; it is here,
/// when they are shown, that the link is followed.
// r[impl contact.values]
pub async fn someone_name(
    conn: &mut kuria_store::Connection,
    tenant: Uuid,
    someone: Named,
) -> diesel::QueryResult<String> {
    match someone {
        Named::User(subject) => {
            let person = people::find(conn, &[subject]).await?.pop();
            Ok(display_name(subject, person.as_ref()))
        }
        Named::Contact(id) => {
            match kuria_store::contacts::find(conn, tenant, &[id])
                .await?
                .pop()
            {
                Some(contact) => contact_name(conn, &contact).await,
                None => Ok(id.to_string()),
            }
        }
    }
}

/// How `subject` is shown: by name, else email, else identifier.
// r[impl person.name]
pub fn display_name(subject: Uuid, person: Option<&Person>) -> String {
    person
        .and_then(|p| p.name.clone().or_else(|| p.email.clone()))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| subject.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn person(name: Option<&str>, email: Option<&str>) -> Person {
        Person {
            subject_id: Uuid::nil(),
            name: name.map(str::to_owned),
            email: email.map(str::to_owned),
            updated_at: Utc::now(),
        }
    }

    // r[verify person.name]
    #[test]
    fn falls_back_from_name_to_email_to_identifier() {
        let id = Uuid::nil();
        assert_eq!(
            display_name(id, Some(&person(Some("Bob"), Some("bob@x.example")))),
            "Bob"
        );
        assert_eq!(
            display_name(id, Some(&person(None, Some("bob@x.example")))),
            "bob@x.example"
        );
        assert_eq!(
            display_name(id, Some(&person(Some(" "), None))),
            id.to_string()
        );
        assert_eq!(display_name(id, None), id.to_string());
    }
}
