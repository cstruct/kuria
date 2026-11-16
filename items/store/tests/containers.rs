//! What the database itself guards for memberships and ranks, and the
//! migration that brought them in.

use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use kuria_engine::nth_appended;
use kuria_store::akter::{self, AktRow};
use kuria_store::containers::{self, MembershipRow};
use kuria_store::schema::akt;
use kuria_store::{Connection, run_next_migration};
use kuria_test_support::TestDatabase;
use serde_json::json;
use uuid::Uuid;

const TENANT: Uuid = Uuid::from_u128(1);
const OTHER: Uuid = Uuid::from_u128(2);
const SUBJECT: Uuid = Uuid::from_u128(3);

/// An akt type version in `tenant`, for akter to point to.
async fn type_version(conn: &mut Connection, tenant: Uuid, key: &str) -> Uuid {
    akter::insert_type_version(conn, tenant, key, &json!({}), SUBJECT, Utc::now())
        .await
        .unwrap()
        .id
}

async fn insert(conn: &mut Connection, tenant: Uuid, ty: Uuid, number: &str, rank: &str) -> AktRow {
    let row = AktRow {
        id: Uuid::now_v7(),
        tenant_id: tenant,
        number: number.to_owned(),
        type_version_id: ty,
        state: "open".to_owned(),
        fields: json!({}),
        revision: 1,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        rank: rank.to_owned(),
        title: None,
        state_since: Utc::now(),
        archived: false,
        archived_at: None,
        archived_by: None,
        archive_reason: None,
        archive_kept: false,
    };
    akter::insert_akt(conn, &row).await.unwrap();
    row
}

fn membership(container: &AktRow, member: &AktRow) -> MembershipRow {
    MembershipRow {
        tenant_id: container.tenant_id,
        container_id: container.id,
        member_id: member.id,
        added_at: Utc::now(),
        added_by: SUBJECT,
    }
}

// r[verify order.rank]
#[tokio::test(flavor = "multi_thread")]
async fn ranks_are_unique_per_tenant() {
    let db = TestDatabase::new().await;
    let mut conn = db.pool().get().await.unwrap();
    let ty = type_version(&mut conn, TENANT, "item").await;
    let other_ty = type_version(&mut conn, OTHER, "item").await;
    insert(&mut conn, TENANT, ty, "I-1", "a0").await;
    insert(&mut conn, OTHER, other_ty, "I-1", "a0").await;
    let row = AktRow {
        id: Uuid::now_v7(),
        number: "I-2".to_owned(),
        ..insert(&mut conn, TENANT, ty, "I-3", "a1").await
    };
    let duplicate = AktRow {
        rank: "a0".to_owned(),
        title: None,
        ..row
    };
    assert!(akter::insert_akt(&mut conn, &duplicate).await.is_err());
}

// r[verify order.everywhere]
#[tokio::test(flavor = "multi_thread")]
async fn ranks_compare_byte_by_byte() {
    let db = TestDatabase::new().await;
    let mut conn = db.pool().get().await.unwrap();
    let ty = type_version(&mut conn, TENANT, "item").await;
    // Uppercase sorts before lowercase in bytes, unlike in most collations.
    insert(&mut conn, TENANT, ty, "I-1", "a0").await;
    insert(&mut conn, TENANT, ty, "I-2", "Zz").await;
    insert(&mut conn, TENANT, ty, "I-3", "a0V").await;
    let order: Vec<String> = akt::table
        .filter(akt::tenant_id.eq(TENANT))
        .order(akt::rank)
        .select(akt::number)
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(order, ["I-2", "I-1", "I-3"]);
    assert_eq!(
        containers::last_rank(&mut conn, TENANT)
            .await
            .unwrap()
            .as_deref(),
        Some("a0V")
    );
    assert_eq!(
        containers::rank_before(&mut conn, TENANT, "a0")
            .await
            .unwrap()
            .as_deref(),
        Some("Zz")
    );
    assert_eq!(
        containers::rank_after(&mut conn, TENANT, "a0")
            .await
            .unwrap()
            .as_deref(),
        Some("a0V")
    );
}

// r[verify container.many]
#[tokio::test(flavor = "multi_thread")]
async fn a_membership_is_stored_once_and_never_of_itself() {
    let db = TestDatabase::new().await;
    let mut conn = db.pool().get().await.unwrap();
    let ty = type_version(&mut conn, TENANT, "item").await;
    let backlog = insert(&mut conn, TENANT, ty, "B-1", "a0").await;
    let epic = insert(&mut conn, TENANT, ty, "E-1", "a1").await;
    let item = insert(&mut conn, TENANT, ty, "I-1", "a2").await;
    containers::insert_membership(&mut conn, &membership(&backlog, &item))
        .await
        .unwrap();
    containers::insert_membership(&mut conn, &membership(&epic, &item))
        .await
        .unwrap();
    assert!(
        containers::insert_membership(&mut conn, &membership(&backlog, &item))
            .await
            .is_err()
    );
    assert!(
        containers::insert_membership(&mut conn, &membership(&item, &item))
            .await
            .is_err()
    );
    let of_item = containers::containers_of(&mut conn, TENANT, item.id)
        .await
        .unwrap();
    let numbers: Vec<_> = of_item.iter().map(|(row, _)| row.number.as_str()).collect();
    assert_eq!(numbers, ["B-1", "E-1"]);
    assert!(
        containers::delete_membership(&mut conn, TENANT, backlog.id, item.id)
            .await
            .unwrap()
    );
    assert!(
        !containers::delete_membership(&mut conn, TENANT, backlog.id, item.id)
            .await
            .unwrap()
    );
}

// r[related container.acyclic]
#[tokio::test(flavor = "multi_thread")]
async fn finds_akter_contained_through_other_containers() {
    let db = TestDatabase::new().await;
    let mut conn = db.pool().get().await.unwrap();
    let ty = type_version(&mut conn, TENANT, "item").await;
    let release = insert(&mut conn, TENANT, ty, "R-1", "a0").await;
    let rc = insert(&mut conn, TENANT, ty, "R-1-RC-1", "a1").await;
    let epic = insert(&mut conn, TENANT, ty, "E-1", "a2").await;
    let item = insert(&mut conn, TENANT, ty, "I-1", "a3").await;
    for (container, member) in [(&release, &rc), (&rc, &epic), (&epic, &item)] {
        containers::insert_membership(&mut conn, &membership(container, member))
            .await
            .unwrap();
    }
    assert!(
        containers::contains(&mut conn, TENANT, release.id, item.id)
            .await
            .unwrap()
    );
    assert!(
        containers::contains(&mut conn, TENANT, item.id, item.id)
            .await
            .unwrap()
    );
    assert!(
        !containers::contains(&mut conn, TENANT, item.id, release.id)
            .await
            .unwrap()
    );
    assert!(
        !containers::contains(&mut conn, OTHER, release.id, item.id)
            .await
            .unwrap()
    );
}

// r[verify order.everywhere]
#[tokio::test(flavor = "multi_thread")]
async fn lists_members_in_rank_order_by_readable_type() {
    let db = TestDatabase::new().await;
    let mut conn = db.pool().get().await.unwrap();
    let item = type_version(&mut conn, TENANT, "item").await;
    let bug = type_version(&mut conn, TENANT, "bug").await;
    let board = insert(&mut conn, TENANT, item, "B-1", "a0").await;
    let late = insert(&mut conn, TENANT, item, "I-1", "a9").await;
    let early = insert(&mut conn, TENANT, item, "I-2", "a3").await;
    let hidden = insert(&mut conn, TENANT, bug, "BUG-1", "a1").await;
    for member in [&late, &early, &hidden] {
        containers::insert_membership(&mut conn, &membership(&board, member))
            .await
            .unwrap();
    }
    let members = containers::members_of(&mut conn, TENANT, &[board.id], &["item".to_owned()], 10)
        .await
        .unwrap();
    let numbers: Vec<_> = members.iter().map(|(_, row)| row.number.as_str()).collect();
    assert_eq!(numbers, ["I-2", "I-1"]);
    let first = containers::members_of(
        &mut conn,
        TENANT,
        &[board.id],
        &["item".to_owned(), "bug".to_owned()],
        2,
    )
    .await
    .unwrap();
    let numbers: Vec<_> = first.iter().map(|(_, row)| row.number.as_str()).collect();
    assert_eq!(numbers, ["BUG-1", "I-2"]);
}

// r[verify order.rank]
#[tokio::test(flavor = "multi_thread")]
async fn the_migration_ranks_existing_akter_in_creation_order() {
    let db = TestDatabase::unmigrated().await;
    // Up to and including the migration that added akter.
    for _ in 0..3 {
        run_next_migration(db.pool()).await.unwrap().unwrap();
    }
    let mut conn = db.pool().get().await.unwrap();
    let ty = type_version(&mut conn, TENANT, "item").await;
    let start = Utc::now();
    // More than 62, so the keys cross from `az` to `b00`.
    for n in 0..70_i64 {
        diesel::sql_query(
            "insert into akt (id, tenant_id, number, type_version_id, state, fields, revision, created_at, updated_at)
             values ($1, $2, $3, $4, 'open', '{}', 1, $5, $5)",
        )
        .bind::<diesel::sql_types::Uuid, _>(Uuid::now_v7())
        .bind::<diesel::sql_types::Uuid, _>(TENANT)
        .bind::<diesel::sql_types::Text, _>(format!("I-{n}"))
        .bind::<diesel::sql_types::Uuid, _>(ty)
        // Created in reverse of insertion, to prove the order is creation's.
        .bind::<diesel::sql_types::Timestamptz, _>(start - Duration::seconds(n))
        .execute(&mut conn)
        .await
        .unwrap();
    }
    drop(conn);
    run_next_migration(db.pool()).await.unwrap().unwrap();

    let mut conn = db.pool().get().await.unwrap();
    let ranked: Vec<(String, String)> = akt::table
        .filter(akt::tenant_id.eq(TENANT))
        .order(akt::created_at)
        .select((akt::number, akt::rank))
        .load(&mut conn)
        .await
        .unwrap();
    for (n, (number, rank)) in ranked.iter().enumerate() {
        assert_eq!(rank, &nth_appended(n as u64), "{number}");
    }
    assert_eq!(ranked[0].0, "I-69");
}

// r[verify akt.title]
#[tokio::test(flavor = "multi_thread")]
async fn the_migration_gives_existing_akter_their_titles() {
    let db = TestDatabase::unmigrated().await;
    // Up to the migration before titles.
    for _ in 0..7 {
        run_next_migration(db.pool()).await.unwrap().unwrap();
    }
    let mut conn = db.pool().get().await.unwrap();
    let pool = db.pool();
    let version = |definition: serde_json::Value| async move {
        let mut conn = pool.get().await.unwrap();
        akter::insert_type_version(&mut conn, TENANT, "t", &definition, SUBJECT, Utc::now())
            .await
            .unwrap()
            .id
    };
    let text = |key: &str, required: bool, multiline: bool| json!({ "key": key, "name": key, "kind": "text", "required": required, "multiline": multiline });
    // Its presentation names the title.
    let presented = version(json!({
        "fields": [text("summary", true, false), text("headline", false, false)],
        "presentation": { "title": "headline" }
    }))
    .await;
    // The first required single-line text field, after a multiline one and
    // an optional one.
    let defaulted = version(json!({
        "fields": [text("notes", true, true), text("label", false, false), text("name", true, false)]
    }))
    .await;
    // None.
    let untitled = version(json!({ "fields": [text("notes", false, true)] })).await;

    let akt = |number: &str, ty: Uuid, fields: serde_json::Value| {
        diesel::sql_query(
            "insert into akt (id, tenant_id, number, type_version_id, state, fields, revision, created_at, updated_at, rank)
             values ($1, $2, $3, $4, 'open', $5, 1, now(), now(), $6)",
        )
        .bind::<diesel::sql_types::Uuid, _>(Uuid::now_v7())
        .bind::<diesel::sql_types::Uuid, _>(TENANT)
        .bind::<diesel::sql_types::Text, _>(number.to_owned())
        .bind::<diesel::sql_types::Uuid, _>(ty)
        .bind::<diesel::sql_types::Jsonb, _>(fields)
        .bind::<diesel::sql_types::Text, _>(number.to_owned())
    };
    for query in [
        akt(
            "A-1",
            presented,
            json!({ "summary": "s", "headline": " Passkeys " }),
        ),
        akt("A-2", presented, json!({ "summary": "s" })),
        akt(
            "A-3",
            defaulted,
            json!({ "notes": "n", "label": "l", "name": "Crash" }),
        ),
        akt("A-4", untitled, json!({ "notes": "n" })),
    ] {
        query.execute(&mut conn).await.unwrap();
    }
    drop(conn);
    run_next_migration(db.pool()).await.unwrap().unwrap();

    let mut conn = db.pool().get().await.unwrap();
    let titles: Vec<(String, Option<String>)> = akt::table
        .order(akt::number)
        .select((akt::number, akt::title))
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(
        titles,
        [
            ("A-1".to_owned(), Some("Passkeys".to_owned())),
            ("A-2".to_owned(), None),
            ("A-3".to_owned(), Some("Crash".to_owned())),
            ("A-4".to_owned(), None),
        ]
    );
}
