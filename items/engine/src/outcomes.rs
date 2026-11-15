//! Outcomes: what an akt's verdicts, or its members' outcomes, add up to
//! (`akt.outcome`). Derived each time they are read, never stored
//! (`akt.outcome.derived`).

use std::collections::HashSet;

use kuria_config::{AktType, FieldKind};
use serde::{Deserialize, Serialize};

use crate::Value;
use crate::value::Someone;
use crate::verdicts::{Given, Verdict};

/// An outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
    Skip,
    Pending,
}

impl Outcome {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Skip => "skip",
            Self::Pending => "pending",
        }
    }

    /// The outcome `key` names, as `as_str` gives it.
    #[must_use]
    pub fn parse(key: &str) -> Option<Self> {
        [Self::Pass, Self::Fail, Self::Skip, Self::Pending]
            .into_iter()
            .find(|o| o.as_str() == key)
    }
}

/// A test's outcome from its assignees and the verdicts they gave: `fail`
/// if any failed it; otherwise `pending` while an assignee hasn't answered,
/// or it has none; otherwise `skip` if all skipped it; otherwise `pass`.
// r[impl akt.outcome]
#[must_use]
pub fn of_verdicts(assignees: &[Someone], given: &[Given]) -> Outcome {
    let verdict_of = |a: &Someone| given.iter().find(|g| g.assignee == *a).map(|g| g.verdict);
    let verdicts: Vec<Option<Verdict>> = assignees.iter().map(verdict_of).collect();
    if verdicts.contains(&Some(Verdict::Fail)) {
        Outcome::Fail
    } else if verdicts.is_empty() || verdicts.contains(&None) {
        Outcome::Pending
    } else if verdicts.iter().all(|v| *v == Some(Verdict::Skip)) {
        Outcome::Skip
    } else {
        Outcome::Pass
    }
}

/// What the outcomes of what a container or section holds add up to:
/// `fail` if any fails; otherwise `pending` if any is pending, or it holds
/// none; otherwise `skip` if all skip; otherwise `pass`.
// r[impl akt.outcome]
// r[impl akt.outcome.section]
pub fn of_all(outcomes: impl IntoIterator<Item = Outcome>) -> Outcome {
    let outcomes: Vec<Outcome> = outcomes.into_iter().collect();
    if outcomes.contains(&Outcome::Fail) {
        Outcome::Fail
    } else if outcomes.is_empty() || outcomes.contains(&Outcome::Pending) {
        Outcome::Pending
    } else if outcomes.iter().all(|o| *o == Outcome::Skip) {
        Outcome::Skip
    } else {
        Outcome::Pass
    }
}

/// The outcome of an akt of `ty` with `fields` that has a `verdict` field,
/// from its own verdicts; `None` if `ty` has no `verdict` field.
#[must_use]
pub fn of_fields(
    ty: &AktType,
    fields: &std::collections::BTreeMap<String, Value>,
) -> Option<Outcome> {
    let field = ty
        .document()
        .fields
        .iter()
        .find(|f| f.kind == FieldKind::Verdict)?;
    let assignees = match field.assignees.as_ref().and_then(|key| fields.get(key)) {
        Some(Value::Users(people)) => people.clone(),
        _ => Vec::new(),
    };
    let given = match fields.get(&field.key) {
        Some(Value::Verdicts(given)) => given.clone(),
        _ => Vec::new(),
    };
    Some(of_verdicts(&assignees, &given))
}

/// Whether akter of the type `key` have outcomes: it has a `verdict` field,
/// or contains a type that has outcomes. `types` gives a type by key.
// r[impl akt.outcome]
pub fn has_outcomes<'a>(key: &str, types: &dyn Fn(&str) -> Option<&'a AktType>) -> bool {
    fn walk<'a>(
        key: &str,
        types: &dyn Fn(&str) -> Option<&'a AktType>,
        seen: &mut HashSet<String>,
    ) -> bool {
        if !seen.insert(key.to_owned()) {
            return false;
        }
        let Some(ty) = types(key) else { return false };
        ty.document()
            .fields
            .iter()
            .any(|f| f.kind == FieldKind::Verdict)
            || ty
                .document()
                .contains
                .iter()
                .any(|c| walk(&c.akt_type, types, seen))
    }
    walk(key, types, &mut HashSet::new())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    use super::*;

    fn person(n: u128) -> Someone {
        Someone::User(Uuid::from_u128(n))
    }

    fn gave(n: u128, verdict: Verdict) -> Given {
        Given {
            assignee: person(n),
            verdict,
            by: Uuid::from_u128(n),
            at: DateTime::<Utc>::UNIX_EPOCH,
        }
    }

    // r[verify akt.outcome]
    #[test]
    fn a_test_s_outcome_follows_its_assignees_verdicts() {
        use Outcome::{Fail, Pass, Pending, Skip};
        use Verdict as V;
        let (a, b) = (person(1), person(2));
        let both = [a, b];
        let cases: [(&[Someone], Vec<Given>, Outcome); 9] = [
            (&[], vec![], Pending),
            (&both, vec![], Pending),
            (&both, vec![gave(1, V::Pass)], Pending),
            (&both, vec![gave(1, V::Fail)], Fail),
            (&both, vec![gave(1, V::Pass), gave(2, V::Pass)], Pass),
            (&both, vec![gave(1, V::Pass), gave(2, V::Skip)], Pass),
            (&both, vec![gave(1, V::Skip), gave(2, V::Skip)], Skip),
            (&both, vec![gave(1, V::Skip), gave(2, V::Fail)], Fail),
            // A verdict from someone no longer assigned doesn't count.
            (&[a], vec![gave(1, V::Skip), gave(2, V::Fail)], Skip),
        ];
        for (assignees, given, expected) in cases {
            assert_eq!(of_verdicts(assignees, &given), expected, "{given:?}");
        }
    }

    fn ty(document: serde_json::Value) -> AktType {
        kuria_config::validate(serde_json::from_value(document).unwrap()).unwrap()
    }

    // r[verify akt.outcome]
    #[test]
    fn types_have_outcomes_through_verdicts_or_what_they_contain() {
        let states = serde_json::json!([{ "key": "open", "name": "Open", "initial": true }]);
        let test = ty(
            serde_json::json!({ "key": "test", "name": "Test", "number": "T-{seq}", "states": states,
            "fields": [{ "key": "testers", "name": "Testers", "kind": "user", "multiple": true },
                       { "key": "verdict", "name": "Verdict", "kind": "verdict", "assignees": "testers" }] }),
        );
        let protocol = ty(
            serde_json::json!({ "key": "protocol", "name": "Protocol", "number": "P-{seq}",
            "states": states, "fields": [], "contains": [{ "type": "test" }, { "type": "protocol" }] }),
        );
        let note = ty(
            serde_json::json!({ "key": "note", "name": "Note", "number": "N-{seq}", "states": states, "fields": [] }),
        );
        let types = |key: &str| {
            [&test, &protocol, &note]
                .into_iter()
                .find(|t| t.key() == key)
        };
        assert!(has_outcomes("test", &types));
        assert!(
            has_outcomes("protocol", &types),
            "it contains tests, and itself"
        );
        assert!(!has_outcomes("note", &types));
        assert!(!has_outcomes("missing", &types));
    }

    // r[verify akt.outcome]
    // r[verify akt.outcome.section]
    #[test]
    fn a_container_s_outcome_follows_what_it_holds() {
        use Outcome::{Fail, Pass, Pending, Skip};
        assert_eq!(of_all([]), Pending, "holding nothing, nothing has passed");
        assert_eq!(of_all([Pass, Skip, Pass]), Pass);
        assert_eq!(of_all([Skip, Skip]), Skip);
        assert_eq!(of_all([Pass, Pending]), Pending);
        assert_eq!(of_all([Pending, Fail, Pass]), Fail);
        // Nested: a section of sections.
        assert_eq!(of_all([of_all([Pass, Skip]), of_all([Skip])]), Pass);
    }
}
