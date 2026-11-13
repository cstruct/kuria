//! Kuria's Postgres storage: the connection pool, the embedded migrations,
//! and the queries the server runs.
//!
//! Diesel models and queries stay in this crate. What the stored values mean,
//! such as how a session's tokens are encrypted, belongs to the crate that
//! writes them.

pub mod akter;
pub mod attachments;
pub mod bundles;
pub mod contacts;
pub mod containers;
pub mod erasure;
pub mod links;
mod migrate;
pub mod people;
mod pool;
pub mod schema;
pub mod sessions;

pub use migrate::{MigrationError, run_next_migration, run_pending_migrations};
pub use pool::{ConnectOptions, Pool, PoolError, connect};

/// A connection checked out of the [`Pool`].
pub type Connection = diesel_async::AsyncPgConnection;
