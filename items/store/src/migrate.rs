use diesel_async::AsyncMigrationHarness;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::Pool;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("checking out a connection: {0}")]
    Pool(#[from] diesel_async::pooled_connection::deadpool::PoolError),
    #[error("migration failed: {0}")]
    Migration(Box<dyn std::error::Error + Send + Sync>),
}

/// Applies every pending embedded migration, in order, and returns their
/// versions.
///
/// The harness blocks its thread while it runs, so this must be called from
/// a multi-threaded tokio runtime.
pub async fn run_pending_migrations(pool: &Pool) -> Result<Vec<String>, MigrationError> {
    let conn = pool.get().await?;
    let mut harness = AsyncMigrationHarness::new(conn);
    let applied = harness
        .run_pending_migrations(MIGRATIONS)
        .map_err(MigrationError::Migration)?;
    Ok(applied.iter().map(ToString::to_string).collect())
}

/// Applies the next pending migration, if any, and returns its version.
/// For tests that check a migration against data written before it.
pub async fn run_next_migration(pool: &Pool) -> Result<Option<String>, MigrationError> {
    let conn = pool.get().await?;
    let mut harness = AsyncMigrationHarness::new(conn);
    let pending = harness
        .pending_migrations(MIGRATIONS)
        .map_err(MigrationError::Migration)?;
    let Some(next) = pending.first() else {
        return Ok(None);
    };
    let version = harness
        .run_migration(next.as_ref())
        .map_err(MigrationError::Migration)?;
    Ok(Some(version.to_string()))
}
