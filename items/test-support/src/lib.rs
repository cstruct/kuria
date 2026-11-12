//! Test helpers shared by Kuria's software items.
//!
//! Tests get an isolated database each: [`TestDatabase`] creates a fresh,
//! migrated database on the development Postgres server and drops it again
//! afterwards. The server comes from `KURIA_TEST_DATABASE_URL`, or
//! `DATABASE_URL` as mise sets it; start it with `mise run db:up`.
//!
//! Database tests must run on a multi-threaded runtime, because the
//! migration harness blocks its thread:
//!
//! ```ignore
//! #[tokio::test(flavor = "multi_thread")]
//! async fn stores_things() {
//!     let db = TestDatabase::new().await;
//!     // use db.pool()
//! }
//! ```

use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use kuria_store::{ConnectOptions, Pool};

const SERVER_URL_VARS: [&str; 2] = ["KURIA_TEST_DATABASE_URL", "DATABASE_URL"];
const VALKEY_URL_VARS: [&str; 2] = ["KURIA_TEST_VALKEY_URL", "KURIA_VALKEY_URL"];

/// The Valkey server for tests, from `KURIA_TEST_VALKEY_URL` or
/// `KURIA_VALKEY_URL` as mise sets it; start it with `mise run db:up`. Tests
/// keep apart by naming their rate limits uniquely ([`unique_name`]).
///
/// # Panics
///
/// When neither is set.
#[must_use]
pub fn valkey_url() -> String {
    VALKEY_URL_VARS
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| {
            panic!(
                "set {} to a Valkey URL (mise sets KURIA_VALKEY_URL; start it with `mise run db:up`)",
                VALKEY_URL_VARS.join(" or ")
            )
        })
}

/// A name no other test uses, such as for a rate limit: `prefix`, a dash and
/// 32 hex digits.
#[must_use]
pub fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::now_v7().simple())
}

/// A freshly created and migrated database, dropped when this value is.
pub struct TestDatabase {
    name: String,
    admin_url: String,
    pool: Pool,
}

impl TestDatabase {
    /// Creates a uniquely named database and applies every migration.
    ///
    /// # Panics
    ///
    /// When no server URL is configured, the server is unreachable, or
    /// migrations fail: a test cannot proceed in any of those cases.
    pub async fn new() -> Self {
        let db = Self::unmigrated().await;
        kuria_store::run_pending_migrations(&db.pool)
            .await
            .expect("migrate test database");
        db
    }

    /// Creates a uniquely named database without applying any migration, for
    /// tests that apply them one by one with
    /// [`kuria_store::run_next_migration`].
    ///
    /// # Panics
    ///
    /// When no server URL is configured or the server is unreachable.
    pub async fn unmigrated() -> Self {
        let admin_url = server_url();
        let name = format!("kuria_test_{}", uuid::Uuid::now_v7().simple());
        let url = with_database(&admin_url, &name);

        let mut admin = AsyncPgConnection::establish(&admin_url)
            .await
            .unwrap_or_else(|e| {
                panic!("cannot reach the test Postgres server (is `mise run db:up` running?): {e}")
            });
        diesel::sql_query(format!("create database \"{name}\""))
            .execute(&mut admin)
            .await
            .expect("create test database");

        let pool = kuria_store::connect(&ConnectOptions {
            url,
            max_connections: 8,
        })
        .expect("build test pool");

        Self {
            name,
            admin_url,
            pool,
        }
    }

    #[must_use]
    pub fn pool(&self) -> &Pool {
        &self.pool
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        // Drop runs outside async context, possibly inside a runtime that is
        // shutting down, so the cleanup gets its own thread and runtime.
        let admin_url = self.admin_url.clone();
        let name = self.name.clone();
        let cleanup = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build cleanup runtime");
            runtime.block_on(async move {
                let mut admin = AsyncPgConnection::establish(&admin_url).await?;
                diesel::sql_query(format!("drop database if exists \"{name}\" with (force)"))
                    .execute(&mut admin)
                    .await?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
            })
        });
        if let Ok(Err(error)) = cleanup.join() {
            eprintln!("could not drop test database {}: {error}", self.name);
        }
    }
}

fn server_url() -> String {
    SERVER_URL_VARS
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| {
            panic!(
                "set {} to a Postgres server URL (mise sets DATABASE_URL; start it with `mise run db:up`)",
                SERVER_URL_VARS.join(" or ")
            )
        })
}

/// `url` with its database replaced by `name`.
fn with_database(url: &str, name: &str) -> String {
    let (base, query) = url
        .split_once('?')
        .map_or((url, None), |(b, q)| (b, Some(q)));
    let base = base.rsplit_once('/').map_or(base, |(prefix, _)| prefix);
    match query {
        Some(query) => format!("{base}/{name}?{query}"),
        None => format!("{base}/{name}"),
    }
}
