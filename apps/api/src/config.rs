//! Runtime configuration loaded from environment variables.
//!
//! Loading validates everything up front so the process fails fast at startup
//! instead of on the first request that needs a missing value.

use std::{fmt, path::PathBuf, str::FromStr};

use axum::http::HeaderValue;
use secrecy::SecretString;

const DEFAULT_API_PORT: u16 = 8080;
const DEFAULT_SOROBAN_RPC_URL: &str = "https://soroban-testnet.stellar.org";
const DEFAULT_HORIZON_URL: &str = "https://horizon-testnet.stellar.org";
/// Shortest accepted admin token, so it cannot be guessed.
const MIN_ADMIN_TOKEN_LEN: usize = 32;

/// Validated API configuration.
#[derive(Debug)]
pub struct Config {
    /// TCP port the HTTP server binds to on all interfaces.
    pub api_port: u16,
    /// Postgres connection string. Secret: it embeds the password.
    pub database_url: SecretString,
    /// Log output format.
    pub log_format: LogFormat,
    /// Origins allowed by CORS. Empty means no cross-origin access.
    pub cors_allowed_origins: Vec<HeaderValue>,
    /// SPP `deployments.json` written by `make setup-testnet`.
    pub deployment_file: PathBuf,
    /// SEP-1 home domain of the anchor. `None` disables ramp mode.
    pub ramp_anchor_home_domain: Option<String>,
    /// Bearer token for `/v1/asp/*`. `None` disables the admin endpoints.
    pub admin_token: Option<SecretString>,
    /// Stellar RPC used for Soroban calls.
    pub soroban_rpc_url: String,
    /// Horizon, to read the intake's payments.
    pub horizon_url: String,
    /// ASP admin (operator) seed. `None` leaves on-chain ASP writes off.
    pub operator_secret: Option<SecretString>,
    /// Intake (omnibus) account seed: SEP-10 with the anchor, pool deposits.
    pub intake_secret: Option<SecretString>,
    /// `apps/pool-cli` binary. `None` disables the shielding worker.
    pub pool_cli_path: Option<PathBuf>,
}

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// One JSON object per line, for production.
    Json,
    /// Human-readable, for local development.
    Pretty,
}

impl FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(Self::Json),
            "pretty" => Ok(Self::Pretty),
            other => Err(format!("expected `json` or `pretty`, got `{other}`")),
        }
    }
}

/// Why the configuration could not be loaded.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    Missing(&'static str),
    #[error("invalid value for {name}: {reason}")]
    Invalid { name: &'static str, reason: String },
}

impl Config {
    /// Loads the configuration from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Loads the configuration through `lookup`, so tests do not have to
    /// mutate the process environment. Empty values count as unset.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let get = |name: &str| lookup(name).filter(|value| !value.trim().is_empty());

        let api_port = parse_or("API_PORT", get("API_PORT"), DEFAULT_API_PORT)?;
        let database_url = get("DATABASE_URL")
            .map(SecretString::from)
            .ok_or(ConfigError::Missing("DATABASE_URL"))?;
        let log_format = parse_or("LOG_FORMAT", get("LOG_FORMAT"), LogFormat::Pretty)?;
        let cors_allowed_origins = get("CORS_ALLOWED_ORIGINS")
            .map(|raw| parse_origins(&raw))
            .transpose()?
            .unwrap_or_default();
        let deployment_file = get("SHIELD_DEPLOYMENT_FILE")
            .map(PathBuf::from)
            .ok_or(ConfigError::Missing("SHIELD_DEPLOYMENT_FILE"))?;
        let ramp_anchor_home_domain = get("RAMP_ANCHOR_HOME_DOMAIN")
            .map(|raw| parse_home_domain(raw.trim()))
            .transpose()?;
        let soroban_rpc_url = get("SOROBAN_RPC_URL")
            .map(|raw| parse_url("SOROBAN_RPC_URL", raw.trim()))
            .transpose()?
            .unwrap_or_else(|| DEFAULT_SOROBAN_RPC_URL.to_owned());
        let horizon_url = get("HORIZON_URL")
            .map(|raw| parse_url("HORIZON_URL", raw.trim()))
            .transpose()?
            .unwrap_or_else(|| DEFAULT_HORIZON_URL.to_owned());
        let operator_secret =
            get("SHIELD_OPERATOR_SECRET").map(|raw| SecretString::from(raw.trim().to_owned()));
        let intake_secret =
            get("SHIELD_INTAKE_SECRET").map(|raw| SecretString::from(raw.trim().to_owned()));
        let pool_cli_path = get("POOL_CLI_PATH").map(|raw| PathBuf::from(raw.trim()));
        let admin_token = get("ADMIN_TOKEN")
            .map(|raw| parse_admin_token(&raw))
            .transpose()?;

        Ok(Self {
            api_port,
            database_url,
            log_format,
            cors_allowed_origins,
            deployment_file,
            ramp_anchor_home_domain,
            admin_token,
            soroban_rpc_url,
            horizon_url,
            operator_secret,
            intake_secret,
            pool_cli_path,
        })
    }
}

fn parse_or<T>(name: &'static str, raw: Option<String>, default: T) -> Result<T, ConfigError>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    raw.map_or(Ok(default), |value| {
        value
            .trim()
            .parse()
            .map_err(|e: T::Err| ConfigError::Invalid {
                name,
                reason: e.to_string(),
            })
    })
}

fn parse_origins(raw: &str) -> Result<Vec<HeaderValue>, ConfigError> {
    raw.split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            if !(origin.starts_with("http://") || origin.starts_with("https://")) {
                return Err(ConfigError::Invalid {
                    name: "CORS_ALLOWED_ORIGINS",
                    reason: format!("`{origin}` must start with http:// or https://"),
                });
            }
            HeaderValue::from_str(origin).map_err(|e| ConfigError::Invalid {
                name: "CORS_ALLOWED_ORIGINS",
                reason: e.to_string(),
            })
        })
        .collect()
}

fn parse_url(name: &'static str, raw: &str) -> Result<String, ConfigError> {
    if raw.starts_with("https://") || raw.starts_with("http://") {
        Ok(raw.trim_end_matches('/').to_owned())
    } else {
        Err(ConfigError::Invalid {
            name,
            reason: format!("`{raw}` must start with http:// or https://"),
        })
    }
}

fn parse_admin_token(raw: &str) -> Result<SecretString, ConfigError> {
    let token = raw.trim();
    if token.len() < MIN_ADMIN_TOKEN_LEN {
        return Err(ConfigError::Invalid {
            name: "ADMIN_TOKEN",
            reason: format!("must be at least {MIN_ADMIN_TOKEN_LEN} characters"),
        });
    }
    Ok(SecretString::from(token.to_owned()))
}

/// A SEP-1 home domain is a bare host (`anchor.example` or `host:port`): the
/// client builds `https://<domain>/.well-known/stellar.toml` from it.
fn parse_home_domain(raw: &str) -> Result<String, ConfigError> {
    let valid = !raw.is_empty()
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
        && !raw.starts_with(['.', '-', ':']);
    if valid {
        Ok(raw.to_ascii_lowercase())
    } else {
        Err(ConfigError::Invalid {
            name: "RAMP_ANCHOR_HOME_DOMAIN",
            reason: format!(
                "`{raw}` must be a bare host such as anchor.example (no scheme or path)"
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use secrecy::ExposeSecret;

    use super::*;

    const DB_URL: &str = "postgres://shield:shield@localhost:5432/shield";

    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let env: HashMap<String, String> = [("SHIELD_DEPLOYMENT_FILE", "/tmp/deployments.json")]
            .iter()
            .chain(vars.iter())
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Config::from_lookup(|name| env.get(name).cloned())
    }

    #[test]
    fn applies_defaults_when_only_database_url_is_set() -> Result<(), ConfigError> {
        let config = load(&[("DATABASE_URL", DB_URL)])?;

        assert_eq!(config.api_port, 8080);
        assert_eq!(config.database_url.expose_secret(), DB_URL);
        assert_eq!(config.log_format, LogFormat::Pretty);
        assert!(config.cors_allowed_origins.is_empty());
        Ok(())
    }

    #[test]
    fn reads_every_variable() -> Result<(), ConfigError> {
        let config = load(&[
            ("DATABASE_URL", DB_URL),
            ("API_PORT", "9090"),
            ("LOG_FORMAT", "json"),
            (
                "CORS_ALLOWED_ORIGINS",
                "http://localhost:3000, https://shield.example",
            ),
        ])?;

        assert_eq!(config.api_port, 9090);
        assert_eq!(config.log_format, LogFormat::Json);
        assert_eq!(
            config.cors_allowed_origins,
            vec![
                HeaderValue::from_static("http://localhost:3000"),
                HeaderValue::from_static("https://shield.example"),
            ]
        );
        Ok(())
    }

    #[test]
    fn requires_the_deployment_file() {
        let env: HashMap<&str, &str> = [("DATABASE_URL", DB_URL)].into_iter().collect();
        let err = Config::from_lookup(|name| env.get(name).map(|v| (*v).to_owned())).err();
        assert_eq!(err, Some(ConfigError::Missing("SHIELD_DEPLOYMENT_FILE")));
    }

    #[test]
    fn ramp_mode_is_off_unless_an_anchor_is_configured() -> Result<(), ConfigError> {
        assert_eq!(
            load(&[("DATABASE_URL", DB_URL)])?.ramp_anchor_home_domain,
            None
        );
        let config = load(&[
            ("DATABASE_URL", DB_URL),
            (
                "RAMP_ANCHOR_HOME_DOMAIN",
                " Lendara-Monorepo-fixh.vercel.app ",
            ),
        ])?;
        assert_eq!(
            config.ramp_anchor_home_domain.as_deref(),
            Some("lendara-monorepo-fixh.vercel.app")
        );
        assert!(
            load(&[
                ("DATABASE_URL", DB_URL),
                ("RAMP_ANCHOR_HOME_DOMAIN", "localhost:8000")
            ])
            .is_ok()
        );
        Ok(())
    }

    #[test]
    fn rejects_an_anchor_with_scheme_or_path() {
        for raw in [
            "https://anchor.example",
            "anchor.example/sep24",
            ".anchor.example",
        ] {
            let err = load(&[("DATABASE_URL", DB_URL), ("RAMP_ANCHOR_HOME_DOMAIN", raw)]).err();
            assert!(
                matches!(
                    err,
                    Some(ConfigError::Invalid {
                        name: "RAMP_ANCHOR_HOME_DOMAIN",
                        ..
                    })
                ),
                "{raw}: {err:?}"
            );
        }
    }

    #[test]
    fn admin_token_is_optional_but_must_be_long() -> Result<(), ConfigError> {
        assert!(load(&[("DATABASE_URL", DB_URL)])?.admin_token.is_none());

        let token = "x".repeat(32);
        let config = load(&[("DATABASE_URL", DB_URL), ("ADMIN_TOKEN", &token)])?;
        assert_eq!(
            config.admin_token.as_ref().map(ExposeSecret::expose_secret),
            Some(token.as_str())
        );
        assert!(
            !format!("{config:?}").contains(&token),
            "token must not leak in Debug"
        );

        let err = load(&[("DATABASE_URL", DB_URL), ("ADMIN_TOKEN", "short")]).err();
        assert!(matches!(
            err,
            Some(ConfigError::Invalid {
                name: "ADMIN_TOKEN",
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn soroban_settings_default_to_testnet_and_keep_the_seed_secret() -> Result<(), ConfigError> {
        let config = load(&[("DATABASE_URL", DB_URL)])?;
        assert_eq!(
            config.soroban_rpc_url,
            "https://soroban-testnet.stellar.org"
        );
        assert_eq!(config.horizon_url, "https://horizon-testnet.stellar.org");
        assert!(config.operator_secret.is_none());

        let seed = "test-operator-secret-not-a-real-seed";
        let config = load(&[
            ("DATABASE_URL", DB_URL),
            ("SOROBAN_RPC_URL", "http://localhost:8000/"),
            ("SHIELD_OPERATOR_SECRET", seed),
        ])?;
        assert_eq!(config.soroban_rpc_url, "http://localhost:8000");
        assert!(!format!("{config:?}").contains(seed));

        let err = load(&[("DATABASE_URL", DB_URL), ("SOROBAN_RPC_URL", "rpc.example")]).err();
        assert!(matches!(
            err,
            Some(ConfigError::Invalid {
                name: "SOROBAN_RPC_URL",
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn rejects_missing_database_url() {
        assert_eq!(load(&[]).err(), Some(ConfigError::Missing("DATABASE_URL")));
    }

    #[test]
    fn treats_blank_values_as_unset() {
        assert_eq!(
            load(&[("DATABASE_URL", "   ")]).err(),
            Some(ConfigError::Missing("DATABASE_URL"))
        );
    }

    #[test]
    fn rejects_invalid_port() {
        for port in ["abc", "-1", "70000"] {
            let err = load(&[("DATABASE_URL", DB_URL), ("API_PORT", port)]).err();
            assert!(
                matches!(
                    err,
                    Some(ConfigError::Invalid {
                        name: "API_PORT",
                        ..
                    })
                ),
                "port {port}: {err:?}"
            );
        }
    }

    #[test]
    fn rejects_unknown_log_format() {
        let err = load(&[("DATABASE_URL", DB_URL), ("LOG_FORMAT", "xml")]).err();
        assert!(matches!(
            err,
            Some(ConfigError::Invalid {
                name: "LOG_FORMAT",
                ..
            })
        ));
    }

    #[test]
    fn rejects_origin_without_scheme() {
        let err = load(&[
            ("DATABASE_URL", DB_URL),
            ("CORS_ALLOWED_ORIGINS", "localhost:3000"),
        ])
        .err();
        assert!(matches!(
            err,
            Some(ConfigError::Invalid {
                name: "CORS_ALLOWED_ORIGINS",
                ..
            })
        ));
    }

    #[test]
    fn debug_output_never_contains_the_database_url() -> Result<(), ConfigError> {
        let config = load(&[("DATABASE_URL", DB_URL)])?;
        assert!(!format!("{config:?}").contains("shield:shield"));
        Ok(())
    }
}
