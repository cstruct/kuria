//! Profiles: the Kuria URL and tenant `kuria login` signed in to, saved in
//! `config.toml` in the client's directory, and which one a command uses.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::CliError;

/// The profile a login names none for.
pub const DEFAULT_PROFILE: &str = "default";

/// The client's directory: `KURIA_CONFIG_DIR`, else `$XDG_CONFIG_HOME/kuria`,
/// else `~/.config/kuria`.
pub fn config_dir() -> Result<PathBuf, CliError> {
    let set = |var: &str| {
        std::env::var_os(var)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = set("KURIA_CONFIG_DIR") {
        return Ok(dir);
    }
    if let Some(dir) = set("XDG_CONFIG_HOME") {
        return Ok(dir.join("kuria"));
    }
    set("HOME")
        .map(|home| home.join(".config").join("kuria"))
        .ok_or_else(|| CliError::refused("NO_CONFIG_DIR", "Set HOME or KURIA_CONFIG_DIR"))
}

/// A saved profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub url: String,
    pub tenant: Option<Uuid>,
}

/// `config.toml`.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The profile commands use when not told otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

impl Config {
    fn path(dir: &Path) -> PathBuf {
        dir.join("config.toml")
    }

    /// The config in `dir`, or an empty one.
    pub fn load(dir: &Path) -> Result<Self, CliError> {
        match std::fs::read_to_string(Self::path(dir)) {
            Ok(text) => toml::from_str(&text).map_err(|e| {
                CliError::refused(
                    "INVALID_CONFIG",
                    format!("{} isn't valid: {e}", Self::path(dir).display()),
                )
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(io_error(&Self::path(dir), &e)),
        }
    }

    pub fn save(&self, dir: &Path) -> Result<(), CliError> {
        std::fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
        let text = toml::to_string_pretty(self).expect("serializable");
        let path = Self::path(dir);
        std::fs::write(&path, text).map_err(|e| io_error(&path, &e))
    }

    /// The name of the profile `asked` names, else the default one.
    #[must_use]
    pub fn chosen<'a>(&'a self, asked: Option<&'a str>) -> &'a str {
        asked.or(self.default.as_deref()).unwrap_or(DEFAULT_PROFILE)
    }
}

#[must_use]
pub fn io_error(path: &Path, error: &std::io::Error) -> CliError {
    CliError::refused("IO", format!("{}: {error}", path.display()))
}

/// What a command is for: the profile it signs in with, and the Kuria and
/// tenant it addresses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub profile: String,
    pub url: Option<String>,
    pub tenant: Option<Uuid>,
}

/// Where a command's URL and tenant come from: the flags, else the
/// environment, else the profile.
#[derive(Clone, Debug, Default)]
pub struct Asked {
    pub profile: Option<String>,
    pub url: Option<String>,
    pub tenant: Option<Uuid>,
    pub env_url: Option<String>,
    pub env_tenant: Option<Uuid>,
}

impl Asked {
    // r[impl cli.profile]
    #[must_use]
    pub fn resolve(&self, config: &Config) -> Target {
        let profile = config.chosen(self.profile.as_deref()).to_owned();
        let saved = config.profiles.get(&profile);
        Target {
            url: self
                .url
                .clone()
                .or_else(|| self.env_url.clone())
                .or_else(|| saved.map(|p| p.url.clone()))
                .map(|url| url.trim_end_matches('/').to_owned()),
            tenant: self
                .tenant
                .or(self.env_tenant)
                .or_else(|| saved.and_then(|p| p.tenant)),
            profile,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Uuid = Uuid::from_u128(1);
    const B: Uuid = Uuid::from_u128(2);
    const C: Uuid = Uuid::from_u128(3);

    fn config() -> Config {
        Config {
            default: Some("work".to_owned()),
            profiles: BTreeMap::from([
                (
                    "work".to_owned(),
                    Profile {
                        url: "https://kuria.work/".to_owned(),
                        tenant: Some(A),
                    },
                ),
                (
                    "home".to_owned(),
                    Profile {
                        url: "https://kuria.home".to_owned(),
                        tenant: None,
                    },
                ),
            ]),
        }
    }

    // r[verify cli.profile]
    #[test]
    fn flags_win_over_the_environment_and_the_environment_over_the_profile() {
        let config = config();
        let saved = Asked::default().resolve(&config);
        assert_eq!(
            saved,
            Target {
                profile: "work".to_owned(),
                url: Some("https://kuria.work".to_owned()),
                tenant: Some(A),
            }
        );
        let env = Asked {
            env_url: Some("https://env".to_owned()),
            env_tenant: Some(B),
            ..Asked::default()
        };
        assert_eq!(env.resolve(&config).url.as_deref(), Some("https://env"));
        assert_eq!(env.resolve(&config).tenant, Some(B));
        let flags = Asked {
            url: Some("https://flag".to_owned()),
            tenant: Some(C),
            ..env
        };
        assert_eq!(flags.resolve(&config).url.as_deref(), Some("https://flag"));
        assert_eq!(flags.resolve(&config).tenant, Some(C));
    }

    // r[verify cli.profile]
    #[test]
    fn a_named_profile_is_used_instead_of_the_default() {
        let asked = Asked {
            profile: Some("home".to_owned()),
            ..Asked::default()
        };
        let target = asked.resolve(&config());
        assert_eq!(target.url.as_deref(), Some("https://kuria.home"));
        assert_eq!(target.tenant, None);
        assert_eq!(
            Asked::default().resolve(&Config::default()).profile,
            "default"
        );
    }

    #[test]
    fn saves_and_loads_the_config() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Config::load(dir.path()).unwrap(), Config::default());
        config().save(dir.path()).unwrap();
        assert_eq!(Config::load(dir.path()).unwrap(), config());
    }
}
