//! Akter partitioned on whether they are archived (`archive.storage`): the
//! migration that splits them keeps what was there, an archive moves an
//! akt with its history, numbers stay unique, and live reads read only the
//! live partitions.

use diesel::sql_types::{Bool, Text, Uuid as SqlUuid};
use diesel::{QueryableByName, sql_query};
use diesel_async::{AsyncConnection, RunQueryDsl};
use kuria_store::{Connection, run_next_migration};
use kuria_test_support::TestDatabase;
use uuid::Uuid;

const TENANT: Uuid = Uuid::from_u128(1);
const SUBJECT: Uuid = Uuid::from_u128(3);

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    n: i64,
}

#[derive(QueryableByName)]
struct Line {
    #[diesel(sql_type = Text, column_name = "QUERY PLAN")]
    line: String,
}

async fn count(conn: &mut Connection, query: &str) -> i64 {
    sql_query(query)
        .get_result::<Count>(conn)
        .await
        .unwrap_or_else(|e| panic!("{query}: {e}"))
        .n
}

async fn run(conn: &mut Connection, query: &str) {
    sql_query(query)
        .execute(conn)
        .await
        .unwrap_or_else(|e| panic!("{query}: {e}"));
}

/// Migrates up to, not including, the archive partitions.
async fn before_partitions() -> TestDatabase {
    let db = TestDatabase::unmigrated().await;
    loop {
        let ran = run_next_migration(db.pool()).await.unwrap().unwrap();
        if ran.contains("000024") {
            return db;
        }
    }
}

/// Two akter, `I-1` with two revisions (the second a transition), a
/// version, a live draft update and change events, and `I-2` linked to it.
async fn seed(conn: &mut Connection) -> (Uuid, Uuid) {
    let ty = Uuid::now_v7();
    run(
        conn,
        &format!(
            "insert into akt_type (tenant_id, key, latest_version) values ('{TENANT}', 'item', 1)"
        ),
    )
    .await;
    run(
        conn,
        &format!(
            "insert into akt_type_version (id, tenant_id, key, version, definition, created_by)
         values ('{ty}', '{TENANT}', 'item', 1, '{{}}', '{SUBJECT}')"
        ),
    )
    .await;
    let (one, two) = (Uuid::now_v7(), Uuid::now_v7());
    for (id, number, rank) in [(one, "I-1", "a0"), (two, "I-2", "a1")] {
        run(conn, &format!(
            "insert into akt (id, tenant_id, number, type_version_id, state, fields, revision, created_at, updated_at, rank)
             values ('{id}', '{TENANT}', '{number}', '{ty}', 'open', '{{}}', 1, '2026-01-01', '2026-01-01', '{rank}')"
        ))
        .await;
        run(conn, &format!(
            "insert into akt_revision (akt_id, revision, tenant_id, author, changes, state, hash, created_at)
             values ('{id}', 1, '{TENANT}', '{SUBJECT}', '{{}}', 'open', decode(repeat('00', 32), 'hex'), '2026-01-01')"
        ))
        .await;
    }
    run(conn, &format!(
        "insert into akt_revision (akt_id, revision, tenant_id, author, transition, changes, state, hash, created_at)
         values ('{one}', 2, '{TENANT}', '{SUBJECT}', 'close', '{{}}', 'closed', decode(repeat('00', 32), 'hex'), '2026-02-01')"
    ))
    .await;
    run(
        conn,
        &format!("update akt set revision = 2, state = 'closed' where id = '{one}'"),
    )
    .await;
    run(conn, &format!(
        "insert into change_event (tenant_id, seq, akt_id, revision, created_at, kind)
         values ('{TENANT}', 1, '{one}', 1, now(), 'revision'), ('{TENANT}', 2, '{one}', 2, now(), 'revision'),
                ('{TENANT}', 3, '{two}', 1, now(), 'revision')"
    ))
    .await;
    run(
        conn,
        &format!(
        "insert into akt_version (tenant_id, akt_id, number, revision, released_by, released_at)
         values ('{TENANT}', '{one}', 1, 2, '{SUBJECT}', now())"
    ),
    )
    .await;
    run(
        conn,
        &format!(
            "insert into akt_draft (tenant_id, akt_id, state, fields, updated_at)
         values ('{TENANT}', '{one}', '\\x00', '{{}}', now())"
        ),
    )
    .await;
    run(
        conn,
        &format!(
            "insert into akt_draft_update (tenant_id, akt_id, seq, update, by, at)
         values ('{TENANT}', '{one}', 1, '\\x00', '{SUBJECT}', now())"
        ),
    )
    .await;
    run(
        conn,
        &format!(
            "insert into akt_link (tenant_id, source_id, kind, target_id, added_at, added_by)
         values ('{TENANT}', '{two}', 'refers_to', '{one}', now(), '{SUBJECT}')"
        ),
    )
    .await;
    (one, two)
}

// r[verify archive.storage]
#[tokio::test(flavor = "multi_thread")]
async fn the_migration_keeps_every_akt_live_with_its_history() {
    let db = before_partitions().await;
    let mut conn = db.pool().get().await.unwrap();
    let (one, _) = seed(&mut conn).await;
    drop(conn);
    assert!(
        run_next_migration(db.pool())
            .await
            .unwrap()
            .unwrap()
            .contains("000025")
    );

    let mut conn = db.pool().get().await.unwrap();
    for (query, n) in [
        ("select count(*) as n from akt_live", 2),
        ("select count(*) as n from akt_archived", 0),
        ("select count(*) as n from akt_number", 2),
        ("select count(*) as n from akt_revision_live", 3),
        ("select count(*) as n from change_event_live", 3),
        ("select count(*) as n from akt_draft_update_live", 1),
        (
            "select count(*) as n from akt_version where not akt_archived",
            1,
        ),
        ("select count(*) as n from akt_link", 1),
    ] {
        assert_eq!(count(&mut conn, query).await, n, "{query}");
    }
    // When it reached its state: its last transition, else its creation.
    assert_eq!(
        count(
            &mut conn,
            &format!(
                "select count(*) as n from akt where id = '{one}' and state_since = '2026-02-01'"
            )
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &mut conn,
            "select count(*) as n from akt where state_since = '2026-01-01'"
        )
        .await,
        1
    );
}

/// The partitioned database, seeded.
async fn partitioned() -> (TestDatabase, Uuid, Uuid) {
    let db = before_partitions().await;
    let mut conn = db.pool().get().await.unwrap();
    let (one, two) = seed(&mut conn).await;
    drop(conn);
    run_next_migration(db.pool()).await.unwrap().unwrap();
    (db, one, two)
}

async fn archive(conn: &mut Connection, id: Uuid, archived: bool) -> diesel::QueryResult<usize> {
    let set = if archived {
        format!(
            "archived = true, archived_at = now(), archived_by = '{{\"subject\": \"{SUBJECT}\"}}'"
        )
    } else {
        "archived = false, archived_at = null, archived_by = null".to_owned()
    };
    let query = format!("update akt set {set} where id = '{id}'");
    conn.transaction::<_, diesel::result::Error, _>(async |conn| {
        sql_query("select set_config('kuria.archiving', 'on', true)")
            .execute(conn)
            .await?;
        sql_query(query).execute(conn).await
    })
    .await
}

// r[verify archive.storage]
#[tokio::test(flavor = "multi_thread")]
async fn an_archive_moves_an_akt_with_its_history() {
    let (db, one, _) = partitioned().await;
    let mut conn = db.pool().get().await.unwrap();
    assert_eq!(archive(&mut conn, one, true).await.unwrap(), 1);
    for (query, n) in [
        ("select count(*) as n from akt_archived", 1),
        ("select count(*) as n from akt_live", 1),
        ("select count(*) as n from akt_revision_archived", 2),
        ("select count(*) as n from akt_revision_live", 1),
        ("select count(*) as n from change_event_archived", 2),
        ("select count(*) as n from akt_draft_update_archived", 1),
        (
            "select count(*) as n from akt_version where akt_archived",
            1,
        ),
        // What only names it is left as it was.
        ("select count(*) as n from akt_link", 1),
        ("select count(*) as n from akt_draft", 1),
    ] {
        assert_eq!(count(&mut conn, query).await, n, "{query}");
    }
    // And back.
    assert_eq!(archive(&mut conn, one, false).await.unwrap(), 1);
    assert_eq!(
        count(&mut conn, "select count(*) as n from akt_revision_live").await,
        3
    );
    assert_eq!(
        count(&mut conn, "select count(*) as n from akt_archived").await,
        0
    );

    // Without saying it is archiving, the move is refused: revisions stay
    // append only.
    let refused = sql_query(format!(
        "update akt set archived = true, archived_at = now(), archived_by = '{{}}' where id = '{one}'"
    ))
    .execute(&mut conn)
    .await;
    assert!(refused.is_err(), "a move outside an archive");
    for query in [
        format!("update akt_revision set reason = 'rewritten' where akt_id = '{one}'"),
        format!("delete from akt_revision where akt_id = '{one}'"),
        format!("update akt_version set label = 'rewritten' where akt_id = '{one}'"),
        format!("update akt_draft_update set message = 'rewritten' where akt_id = '{one}'"),
    ] {
        assert!(
            sql_query(&query).execute(&mut conn).await.is_err(),
            "{query}"
        );
    }
}

// r[verify archive.storage]
#[tokio::test(flavor = "multi_thread")]
async fn numbers_stay_unique_across_partitions() {
    let (db, one, _) = partitioned().await;
    let mut conn = db.pool().get().await.unwrap();
    archive(&mut conn, one, true).await.unwrap();
    let ty: Uuid = sql_query("select type_version_id as id from akt limit 1")
        .get_result::<Id>(&mut conn)
        .await
        .unwrap()
        .id;
    let again = sql_query(format!(
        "insert into akt (id, tenant_id, number, type_version_id, state, fields, revision, created_at, updated_at, rank, state_since)
         values ('{}', '{TENANT}', 'I-1', '{ty}', 'open', '{{}}', 1, now(), now(), 'b0', now())",
        Uuid::now_v7()
    ))
    .execute(&mut conn)
    .await;
    assert!(again.is_err(), "I-1 is taken by the archived akt");
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

// r[verify archive.storage.live]
#[tokio::test(flavor = "multi_thread")]
async fn live_reads_read_only_the_live_partitions() {
    let (db, _, _) = partitioned().await;
    let mut conn = db.pool().get().await.unwrap();
    for query in [
        "select id from akt where tenant_id = $1 and archived = $2 order by updated_at desc, id limit 50",
        "select id from akt where tenant_id = $1 and archived = $2 and lower(number) like 'i-%'",
        "select r.revision from akt_revision r where r.akt_id = $1 and r.akt_archived = $2",
    ] {
        let plan: Vec<Line> = sql_query(format!("explain (costs off) {query}"))
            .bind::<SqlUuid, _>(TENANT)
            .bind::<Bool, _>(false)
            .load(&mut conn)
            .await
            .unwrap();
        let plan: String = plan.into_iter().map(|l| l.line + "\n").collect();
        assert!(plan.contains("_live"), "{query}\n{plan}");
        // No archived partition, nor any of its indexes, is read.
        for archived in ["on akt_archived", "akt_archived_", "akt_revision_archived"] {
            assert!(!plan.contains(archived), "{query}\n{plan}");
        }
    }
}
