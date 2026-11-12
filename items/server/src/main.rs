//! Runs the Kuria server, configured from the environment.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use konklav_relying_client::RelyingClient;
use kuria_server::erasure::handle_erasures;
use kuria_server::oidc::{self, Provider};
use kuria_server::session::{self, BrowserSessions, Cookies, SessionCipher};
use kuria_server::{Config, Options, app};
use kuria_store::{ConnectOptions, Pool};
use tracing_subscriber::EnvFilter;

/// How long to wait at startup for the relying client's caches to be
/// trusted. Until they are, decisions go to Konklav's check API.
const TRUSTED_WAIT: Duration = Duration::from_secs(30);

/// How long to wait before reading Konklav's erased subjects again after a
/// read failed.
const ERASURE_RETRY: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;

    // r[impl auth.service-principal]
    let client = RelyingClient::new(config.client());
    // Asked for before starting, so the notice that erasures may have been
    // missed while the server wasn't running isn't lost.
    let erasures = client.erasures();
    // The background tasks follow the change feed and refresh keys and
    // entitlements for as long as the server runs.
    let _tasks = client.start();
    if !wait_until_trusted(&client).await {
        tracing::warn!(
            "the relying client's caches aren't trusted yet; deciding through Konklav's check API until they are"
        );
    }

    let pool = kuria_store::connect(&ConnectOptions {
        url: config.database_url.clone(),
        max_connections: 16,
    })?;
    let applied = kuria_store::run_pending_migrations(&pool).await?;
    if !applied.is_empty() {
        tracing::info!(?applied, "applied migrations");
    }

    let provider = Provider::discover(
        &config.issuer,
        oidc::Client {
            id: config.oidc_client_id.clone(),
            secret: config.oidc_client_secret.clone(),
            redirect_uri: format!("{}/auth/callback", config.public_url),
        },
    )
    .await?;
    let valkey = redis::Client::open(config.valkey_url.as_str())?
        .get_connection_manager()
        .await?;
    let sign_in_limit = session::sign_in_limit(
        "kuria.sign-in",
        session::SIGN_IN_WINDOWS.to_vec(),
        valkey,
        &config.rate_limit_secret,
    )?;
    let sessions = Arc::new(BrowserSessions {
        pool: pool.clone(),
        provider,
        cipher: SessionCipher::from_base64(&config.session_key)?,
        cookies: Cookies::new(config.secure_cookies),
        allowed_origins: config.allowed_origins.clone(),
        sign_in_limit,
    });
    tokio::spawn(handle_erasures(
        pool.clone(),
        client.clone(),
        erasures,
        ERASURE_RETRY,
    ));
    tokio::spawn(sweep_login_attempts(pool));

    let router = app(
        Arc::new(client),
        Options {
            public_url: config.public_url.clone(),
            trusted_proxies: config.trusted_proxies.clone(),
            sessions: Some(sessions),
        },
    )?;
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!(address = %config.listen, public_url = %config.public_url, "listening");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}

async fn wait_until_trusted(client: &RelyingClient) -> bool {
    let deadline = tokio::time::Instant::now() + TRUSTED_WAIT;
    while tokio::time::Instant::now() < deadline {
        if client.is_trusted() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    client.is_trusted()
}

/// Removes expired login attempts every ten minutes.
async fn sweep_login_attempts(pool: Pool) {
    let mut interval = tokio::time::interval(Duration::from_secs(600));
    loop {
        interval.tick().await;
        let swept = async {
            let mut conn = pool.get().await.map_err(|e| e.to_string())?;
            kuria_store::sessions::delete_expired_login_attempts(&mut conn, chrono::Utc::now())
                .await
                .map_err(|e| e.to_string())
        };
        if let Err(e) = swept.await {
            tracing::warn!("removing expired login attempts: {e}");
        }
    }
}
