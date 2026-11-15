//! Repositories' refs: moved only by compare-and-swap (`git.repo.storage`).

use chrono::Utc;
use kuria_store::akter::{self, AktRow};
use kuria_store::git;
use kuria_test_support::TestDatabase;
use serde_json::json;
use uuid::Uuid;

const TENANT: Uuid = Uuid::from_u128(1);
const SUBJECT: Uuid = Uuid::from_u128(3);

// r[verify git.repo.storage]
#[tokio::test(flavor = "multi_thread")]
async fn a_ref_moves_only_from_where_it_is() {
    let db = TestDatabase::new().await;
    let mut conn = db.pool().get().await.unwrap();
    let ty =
        akter::insert_type_version(&mut conn, TENANT, "index", &json!({}), SUBJECT, Utc::now())
            .await
            .unwrap()
            .id;
    let row = AktRow {
        id: Uuid::now_v7(),
        tenant_id: TENANT,
        number: "D-1".to_owned(),
        type_version_id: ty,
        state: "open".to_owned(),
        fields: json!({}),
        revision: 1,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        rank: "a0".to_owned(),
        title: None,
    };
    akter::insert_akt(&mut conn, &row).await.unwrap();
    let main = "refs/heads/main";
    let moved = async |conn: &mut kuria_store::Connection, old, new| {
        git::move_ref(conn, TENANT, row.id, main, old, new)
            .await
            .unwrap()
    };
    assert!(moved(&mut conn, None, Some("a")).await);
    // Created already: a second creation, as by a racing push, is refused.
    assert!(!moved(&mut conn, None, Some("b")).await);
    assert!(!moved(&mut conn, Some("b"), Some("c")).await);
    assert!(moved(&mut conn, Some("a"), Some("c")).await);
    assert_eq!(
        git::refs(&mut conn, TENANT, row.id).await.unwrap(),
        [(main.to_owned(), "c".to_owned())]
    );
    assert!(!moved(&mut conn, Some("a"), None).await);
    assert!(moved(&mut conn, Some("c"), None).await);
    assert!(moved(&mut conn, None, None).await);
    assert_eq!(git::refs(&mut conn, TENANT, row.id).await.unwrap(), []);
}
