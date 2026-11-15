//! Runs the Kuria server, configured from the environment.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use konklav_relying_client::RelyingClient;
use kuria_server::attachments::Storage;
use kuria_server::changes::{Hub, Relay};
use kuria_server::cli_relay::{self as relay, CliRelay};
use kuria_server::erasure::handle_erasures;
use kuria_server::oidc::{self, Provider};
use kuria_server::session::{self, BrowserSessions, Cookies, SessionCipher};
use kuria_server::web::{LoginProxy, Web};
use kuria_server::{Changes, Config, Options, app};
use kuria_store::{ConnectOptions, Pool};
use tracing_subscriber::EnvFilter;

/// How long to wait at startup for the relying client's caches to be
/// trusted. Until they are, decisions go to Konklav's check API.
const TRUSTED_WAIT: Duration = Duration::from_secs(30);

/// How long to wait before reading Konklav's erased subjects again after a
/// read failed.
const ERASURE_RETRY: Duration = Duration::from_secs(30);

fn main() -> ExitCode {
    // `kuria-server render` renders one PDF or diagram under limits, for
    // the server that started it (`render.limits`).
    if std::env::args().nth(1).as_deref() == Some("render") {
        return kuria_typst::worker::main();
    }
    serve()
}

#[tokio::main]
async fn serve() -> ExitCode {
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

/// The CLI's login relay, holding codes in Valkey and limited by rate.
fn cli_relay(
    valkey: redis::aio::ConnectionManager,
    config: &Config,
) -> Result<Arc<CliRelay>, Box<dyn std::error::Error>> {
    Ok(Arc::new(CliRelay {
        limit: session::sign_in_limit(
            "kuria.cli-relay",
            relay::RELAY_WINDOWS.to_vec(),
            valkey.clone(),
            &config.rate_limit_secret,
        )?,
        valkey,
        hold: relay::HOLD,
        discovery: relay::Discovery::new(&config.public_url, &config.issuer, config.default_tenant),
    }))
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // `kuria-server schema` prints the GraphQL schema, for the app's
    // generated types.
    if std::env::args().nth(1).as_deref() == Some("schema") {
        print!("{}", kuria_server::schema_sdl());
        return Ok(());
    }
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

    // From Konklav directly: the issuer may be this server's own origin,
    // proxying Konklav, which isn't listening yet.
    let provider = Provider::discover_at(
        &config.subject_url,
        &config.issuer,
        oidc::Client {
            id: config.oidc_client_id.clone(),
            secret: config.oidc_client_secret.clone(),
            redirect_uri: format!("{}/auth/callback", config.public_url),
        },
    )
    .await?;
    let valkey_client = redis::Client::open(config.valkey_url.as_str())?;
    let valkey = valkey_client.get_connection_manager().await?;
    let sign_in_limit = session::sign_in_limit(
        "kuria.sign-in",
        session::SIGN_IN_WINDOWS.to_vec(),
        valkey.clone(),
        &config.rate_limit_secret,
    )?;
    let sessions = Arc::new(BrowserSessions {
        pool: pool.clone(),
        provider,
        cipher: SessionCipher::from_base64(&config.session_key)?,
        cookies: Cookies::new(config.secure_cookies),
        allowed_origins: config.allowed_origins.clone(),
        sign_in_limit,
        default_tenant: config.default_tenant,
    });
    tokio::spawn(handle_erasures(
        pool.clone(),
        client.clone(),
        erasures,
        ERASURE_RETRY,
    ));
    tokio::spawn(sweep_login_attempts(pool.clone()));

    // Change events: every instance publishes the outbox and fans events out
    // to its own subscribers.
    let relay = Relay::new(pool.clone(), valkey.clone());
    let waker = relay.waker();
    tokio::spawn(relay.run());
    let hub = Hub::new(pool.clone(), valkey_client.clone());
    tokio::spawn(hub.clone().run());

    let storage = Storage::s3(
        &config.storage,
        usize::try_from(config.attachment_limit).unwrap_or(usize::MAX),
    )?;

    let router = app(
        Arc::new(client.clone()),
        Options {
            public_url: config.public_url.clone(),
            trusted_proxies: config.trusted_proxies.clone(),
            pool: pool.clone(),
            decider: Arc::new(client.clone()),
            sessions: Some(sessions),
            changes: Some(Changes { hub, waker }),
            storage,
            web: config.web_dir.as_deref().map(Web::load).transpose()?,
            login_proxy: if config.login_app {
                Some(LoginProxy::new(&config.subject_url)?)
            } else {
                None
            },
            cli_relay: Some(cli_relay(valkey.clone(), &config)?),
            renderer: kuria_typst::Renderer::current(config.render_limits)?,
            git_cache: config.git_cache.clone(),
            git_timing: kuria_server::GitTiming::serving(),
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
