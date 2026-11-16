//! The server's configuration, read from the environment.

use std::net::SocketAddr;

use konklav_relying::Config as RelyingConfig;
use konklav_relying_client::ClientConfig;

/// What the server needs to run. It holds secrets, so it is not `Debug`.
#[derive(Clone)]
pub struct Config {
    /// The address to listen on (`KURIA_LISTEN`, `0.0.0.0:8080` by default).
    pub listen: SocketAddr,
    /// The URL clients address the server by, without a trailing slash
    /// (`KURIA_PUBLIC_URL`). DPoP proofs name it rather than the address the
    /// server listens on.
    pub public_url: String,
    /// Konklav's issuer, as its tokens' `iss` names it (`KONKLAV_ISSUER`).
    pub issuer: String,
    /// The audiences Kuria accepts tokens for (`KONKLAV_AUDIENCE`,
    /// comma-separated): its OIDC client's, for browsers, and Konklav's own,
    /// if software that signs in to Konklav directly calls Kuria.
    pub audiences: Vec<String>,
    /// Konklav's relying surface (`KONKLAV_RELYING_URL`).
    pub relying_url: String,
    /// Konklav's subject-facing surface, where the service principal logs in
    /// (`KONKLAV_SUBJECT_URL`).
    pub subject_url: String,
    /// The service principal's secret key id (`KONKLAV_SERVICE_KEY`, as
    /// `<key id>.<secret>`).
    pub key_id: String,
    /// The service principal's secret.
    pub secret: String,
    /// Kuria's own database (`DATABASE_URL`).
    pub database_url: String,
    /// Kuria's OIDC client id at Konklav, for browser sign-in
    /// (`KURIA_OIDC_CLIENT_ID`).
    pub oidc_client_id: String,
    /// Kuria's OIDC client secret (`KURIA_OIDC_CLIENT_SECRET`).
    pub oidc_client_secret: String,
    /// The key browser sessions' tokens are encrypted under, 32 bytes in
    /// base64 (`KURIA_SESSION_KEY`).
    pub session_key: String,
    /// The origins cookie requests may come from (`KURIA_ALLOWED_ORIGINS`,
    /// comma-separated; the public URL's origin by default).
    pub allowed_origins: Vec<String>,
    /// Valkey, for state the server instances share, such as rate limits
    /// (`KURIA_VALKEY_URL`, a `redis://` URL).
    pub valkey_url: String,
    /// The secret rate-limit keys are hashed under before they reach Valkey,
    /// at least 32 bytes (`KURIA_RATE_LIMIT_SECRET`). Every instance must
    /// share it.
    pub rate_limit_secret: String,
    /// The proxies whose `X-Forwarded-For` names the client, as addresses or
    /// CIDR networks (`KURIA_TRUSTED_PROXIES`, comma-separated; none by
    /// default).
    pub trusted_proxies: Vec<String>,
    /// Where attachments are stored.
    pub storage: StorageConfig,
    /// The largest attachment accepted, in bytes (`KURIA_ATTACHMENT_LIMIT`,
    /// 100 MiB by default).
    pub attachment_limit: u64,
    /// What rendering a PDF or a diagram may use (`render.limits`):
    /// `KURIA_RENDER_SECONDS` (30), `KURIA_RENDER_CPU_SECONDS` (20) and
    /// `KURIA_RENDER_MEMORY_MB` (1024).
    pub render_limits: kuria_typst::Limits,
    /// The tenant a sign-in that names none is for (`KURIA_DEFAULT_TENANT`).
    pub default_tenant: Option<uuid::Uuid>,
    /// How often archive rules are applied to every akt
    /// (`KURIA_ARCHIVE_SWEEP_SECONDS`, 3600 by default; `archive.sweep`).
    pub archive_sweep: std::time::Duration,
    /// The built app, to serve from the server's own origin
    /// (`KURIA_WEB_DIR`).
    pub web_dir: Option<std::path::PathBuf>,
    /// Whether the server is Konklav's login application, proxying its
    /// subject-facing surface (`KURIA_LOGIN_APP`, `false` by default).
    pub login_app: bool,
    /// Whether cookies are `Secure` and `__Host-` prefixed
    /// (`KURIA_SECURE_COOKIES`, `true` by default). Turn it off only for
    /// local development over plain HTTP.
    pub secure_cookies: bool,
}

/// S3-compatible object storage for attachments.
#[derive(Clone)]
pub struct StorageConfig {
    /// The bucket (`KURIA_S3_BUCKET`).
    pub bucket: String,
    /// An S3-compatible endpoint other than AWS (`KURIA_S3_ENDPOINT`), such
    /// as the development gateway.
    pub endpoint: Option<String>,
    /// `KURIA_S3_REGION`, `us-east-1` by default.
    pub region: String,
    /// `KURIA_S3_ACCESS_KEY` and `KURIA_S3_SECRET_KEY`; without them, the
    /// standard AWS environment and instance credentials are used.
    pub credentials: Option<(String, String)>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0} must be set")]
    Missing(&'static str),
    #[error("{0} is invalid: {1}")]
    Invalid(&'static str, String),
}

impl Config {
    /// Reads the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads the configuration through `lookup`, which returns a variable's
    /// value or `None` when it is unset.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let required = |name: &'static str| {
            lookup(name)
                .filter(|value| !value.is_empty())
                .ok_or(ConfigError::Missing(name))
        };
        let listen = match lookup("KURIA_LISTEN") {
            Some(listen) => listen.parse().map_err(|e: std::net::AddrParseError| {
                ConfigError::Invalid("KURIA_LISTEN", e.to_string())
            })?,
            None => SocketAddr::from(([0, 0, 0, 0], 8080)),
        };
        let public_url = required("KURIA_PUBLIC_URL")?
            .trim_end_matches('/')
            .to_owned();
        let key = required("KONKLAV_SERVICE_KEY")?;
        let Some((key_id, secret)) = key.split_once('.') else {
            return Err(ConfigError::Invalid(
                "KONKLAV_SERVICE_KEY",
                "expected <key id>.<secret>".to_owned(),
            ));
        };
        let allowed_origins = match lookup("KURIA_ALLOWED_ORIGINS") {
            Some(origins) if !origins.trim().is_empty() => origins
                .split(',')
                .map(|o| o.trim().trim_end_matches('/').to_owned())
                .filter(|o| !o.is_empty())
                .collect(),
            _ => vec![origin(&public_url).ok_or_else(|| {
                ConfigError::Invalid("KURIA_PUBLIC_URL", "expected an absolute URL".to_owned())
            })?],
        };
        let secure_cookies = flag(&lookup, "KURIA_SECURE_COOKIES", true)?;
        let trusted_proxies = list(&lookup("KURIA_TRUSTED_PROXIES").unwrap_or_default());
        let storage = StorageConfig {
            bucket: required("KURIA_S3_BUCKET")?,
            endpoint: lookup("KURIA_S3_ENDPOINT").filter(|e| !e.is_empty()),
            region: lookup("KURIA_S3_REGION")
                .filter(|r| !r.is_empty())
                .unwrap_or_else(|| "us-east-1".to_owned()),
            credentials: match (lookup("KURIA_S3_ACCESS_KEY"), lookup("KURIA_S3_SECRET_KEY")) {
                (Some(key), Some(secret)) if !key.is_empty() && !secret.is_empty() => {
                    Some((key, secret))
                }
                _ => None,
            },
        };
        let attachment_limit = match lookup("KURIA_ATTACHMENT_LIMIT") {
            Some(limit) if !limit.is_empty() => limit.parse().map_err(|_| {
                ConfigError::Invalid(
                    "KURIA_ATTACHMENT_LIMIT",
                    "expected a number of bytes".to_owned(),
                )
            })?,
            _ => 100 * 1024 * 1024,
        };
        let render_limits = render_limits(&lookup)?;
        let archive_sweep = archive_sweep(&lookup)?;
        let default_tenant = match lookup("KURIA_DEFAULT_TENANT") {
            Some(tenant) if !tenant.is_empty() => Some(tenant.parse().map_err(|_| {
                ConfigError::Invalid("KURIA_DEFAULT_TENANT", "expected a tenant id".to_owned())
            })?),
            _ => None,
        };
        let login_app = flag(&lookup, "KURIA_LOGIN_APP", false)?;
        let rate_limit_secret = required("KURIA_RATE_LIMIT_SECRET")?;
        if rate_limit_secret.len() < 32 {
            return Err(ConfigError::Invalid(
                "KURIA_RATE_LIMIT_SECRET",
                "expected at least 32 bytes".to_owned(),
            ));
        }
        Ok(Self {
            listen,
            issuer: required("KONKLAV_ISSUER")?,
            audiences: list(&required("KONKLAV_AUDIENCE")?),
            relying_url: required("KONKLAV_RELYING_URL")?,
            subject_url: required("KONKLAV_SUBJECT_URL")?,
            key_id: key_id.to_owned(),
            secret: secret.to_owned(),
            database_url: required("DATABASE_URL")?,
            oidc_client_id: required("KURIA_OIDC_CLIENT_ID")?,
            oidc_client_secret: required("KURIA_OIDC_CLIENT_SECRET")?,
            session_key: required("KURIA_SESSION_KEY")?,
            allowed_origins,
            valkey_url: required("KURIA_VALKEY_URL")?,
            rate_limit_secret,
            trusted_proxies,
            storage,
            attachment_limit,
            render_limits,
            archive_sweep,
            default_tenant,
            web_dir: lookup("KURIA_WEB_DIR")
                .filter(|d| !d.is_empty())
                .map(std::path::PathBuf::from),
            login_app,
            secure_cookies,
            public_url,
        })
    }

    /// The relying client's configuration.
    #[must_use]
    pub fn client(&self) -> ClientConfig {
        ClientConfig {
            relying_url: self.relying_url.clone(),
            subject_url: self.subject_url.clone(),
            key_id: self.key_id.clone(),
            secret: self.secret.clone(),
            core: RelyingConfig {
                issuer: self.issuer.clone(),
                audiences: self.audiences.clone(),
                ..RelyingConfig::default()
            },
        }
    }
}

/// The origin of an absolute URL: its scheme and authority.
fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    (!scheme.is_empty() && !authority.is_empty()).then(|| format!("{scheme}://{authority}"))
}

/// A comma-separated list, without blanks.
fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

/// The variable `name` as `true` or `false`, `default` when it is unset or
/// empty.
fn flag(
    lookup: &impl Fn(&str) -> Option<String>,
    name: &'static str,
    default: bool,
) -> Result<bool, ConfigError> {
    match lookup(name).as_deref() {
        None | Some("") => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(other) => Err(ConfigError::Invalid(
            name,
            format!("expected true or false, not {other}"),
        )),
    }
}

/// How often archive rules are applied (`KURIA_ARCHIVE_SWEEP_SECONDS`,
/// 3600 by default).
// r[impl archive.sweep]
fn archive_sweep(
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<std::time::Duration, ConfigError> {
    match lookup("KURIA_ARCHIVE_SWEEP_SECONDS") {
        Some(value) if !value.is_empty() => value
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .map(std::time::Duration::from_secs)
            .ok_or_else(|| {
                ConfigError::Invalid(
                    "KURIA_ARCHIVE_SWEEP_SECONDS",
                    "expected a whole number above 0".to_owned(),
                )
            }),
        _ => Ok(std::time::Duration::from_secs(3600)),
    }
}

/// The limits rendering runs under, each a whole number from the
/// environment, else its default.
// r[impl render.limits]
fn render_limits(
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<kuria_typst::Limits, ConfigError> {
    let number = |name: &'static str, default: u64| -> Result<u64, ConfigError> {
        match lookup(name) {
            Some(value) if !value.is_empty() => {
                value.parse::<u64>().ok().filter(|n| *n > 0).ok_or_else(|| {
                    ConfigError::Invalid(name, "expected a whole number above 0".to_owned())
                })
            }
            _ => Ok(default),
        }
    };
    let defaults = kuria_typst::Limits::default();
    Ok(kuria_typst::Limits {
        wall: std::time::Duration::from_secs(number(
            "KURIA_RENDER_SECONDS",
            defaults.wall.as_secs(),
        )?),
        cpu_seconds: number("KURIA_RENDER_CPU_SECONDS", defaults.cpu_seconds)?,
        memory_bytes: number(
            "KURIA_RENDER_MEMORY_MB",
            defaults.memory_bytes / (1024 * 1024),
        )? * 1024
            * 1024,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn lookup(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| vars.get(name).cloned()
    }

    const COMPLETE: [(&str, &str); 13] = [
        ("KURIA_S3_BUCKET", "kuria"),
        ("KURIA_VALKEY_URL", "redis://localhost:6379"),
        (
            "KURIA_RATE_LIMIT_SECRET",
            "0123456789abcdef0123456789abcdef",
        ),
        ("KURIA_PUBLIC_URL", "https://kuria.example/app/"),
        ("DATABASE_URL", "postgres://localhost/kuria"),
        ("KURIA_OIDC_CLIENT_ID", "kuria"),
        ("KURIA_OIDC_CLIENT_SECRET", "client-secret"),
        (
            "KURIA_SESSION_KEY",
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
        ),
        ("KONKLAV_ISSUER", "https://auth.example"),
        ("KONKLAV_AUDIENCE", "kuria"),
        ("KONKLAV_RELYING_URL", "http://konklav:8081"),
        ("KONKLAV_SUBJECT_URL", "http://konklav:8080"),
        ("KONKLAV_SERVICE_KEY", "key-1.s3cret"),
    ];

    #[test]
    fn reads_a_complete_configuration() {
        let config = Config::from_lookup(lookup(&COMPLETE)).unwrap();
        assert_eq!(config.listen, SocketAddr::from(([0, 0, 0, 0], 8080)));
        assert_eq!(config.public_url, "https://kuria.example/app");
        assert_eq!(
            config.allowed_origins,
            vec!["https://kuria.example".to_owned()]
        );
        assert!(config.secure_cookies);
        assert_eq!(config.trusted_proxies, Vec::<String>::new());
        assert_eq!(config.key_id, "key-1");
        assert_eq!(config.secret, "s3cret");
        assert_eq!(config.client().core.audiences, vec!["kuria".to_owned()]);
        let mut vars = COMPLETE.to_vec();
        vars.retain(|(name, _)| *name != "KONKLAV_AUDIENCE");
        vars.push(("KONKLAV_AUDIENCE", "kuria, konklav"));
        assert_eq!(
            Config::from_lookup(lookup(&vars)).unwrap().audiences,
            ["kuria", "konklav"]
        );
    }

    #[test]
    fn names_a_missing_variable() {
        let vars: Vec<_> = COMPLETE
            .into_iter()
            .filter(|(name, _)| *name != "KONKLAV_ISSUER")
            .collect();
        let Err(error) = Config::from_lookup(lookup(&vars)) else {
            panic!("a configuration without KONKLAV_ISSUER");
        };
        assert_eq!(error.to_string(), "KONKLAV_ISSUER must be set");
    }

    #[test]
    fn refuses_a_service_key_without_a_secret() {
        let mut vars = COMPLETE.to_vec();
        vars.retain(|(name, _)| *name != "KONKLAV_SERVICE_KEY");
        vars.push(("KONKLAV_SERVICE_KEY", "key-1"));
        assert!(matches!(
            Config::from_lookup(lookup(&vars)),
            Err(ConfigError::Invalid("KONKLAV_SERVICE_KEY", _))
        ));
    }

    #[test]
    fn reads_the_app_the_login_page_and_the_default_tenant() {
        let config = Config::from_lookup(lookup(&COMPLETE)).unwrap();
        assert_eq!(
            (config.web_dir, config.login_app, config.default_tenant),
            (None, false, None)
        );
        let mut vars = COMPLETE.to_vec();
        vars.extend([
            ("KURIA_WEB_DIR", "/srv/kuria"),
            ("KURIA_LOGIN_APP", "true"),
            (
                "KURIA_DEFAULT_TENANT",
                "01900000-0000-7000-8000-000000000001",
            ),
        ]);
        let config = Config::from_lookup(lookup(&vars)).unwrap();
        assert_eq!(
            config.web_dir.unwrap(),
            std::path::PathBuf::from("/srv/kuria")
        );
        assert!(config.login_app);
        assert_eq!(
            config.default_tenant.unwrap().to_string(),
            "01900000-0000-7000-8000-000000000001"
        );
        for (name, value) in [("KURIA_LOGIN_APP", "yes"), ("KURIA_DEFAULT_TENANT", "shop")] {
            let mut vars = COMPLETE.to_vec();
            vars.push((name, value));
            assert!(
                matches!(Config::from_lookup(lookup(&vars)), Err(ConfigError::Invalid(n, _)) if n == name),
                "{name}={value}"
            );
        }
    }
}
