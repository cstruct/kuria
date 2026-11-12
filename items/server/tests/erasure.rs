//! Erasure, against a stand-in for Konklav's list of erased subjects and a
//! fresh database for each test.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use konklav_relying_client::{ErasureNotice, SubjectErased};
use kuria_server::erasure::{ErasedSubjects, handle_erasures};
use kuria_store::schema::browser_session;
use kuria_store::sessions::{self, BrowserSession};
use kuria_store::{Pool, erasure};
use kuria_test_support::TestDatabase;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use uuid::Uuid;

const ALICE: Uuid = Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_00a1);
const BOB: Uuid = Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_00b0);

/// Konklav's list of erased subjects, with what it was asked.
#[derive(Default)]
struct FakeList {
    erased: Mutex<Vec<SubjectErased>>,
    asked: Mutex<Vec<Option<i64>>>,
    failing: Mutex<bool>,
}

#[derive(Clone, Default)]
struct List(Arc<FakeList>);

impl List {
    fn erase(&self, subject: Uuid, erased_at: i64) -> SubjectErased {
        let erased = SubjectErased {
            subject: subject.into(),
            erased_at,
        };
        self.0.erased.lock().unwrap().push(erased);
        erased
    }

    fn asked(&self) -> Vec<Option<i64>> {
        self.0.asked.lock().unwrap().clone()
    }

    fn set_failing(&self, failing: bool) {
        *self.0.failing.lock().unwrap() = failing;
    }
}

impl ErasedSubjects for List {
    fn erased_subjects(
        &self,
        since: Option<i64>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SubjectErased>, String>> + Send + '_>> {
        Box::pin(async move {
            self.0.asked.lock().unwrap().push(since);
            if *self.0.failing.lock().unwrap() {
                return Err("Konklav is unreachable".to_owned());
            }
            Ok(self
                .0
                .erased
                .lock()
                .unwrap()
                .iter()
                .filter(|e| since.is_none_or(|since| e.erased_at >= since))
                .copied()
                .collect())
        })
    }
}

struct World {
    db: TestDatabase,
    list: List,
    notices: UnboundedSender<ErasureNotice>,
}

impl World {
    /// Starts handling erasures, retrying failed catch-ups every 20 ms.
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let list = List::default();
        let (notices, receiver) = unbounded_channel();
        tokio::spawn(handle_erasures(
            db.pool().clone(),
            list.clone(),
            receiver,
            Duration::from_millis(20),
        ));
        Self { db, list, notices }
    }

    fn pool(&self) -> &Pool {
        self.db.pool()
    }

    async fn add_session(&self, subject: Uuid) {
        let mut conn = self.pool().get().await.unwrap();
        sessions::insert_session(
            &mut conn,
            &BrowserSession {
                id_hash: Uuid::now_v7().as_bytes().repeat(2),
                subject_id: subject,
                tenant_id: Uuid::nil(),
                access_token: vec![1],
                refresh_token: vec![2],
                access_expires_at: Utc::now(),
            },
        )
        .await
        .unwrap();
    }

    async fn sessions_of(&self, subject: Uuid) -> i64 {
        let mut conn = self.pool().get().await.unwrap();
        browser_session::table
            .filter(browser_session::subject_id.eq(subject))
            .count()
            .get_result(&mut conn)
            .await
            .unwrap()
    }

    async fn handled_until(&self) -> Option<i64> {
        let mut conn = self.pool().get().await.unwrap();
        erasure::handled_until(&mut conn).await.unwrap()
    }

    /// Waits, for up to five seconds, until `done` holds.
    async fn eventually<F, Fut>(&self, what: &str, done: F)
    where
        F: Fn() -> Fut,
        Fut: Future<Output = bool>,
    {
        for _ in 0..250 {
            if done().await {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("timed out waiting until {what}");
    }
}

// r[verify erasure.browser-sessions]
#[tokio::test(flavor = "multi_thread")]
async fn an_erasure_deletes_that_subjects_sessions_only() {
    let world = World::new().await;
    world.add_session(ALICE).await;
    world.add_session(ALICE).await;
    world.add_session(BOB).await;

    let erased = world.list.erase(ALICE, 1_000);
    world.notices.send(ErasureNotice::Erased(erased)).unwrap();
    world
        .eventually("Alice's sessions are gone", || async {
            world.sessions_of(ALICE).await == 0
        })
        .await;
    assert_eq!(world.sessions_of(BOB).await, 1);
    // A live notice doesn't move the stored time.
    assert_eq!(world.handled_until().await, None);
}

// r[verify erasure.catch-up]
#[tokio::test(flavor = "multi_thread")]
async fn catches_up_from_the_stored_time_when_erasures_were_missed() {
    let world = World::new().await;
    world.add_session(ALICE).await;
    world.add_session(BOB).await;
    world.list.erase(ALICE, 1_000);

    // As at every start: no position, so read everything.
    world
        .notices
        .send(ErasureNotice::Missed { since: None })
        .unwrap();
    world
        .eventually("the stored time moves", || async {
            world.handled_until().await == Some(1_000)
        })
        .await;
    assert_eq!(world.sessions_of(ALICE).await, 0);
    assert_eq!(world.sessions_of(BOB).await, 1);

    // The next catch-up reads from the stored time, not from the notice's.
    world.list.erase(BOB, 2_000);
    world
        .notices
        .send(ErasureNotice::Missed { since: Some(1) })
        .unwrap();
    world
        .eventually("the stored time moves again", || async {
            world.handled_until().await == Some(2_000)
        })
        .await;
    assert_eq!(world.sessions_of(BOB).await, 0);
    assert_eq!(world.list.asked(), vec![None, Some(1_000)]);
}

// r[verify erasure.catch-up]
#[tokio::test(flavor = "multi_thread")]
async fn never_moves_the_stored_time_back() {
    let world = World::new().await;
    let mut conn = world.pool().get().await.unwrap();
    erasure::advance_handled_until(&mut conn, 5_000)
        .await
        .unwrap();
    erasure::advance_handled_until(&mut conn, 4_000)
        .await
        .unwrap();
    assert_eq!(world.handled_until().await, Some(5_000));
}

// r[verify erasure.retry]
#[tokio::test(flavor = "multi_thread")]
async fn keeps_trying_a_catch_up_until_it_succeeds() {
    let world = World::new().await;
    world.add_session(ALICE).await;
    world.list.erase(ALICE, 1_000);
    world.list.set_failing(true);

    world
        .notices
        .send(ErasureNotice::Missed { since: None })
        .unwrap();
    world
        .eventually("it has tried three times", || async {
            world.list.asked().len() >= 3
        })
        .await;
    assert_eq!(world.handled_until().await, None);
    assert_eq!(world.sessions_of(ALICE).await, 1);

    // Live notices are still handled meanwhile.
    world.add_session(BOB).await;
    let erased = world.list.erase(BOB, 1_500);
    world.notices.send(ErasureNotice::Erased(erased)).unwrap();
    world
        .eventually("Bob's sessions are gone", || async {
            world.sessions_of(BOB).await == 0
        })
        .await;

    world.list.set_failing(false);
    world
        .eventually("the catch-up succeeds", || async {
            world.handled_until().await == Some(1_500)
        })
        .await;
    assert_eq!(world.sessions_of(ALICE).await, 0);
}
