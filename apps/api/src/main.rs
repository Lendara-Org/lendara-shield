//! Binary entry point: configuration, tracing, database, HTTP server and
//! graceful shutdown. No business logic lives here.
//!
//! Usage: `shield-api` serves the API (applying pending migrations first);
//! `shield-api migrate` only applies migrations and exits.

use std::{net::SocketAddr, process::ExitCode};

use std::sync::Arc;

use shield_api::{
    HttpOptions, build_app, config::Config, db, deployment::PoolDeployment, http::InfoResponse,
    state::AppState, telemetry,
};
use tokio::net::TcpListener;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Serve,
    Migrate,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Tracing may not be initialised yet (e.g. invalid config), so
            // startup failures go straight to stderr.
            #[allow(clippy::print_stderr)]
            {
                eprintln!("shield-api: {err}");
            }
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), BoxError> {
    let command = match std::env::args().nth(1).as_deref() {
        None | Some("serve") => Command::Serve,
        Some("migrate") => Command::Migrate,
        Some(other) => {
            return Err(format!("unknown command `{other}` (expected serve or migrate)").into());
        }
    };

    let config = Config::from_env()?;
    telemetry::init(config.log_format)?;
    let deployment = PoolDeployment::load(&config.deployment_file)?;
    tracing::info!(pool = %deployment.pool, asset = %deployment.asset.code, "pool deployment loaded");

    let pool = db::connect(&config.database_url).await?;
    db::MIGRATOR.run(&pool).await?;
    tracing::info!("database migrations applied");
    if command == Command::Migrate {
        return Ok(());
    }

    let app = build_app(
        AppState {
            db: pool.clone(),
            info: Arc::new(InfoResponse::new(
                &deployment,
                config.ramp_anchor_home_domain,
            )),
        },
        HttpOptions {
            cors_allowed_origins: config.cors_allowed_origins,
        },
    );
    let addr = SocketAddr::from(([0, 0, 0, 0], config.api_port));
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "shield-api listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    pool.close().await;
    tracing::info!("shield-api stopped");
    Ok(())
}

/// Resolves on Ctrl+C or SIGTERM so in-flight requests can finish.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %err, "failed to listen for Ctrl+C");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received, draining connections");
}
