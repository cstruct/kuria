//! The commands that read (`cli.read`): what a tenant has (`types`,
//! `schema`, `users`, `contacts`), lists of akter (`ls`, `find`), and one
//! akt (`cat`, `show`, `history`).

mod akter;
mod catalog;
mod list;
mod search;

pub use akter::{WithMembers, cat, history, show};
pub(crate) use akter::{akt_data, members_of};
pub use catalog::{contacts, schema, types, users};
pub use list::{Listing, find, ls};
pub use search::{Searching, search};

use serde_json::json;

use crate::error::CliError;
use crate::local::{self, AktDir, git::Times};
use crate::model::{SHAPE, Shape, read};
use crate::session::Session;
use crate::values::People;

/// Which akter a list answers (`cli.archive`, `archive.read`): live ones,
/// the default, archived ones only, or both.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Archived {
    #[default]
    Live,
    Archived,
    All,
}

impl Archived {
    /// As `--archived` and `--with-archived` ask.
    #[must_use]
    pub fn asked(archived: bool, with_archived: bool) -> Self {
        match (archived, with_archived) {
            (true, _) => Self::Archived,
            (false, true) => Self::All,
            (false, false) => Self::Live,
        }
    }

    /// As GraphQL's `ArchivedFilter` names it.
    #[must_use]
    pub fn filter(self) -> &'static str {
        match self {
            Self::Live => "LIVE",
            Self::Archived => "ARCHIVED",
            Self::All => "ALL",
        }
    }

    /// Whether an akt, archived or not as `archived` says, is answered.
    #[must_use]
    pub fn allows(self, archived: bool) -> bool {
        match self {
            Self::Live => !archived,
            Self::Archived => archived,
            Self::All => true,
        }
    }
}

/// Where reads read: a Kuria server, or an akt directory (`cli.local.dir`).
#[derive(Clone, Copy)]
pub enum Source<'a> {
    Server(&'a Session),
    Local(&'a Local),
}

/// An akt directory, loaded, with when its files changed and who `@me` is.
pub struct Local {
    pub dir: AktDir,
    pub times: Times,
    /// `KURIA_ME`, else git's `user.email` (`cli.local.people`).
    pub me: Option<String>,
}

impl Local {
    /// The akt directory at `root`.
    pub fn open(root: &std::path::Path, me: Option<String>) -> Result<Self, CliError> {
        let mut dir = AktDir::open(root)?;
        let times = Times::of(root);
        dir.load_archived(&times);
        Ok(Self {
            times,
            me: me.or_else(|| local::git::email(root)),
            dir,
        })
    }

    /// Who `@me` is here.
    pub fn me(&self) -> Result<&str, CliError> {
        self.me.as_deref().ok_or_else(|| {
            CliError::refused(
                "BAD_REQUEST",
                "`@me` is no one here: set KURIA_ME, or git's user.email",
            )
        })
    }
}

/// The latest version of the akt type `key`, or `UNKNOWN_AKT_TYPE`.
pub async fn shape(source: Source<'_>, key: &str) -> Result<Shape, CliError> {
    let session = match source {
        Source::Server(session) => session,
        Source::Local(local) => {
            return match local.dir.types.get(key) {
                Some(t) => read(local::answers::shape(t), "the akt type"),
                None => Err(unknown_type(key)),
            };
        }
    };
    let data = session
        .graphql(
            &format!("query($k: String!) {{ aktType(key: $k) {{ ...Shape document }} }} {SHAPE}"),
            json!({ "k": key }),
        )
        .await?;
    if data["aktType"].is_null() {
        return Err(unknown_type(key));
    }
    read(data["aktType"].clone(), "the akt type")
}

fn unknown_type(key: &str) -> CliError {
    CliError::from_server(
        "UNKNOWN_AKT_TYPE",
        format!("No akt type `{key}`; see `kuria types`"),
        None,
    )
}

/// The tenant's users, and its contacts if the session may list them.
pub async fn people(source: Source<'_>) -> Result<People, CliError> {
    let session = match source {
        Source::Server(session) => session,
        Source::Local(local) => {
            return Ok(People {
                me: local.me.clone(),
                ..People::default()
            });
        }
    };
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
    let viewer = session.graphql("{ viewer { subject } }", json!({})).await?;
    Ok(People {
        users: read(users["users"].clone(), "users")?,
        contacts,
        me: viewer["viewer"]["subject"].as_str().map(str::to_owned),
    })
}
