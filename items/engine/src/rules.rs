//! The rules for numbering akter, containing them in others, linking and
//! ordering them: what the server's write path and the CLI's local mode
//! both enforce (`cli.local.rules`).
//!
//! No I/O: what a rule needs to know about other akter (whether a
//! membership exists, which containers an akt is in) the caller finds and
//! passes in.
//!
//! - [`numbering`]: counter scopes and akt numbers.
//! - [`containment`]: creating inside containers, and gaining members.
//! - [`links`]: link kinds, targets and pins.
//! - [`records`]: what membership and link revisions record.

pub mod containment;
pub mod links;
pub mod numbering;
pub mod records;

use kuria_config::AktType;

/// An akt as the rules see it: its number, state and type, and its latest
/// revision.
#[derive(Clone, Copy, Debug)]
pub struct AktRef<'a> {
    pub number: &'a str,
    pub state: &'a str,
    pub ty: &'a AktType,
    pub revision: i32,
}

impl AktRef<'_> {
    /// Whether it is in one of its type's final states.
    #[must_use]
    pub fn is_final(&self) -> bool {
        self.ty.state(self.state).is_some_and(|s| s.is_final)
    }
}

/// Why a containment, link or ordering rule refused a change. Each is one
/// of the codes the server answers.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Broken {
    #[error("`{container}` can't contain akter of the type `{member}`")]
    NotContainable { container: String, member: String },
    #[error("`{member}` is already a member of `{container}`")]
    AlreadyMember { container: String, member: String },
    #[error("`{member}` is not a member of `{container}`")]
    NotMember { container: String, member: String },
    #[error("`{member}` is already in `{other}`, and can be in only one `{akt_type}`")]
    Exclusive {
        member: String,
        other: String,
        akt_type: String,
    },
    #[error("`{member}` contains `{container}`, so it can't also be in it")]
    Cycle { container: String, member: String },
    #[error("`{0}` is in a final state, so its members can't change")]
    Frozen(String),
    #[error("`{container}` doesn't take new `{member_type}` akter while `{state}`")]
    NotAccepting {
        container: String,
        member_type: String,
        state: String,
    },
    #[error("an akt of the type `{akt_type}` must be created in one of: {}", .allowed.join(", "))]
    CreatedIn {
        akt_type: String,
        allowed: Vec<String>,
    },
    #[error("{0}")]
    InvalidLink(String),
    #[error("`{source_number}` has no `{kind}` link to `{target}`")]
    NotLinked {
        source_number: String,
        kind: String,
        target: String,
    },
    #[error("an akt can't be moved next to itself")]
    SelfMove,
}
