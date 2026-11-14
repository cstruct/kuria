//! Copying an akt with part of its member tree (`akt.copy`): which akter a
//! copy keeps, and which values it leaves behind. The stores walk the tree
//! and make the copies.

use std::collections::{HashMap, HashSet};

use kuria_config::FieldKind;

/// The akter of `tree` a copy keeps: all of them, or those `only` names,
/// those between them and the root, and every member under them. `tree`
/// lists each akt by number with its parent's, the root first and each
/// parent before its members. Naming an akt outside the tree is refused.
// r[impl akt.copy.only]
pub fn kept(
    tree: &[(&str, Option<&str>)],
    only: Option<&[String]>,
) -> Result<HashSet<String>, String> {
    let Some(only) = only else {
        return Ok(tree.iter().map(|(n, _)| (*n).to_owned()).collect());
    };
    let Some((root, _)) = tree.first() else {
        return Ok(HashSet::new());
    };
    let parent_of: HashMap<&str, Option<&str>> = tree.iter().copied().collect();
    let mut kept = HashSet::from([(*root).to_owned()]);
    let mut named: HashSet<&str> = HashSet::new();
    for number in only {
        if !parent_of.contains_key(number.as_str()) {
            return Err(format!("`{number}` isn't in the tree of {root}"));
        }
        named.insert(number.as_str());
        let mut up = Some(number.as_str());
        while let Some(n) = up {
            kept.insert(n.to_owned());
            up = parent_of[n];
        }
    }
    // Every member under a named one, parents being listed first.
    for (number, parent) in tree {
        if parent.is_some_and(|p| named.contains(p)) {
            named.insert(number);
            kept.insert((*number).to_owned());
        }
    }
    Ok(kept)
}

/// Whether a copy keeps a value of `kind`: not verdicts and checkboxes,
/// records of what someone did, nor sections, which are set once the
/// copy's members exist.
#[must_use]
pub fn keeps(kind: FieldKind) -> bool {
    !matches!(
        kind,
        FieldKind::Verdict | FieldKind::Checkbox | FieldKind::Sections
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify akt.copy.only]
    #[test]
    fn a_copy_keeps_what_only_names_with_its_path_and_its_members() {
        let tree = [
            ("P", None),
            ("S1", Some("P")),
            ("S2", Some("P")),
            ("T1", Some("S1")),
            ("T2", Some("S1")),
            ("T3", Some("S2")),
        ];
        let all = kept(&tree, None).unwrap();
        assert_eq!(all.len(), 6);
        let some = kept(&tree, Some(&["S1".to_owned()])).unwrap();
        let mut some: Vec<&str> = some.iter().map(String::as_str).collect();
        some.sort_unstable();
        assert_eq!(some, ["P", "S1", "T1", "T2"]);
        let one = kept(&tree, Some(&["T3".to_owned()])).unwrap();
        let mut one: Vec<&str> = one.iter().map(String::as_str).collect();
        one.sort_unstable();
        assert_eq!(one, ["P", "S2", "T3"]);
        assert!(kept(&tree, Some(&["X".to_owned()])).is_err());
    }
}
