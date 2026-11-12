use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool;

/// A pool of Postgres connections.
pub type Pool = deadpool::Pool<AsyncPgConnection>;

/// The error of building a pool.
pub type PoolError = deadpool::BuildError;

/// How to reach the database.
#[derive(Clone)]
pub struct ConnectOptions {
    /// A libpq-style connection URL. It usually holds a password, so it is
    /// never logged.
    pub url: String,
    pub max_connections: usize,
}

impl std::fmt::Debug for ConnectOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectOptions")
            .field("url", &"<redacted>")
            .field("max_connections", &self.max_connections)
            .finish()
    }
}

/// Builds a pool. Connections are opened when they are first checked out.
pub fn connect(options: &ConnectOptions) -> Result<Pool, PoolError> {
    let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(&options.url);
    Pool::builder(manager)
        .max_size(options.max_connections)
        .build()
}
