//! Tracing setup.
//!
//! Log hygiene (AGENTS.md §5.2): never log view keys, notes, note secrets or
//! the pair "user address + amount". Log the intent id instead.

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::config::LogFormat;

const DEFAULT_FILTER: &str = "info,shield_api=debug,tower_http=info,sqlx=warn";

/// Installs the global tracing subscriber. `RUST_LOG` overrides the filter.
pub fn init(format: LogFormat) -> Result<(), tracing_subscriber::util::TryInitError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let registry = tracing_subscriber::registry().with(filter);
    match format {
        LogFormat::Json => registry
            .with(
                fmt::layer()
                    .json()
                    .flatten_event(true)
                    .with_current_span(true),
            )
            .try_init(),
        LogFormat::Pretty => registry.with(fmt::layer()).try_init(),
    }
}
