//! The outcome filter's SQL against the engine's rules (`akt.filter.outcome`).

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use kuria_engine::outcomes::{Outcome, of_verdicts};
use kuria_engine::verdicts::{Given, Verdict};
use kuria_engine::{Someone, Value, fields_to_json};
use kuria_store::akter::{self, AktRow, Condition, Listing, Match};
use kuria_test_support::TestDatabase;
use serde_json::json;
use uuid::Uuid;

const TENANT: Uuid = Uuid::from_u128(1);
const SUBJECT: Uuid = Uuid::from_u128(3);

fn gave(assignee: Someone, verdict: Verdict) -> Given {
    Given {
        assignee,
        verdict,
        by: SUBJECT,
        at: DateTime::<Utc>::UNIX_EPOCH,
    }
}

// r[verify akt.filter.outcome]
// r[verify akt.outcome]
#[tokio::test(flavor = "multi_thread")]
async fn the_outcome_filter_agrees_with_the_engine() {
    use Verdict::{Fail, Pass, Skip};
    let db = TestDatabase::new().await;
    let mut conn = db.pool().get().await.unwrap();
    let ty = akter::insert_type_version(&mut conn, TENANT, "test", &json!({}), SUBJECT, Utc::now())
        .await
        .unwrap()
        .id;
    let a = Someone::User(Uuid::from_u128(10));
    let b = Someone::Contact(Uuid::from_u128(11));
    let both = vec![a, b];
    // The engine's table, a contact among the assignees.
    let cases: Vec<(Option<Vec<Someone>>, Vec<Given>)> = vec![
        (None, vec![]),
        (Some(vec![]), vec![]),
        (Some(both.clone()), vec![]),
        (Some(both.clone()), vec![gave(a, Pass)]),
        (Some(both.clone()), vec![gave(a, Fail)]),
        (Some(both.clone()), vec![gave(a, Pass), gave(b, Pass)]),
        (Some(both.clone()), vec![gave(a, Pass), gave(b, Skip)]),
        (Some(both.clone()), vec![gave(a, Skip), gave(b, Skip)]),
        (Some(both.clone()), vec![gave(a, Skip), gave(b, Fail)]),
        // A verdict from someone no longer assigned doesn't count.
        (Some(vec![a]), vec![gave(a, Skip), gave(b, Fail)]),
        (Some(vec![b]), vec![gave(b, Pass)]),
    ];
    let mut expected: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (i, (assignees, given)) in cases.iter().enumerate() {
        let number = format!("T-{i}");
        let mut fields = BTreeMap::new();
        if let Some(people) = assignees {
            fields.insert("testers".to_owned(), Value::Users(people.clone()));
        }
        if !given.is_empty() {
            fields.insert("verdict".to_owned(), Value::Verdicts(given.clone()));
        }
        let outcome = of_verdicts(assignees.as_deref().unwrap_or_default(), given);
        expected
            .entry(outcome.as_str())
            .or_default()
            .insert(number.clone());
        let row = AktRow {
            id: Uuid::now_v7(),
            tenant_id: TENANT,
            number,
            type_version_id: ty,
            state: "open".to_owned(),
            fields: serde_json::Value::Object(fields_to_json(&fields)),
            revision: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            rank: format!("a{i:02}"),
            title: None,
        };
        akter::insert_akt(&mut conn, &row).await.unwrap();
    }
    let keys = ["test".to_owned()];
    for outcome in [
        Outcome::Pass,
        Outcome::Fail,
        Outcome::Skip,
        Outcome::Pending,
    ] {
        let conditions = [Condition(vec![Match::Outcome {
            verdict: "verdict".to_owned(),
            assignees: Some("testers".to_owned()),
            outcome: outcome.as_str().to_owned(),
        }])];
        let listing = Listing {
            keys: &keys,
            state: None,
            conditions: &conditions,
            after: None,
            limit: 100,
        };
        let found: BTreeSet<String> = akter::list_akter(&mut conn, TENANT, &listing)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.number)
            .collect();
        assert_eq!(
            found,
            expected.get(outcome.as_str()).cloned().unwrap_or_default(),
            "{outcome:?}"
        );
    }
    assert_eq!(expected.len(), 4, "every outcome is among the cases");
}
