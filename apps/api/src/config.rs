//! Runtime configuration loaded from environment variables.
//!
//! Loading validates everything up front so the process fails fast at startup
//! instead of on the first request that needs a missing value.

use std::{fmt, str::FromStr};

use axum::http::HeaderValue;
use secrecy::SecretString;

const DEFAULT_API_PORT: u16 = 8080;

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

        Ok(Self {
            api_port,
            database_url,
            log_format,
            cors_allowed_origins,
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use secrecy::ExposeSecret;

    use super::*;

    const DB_URL: &str = "postgres://shield:shield@localhost:5432/shield";

    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let env: HashMap<String, String> = vars
            .iter()
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
