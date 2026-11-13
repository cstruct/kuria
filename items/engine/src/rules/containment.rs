//! Containers: which akter one may be created in, and which members it
//! may gain (`container.*`).

use std::collections::{BTreeSet, VecDeque};

use kuria_config::AktType;

use super::{AktRef, Broken};

/// Refuses an akt type with `created_in` created anywhere but in one of
/// those types: its parent, the first container, decides.
// r[impl container.created-in]
pub fn check_created_in(ty: &AktType, parents: &[&AktType]) -> Result<(), Broken> {
    let allowed = ty.created_in();
    if allowed.is_empty() {
        return Ok(());
    }
    match parents.first() {
        Some(parent) if allowed.iter().any(|a| a == parent.key()) => Ok(()),
        _ => Err(Broken::CreatedIn {
            akt_type: ty.key().to_owned(),
            allowed: allowed.to_vec(),
        }),
    }
}

/// Refuses naming one container twice for a new akt.
pub fn check_distinct(containers: &[&str]) -> Result<(), Broken> {
    let mut seen = BTreeSet::new();
    match containers.iter().find(|n| !seen.insert(**n)) {
        Some(twice) => Err(Broken::AlreadyMember {
            container: (*twice).to_owned(),
            member: "the new akt".to_owned(),
        }),
        None => Ok(()),
    }
}

/// Refuses containers a new akt of `ty` can't be created in: of a type that
/// doesn't contain it, frozen, not accepting it in their state, or two of
/// one exclusive type.
// r[impl container.contains]
// r[impl container.frozen]
// r[impl container.while]
// r[impl container.exclusive]
pub fn check_create_inside(ty: &AktType, parents: &[AktRef<'_>]) -> Result<(), Broken> {
    let mut exclusive: Vec<(&str, &str)> = Vec::new();
    for parent in parents {
        let Some(contains) = parent.ty.contains(ty.key()) else {
            return Err(Broken::NotContainable {
                container: parent.number.to_owned(),
                member: ty.key().to_owned(),
            });
        };
        check_unfrozen(parent)?;
        if !contains.accepts_in(parent.state) {
            return Err(Broken::NotAccepting {
                container: parent.number.to_owned(),
                member_type: ty.key().to_owned(),
                state: parent.state.to_owned(),
            });
        }
        if contains.exclusive {
            let key = parent.ty.key();
            if let Some((_, other)) = exclusive.iter().find(|(k, _)| *k == key) {
                return Err(Broken::Exclusive {
                    member: "the new akt".to_owned(),
                    other: (*other).to_owned(),
                    akt_type: key.to_owned(),
                });
            }
            exclusive.push((key, parent.number));
        }
    }
    Ok(())
}

/// Refuses changing the members of a container in a final state.
// r[impl container.frozen]
pub fn check_unfrozen(container: &AktRef<'_>) -> Result<(), Broken> {
    if container.is_final() {
        return Err(Broken::Frozen(container.number.to_owned()));
    }
    Ok(())
}

/// Refuses an akt named as its own container.
// r[impl container.acyclic]
pub fn check_not_itself(containers: &[&str], member: &str) -> Result<(), Broken> {
    match containers.iter().find(|c| **c == member) {
        Some(container) => Err(Broken::Cycle {
            container: (*container).to_owned(),
            member: member.to_owned(),
        }),
        None => Ok(()),
    }
}

/// What a store knows about a container and a would-be member.
#[derive(Clone, Copy, Debug, Default)]
pub struct Joining<'a> {
    /// The member is already in the container.
    pub already: bool,
    /// Another container of the container's type that the member is in,
    /// leaving out one it is leaving in the same change.
    pub other_of_type: Option<&'a str>,
    /// The member contains the container, directly or through others.
    pub contains_container: bool,
}

/// Refuses what would stop `container` gaining `member`: a type it doesn't
/// contain, a membership already there, a state it doesn't accept that type
/// in, exclusivity, and cycles, in that order.
// r[impl container.contains]
// r[impl container.while]
// r[impl container.exclusive]
// r[impl container.acyclic]
pub fn check_joining(
    container: &AktRef<'_>,
    member: &AktRef<'_>,
    known: Joining<'_>,
) -> Result<(), Broken> {
    let Some(contains) = container.ty.contains(member.ty.key()) else {
        return Err(Broken::NotContainable {
            container: container.number.to_owned(),
            member: member.ty.key().to_owned(),
        });
    };
    if known.already {
        return Err(Broken::AlreadyMember {
            container: container.number.to_owned(),
            member: member.number.to_owned(),
        });
    }
    if !contains.accepts_in(container.state) {
        return Err(Broken::NotAccepting {
            container: container.number.to_owned(),
            member_type: member.ty.key().to_owned(),
            state: container.state.to_owned(),
        });
    }
    if contains.exclusive
        && let Some(other) = known.other_of_type
    {
        return Err(Broken::Exclusive {
            member: member.number.to_owned(),
            other: other.to_owned(),
            akt_type: container.ty.key().to_owned(),
        });
    }
    if known.contains_container {
        return Err(Broken::Cycle {
            container: container.number.to_owned(),
            member: member.number.to_owned(),
        });
    }
    Ok(())
}

/// Whether `from` contains `to`, directly or through its members' members,
/// given each akt's members: what [`Joining::contains_container`] asks, for
/// a store without a recursive query.
#[must_use]
pub fn contains(from: &str, to: &str, mut members_of: impl FnMut(&str) -> Vec<String>) -> bool {
    let mut seen = BTreeSet::new();
    let mut next = VecDeque::from([from.to_owned()]);
    while let Some(akt) = next.pop_front() {
        for member in members_of(&akt) {
            if member == to {
                return true;
            }
            if seen.insert(member.clone()) {
                next.push_back(member);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use kuria_config::{AktTypeDocument, validate};
    use serde_json::{Value, json};

    use super::*;

    fn ty(document: Value) -> AktType {
        validate(serde_json::from_value::<AktTypeDocument>(document).unwrap()).unwrap()
    }

    fn backlog() -> AktType {
        ty(json!({
            "key": "backlog", "name": "Backlog", "number": "B-{seq}", "fields": [],
            "states": [
                { "key": "open", "name": "Open", "initial": true },
                { "key": "frozen", "name": "Frozen" },
                { "key": "closed", "name": "Closed", "final": true }
            ],
            "contains": [{ "type": "item", "exclusive": true, "while": ["open"] }]
        }))
    }

    fn item() -> AktType {
        ty(json!({
            "key": "item", "name": "Item", "number": "I-{seq}", "fields": [],
            "states": [{ "key": "open", "name": "Open", "initial": true }],
            "created_in": ["backlog"]
        }))
    }

    fn at<'a>(number: &'a str, state: &'a str, ty: &'a AktType) -> AktRef<'a> {
        AktRef {
            number,
            state,
            ty,
            revision: 1,
        }
    }

    // r[verify container.created-in]
    // r[verify container.exclusive]
    // r[verify container.frozen]
    // r[verify container.while]
    #[test]
    fn checks_where_an_akt_is_created() {
        let (backlog, item) = (backlog(), item());
        assert!(check_created_in(&item, &[&backlog]).is_ok());
        assert!(matches!(
            check_created_in(&item, &[]),
            Err(Broken::CreatedIn { .. })
        ));
        assert!(check_create_inside(&item, &[at("B-1", "open", &backlog)]).is_ok());
        let two = [at("B-1", "open", &backlog), at("B-2", "open", &backlog)];
        assert!(
            matches!(check_create_inside(&item, &two), Err(Broken::Exclusive { other, .. }) if other == "B-1")
        );
        assert_eq!(
            check_create_inside(&item, &[at("B-1", "closed", &backlog)]),
            Err(Broken::Frozen("B-1".to_owned()))
        );
        assert!(matches!(
            check_create_inside(&item, &[at("B-1", "frozen", &backlog)]),
            Err(Broken::NotAccepting { .. })
        ));
        assert!(matches!(
            check_create_inside(&backlog, &[at("I-1", "open", &item)]),
            Err(Broken::NotContainable { .. })
        ));
        assert!(matches!(
            check_distinct(&["B-1", "B-1"]),
            Err(Broken::AlreadyMember { .. })
        ));
    }

    // r[verify container.contains]
    // r[verify container.acyclic]
    #[test]
    fn checks_a_member_joining() {
        let (backlog, item) = (backlog(), item());
        let (c, m) = (at("B-1", "open", &backlog), at("I-1", "open", &item));
        assert!(check_joining(&c, &m, Joining::default()).is_ok());
        let already = Joining {
            already: true,
            ..Joining::default()
        };
        assert!(matches!(
            check_joining(&c, &m, already),
            Err(Broken::AlreadyMember { .. })
        ));
        let elsewhere = Joining {
            other_of_type: Some("B-2"),
            ..Joining::default()
        };
        assert!(matches!(
            check_joining(&c, &m, elsewhere),
            Err(Broken::Exclusive { .. })
        ));
        let cycle = Joining {
            contains_container: true,
            ..Joining::default()
        };
        assert!(matches!(
            check_joining(&c, &m, cycle),
            Err(Broken::Cycle { .. })
        ));
        assert!(matches!(
            check_not_itself(&["B-1"], "B-1"),
            Err(Broken::Cycle { .. })
        ));
        // Rules apply in their order: not containable comes first.
        assert!(matches!(
            check_joining(&m, &c, already),
            Err(Broken::NotContainable { .. })
        ));
    }

    #[test]
    fn finds_containment_through_members() {
        let members = |akt: &str| match akt {
            "A" => vec!["B".to_owned()],
            "B" => vec!["C".to_owned(), "A".to_owned()],
            _ => Vec::new(),
        };
        assert!(contains("A", "C", members));
        assert!(!contains("C", "A", members));
    }
}
