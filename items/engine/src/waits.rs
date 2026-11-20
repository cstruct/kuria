//! What a transition waits for: its akt's members of some types, and the
//! targets of some of its link kinds, each in a final state, and at least
//! one link of each kind it requires (`akt.transition.members`,
//! `akt.transition.links`).
//!
//! No I/O: the caller finds the members and links, with whether each is
//! final, and passes them in.

use kuria_config::TransitionDef;

/// A member of the akt, as a waiting transition sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub number: String,
    /// Its akt type's key.
    pub akt_type: String,
    /// Whether it is in a final state of its akt type version.
    pub is_final: bool,
}

/// A link from the akt, as a waiting transition sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Linked {
    /// The link kind's key.
    pub kind: String,
    /// The target's number.
    pub number: String,
    /// Whether the target is in a final state of its akt type version.
    pub is_final: bool,
}

/// What a transition still waits for; nothing if both are empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Waits {
    /// The members and link targets not yet final, by number, each once,
    /// in the order found.
    pub open: Vec<String>,
    /// The link kinds it requires and the akt has no link of, by key.
    pub lacking: Vec<String>,
}

impl Waits {
    /// Whether it waits for nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.open.is_empty() && self.lacking.is_empty()
    }
}

/// Whether `def` waits for anything at all, so that a caller need find
/// members and links only then.
#[must_use]
pub fn waits_at_all(def: &TransitionDef) -> bool {
    !def.waits_for.is_empty() || !def.requires_links.is_empty()
}

/// What `def` waits for, given the akt's `members` and its outgoing
/// `links`.
// r[impl akt.transition.members]
// r[impl akt.transition.links]
#[must_use]
pub fn of(def: &TransitionDef, members: &[Member], links: &[Linked]) -> Waits {
    let mut open: Vec<String> = Vec::new();
    let mut add = |number: &str| {
        if !open.iter().any(|n| n == number) {
            open.push(number.to_owned());
        }
    };
    for member in members {
        if !member.is_final && def.waits_for.members.contains(&member.akt_type) {
            add(&member.number);
        }
    }
    for link in links {
        if !link.is_final && def.waits_for.links.contains(&link.kind) {
            add(&link.number);
        }
    }
    let lacking = def
        .requires_links
        .iter()
        .filter(|kind| !links.iter().any(|l| &l.kind == *kind))
        .cloned()
        .collect();
    Waits { open, lacking }
}
