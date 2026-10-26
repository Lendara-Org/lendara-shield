//! Binary entry point: configuration, tracing, database, HTTP server and
//! graceful shutdown. No business logic lives here.
//!
//! Usage: `shield-api` serves the API (applying pending migrations first);
//! `shield-api migrate` only applies migrations and exits.

use std::{net::SocketAddr, process::ExitCode, sync::Arc};

use shield_api::{
    HttpOptions,
    adapters::{
        account_of,
        anchor::{Sep24AnchorClient, Sep24AnchorConfig},
        horizon::HorizonLedger,
        soroban::{DEFAULT_VALIDITY, SorobanAspConfig, SorobanAspContract},
    },
    asp::service::AspService,
    background::{self, Background},
    build_app,
    config::Config,
    db,
    deployment::PoolDeployment,
    http::InfoResponse,
    intent::{
        anchor_sync::AnchorSync, matching::ExpectedAsset, service::IntentService,
        watcher::IntakeWatcher,
    },
    ports::{
        AnchorClient, AspContract, Clock, SystemClock, UnconfiguredAnchorClient,
        UnconfiguredAspContract,
    },
    state::AppState,
    telemetry,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

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
    if config.admin_token.is_none() {
        tracing::warn!("ADMIN_TOKEN is not set: /v1/asp/* answers 401");
    }
    let deployment = PoolDeployment::load(&config.deployment_file)?;
    tracing::info!(pool = %deployment.pool, asset = %deployment.asset.code, "pool deployment loaded");

    let pool = db::connect(&config.database_url).await?;
    db::MIGRATOR.run(&pool).await?;
    tracing::info!("database migrations applied");
    if command == Command::Migrate {
        return Ok(());
    }

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let asp_contract = asp_contract(&config, &deployment, clock.clone())?;
    let anchor = anchor_client(&config, &deployment, clock.clone())?;
    let background = background_loop(&config, &deployment, &pool, anchor.as_ref(), &clock)?;

    let app = build_app(
        AppState {
            db: pool.clone(),
            info: Arc::new(InfoResponse::new(
                &deployment,
                config.ramp_anchor_home_domain,
            )),
            asp: AspService::new(pool.clone(), asp_contract, clock.clone()),
            intents: IntentService::new(pool.clone(), anchor, deployment.asset.code.clone()),
            admin_token: Arc::new(config.admin_token),
        },
        HttpOptions {
            cors_allowed_origins: config.cors_allowed_origins,
        },
    );
    let addr = SocketAddr::from(([0, 0, 0, 0], config.api_port));
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "shield-api listening");

    let shutdown = CancellationToken::new();
    let background = background.map(|b| tokio::spawn(b.run(background::TICK, shutdown.clone())));

    let stop = shutdown.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            stop.cancel();
        })
        .await?;

    shutdown.cancel();
    if let Some(handle) = background {
        // Lets the current round finish its database transaction.
        if let Err(err) = handle.await {
            tracing::error!(error = %err, "background loop panicked");
        }
    }
    pool.close().await;
    tracing::info!("shield-api stopped");
    Ok(())
}

/// The Soroban ASP adapter when the operator seed is configured; otherwise a
/// placeholder that keeps users `pending`.
fn asp_contract(
    config: &Config,
    deployment: &PoolDeployment,
    clock: Arc<dyn Clock>,
) -> Result<Arc<dyn AspContract>, BoxError> {
    let Some(operator_secret) = config.operator_secret.clone() else {
        tracing::warn!("SHIELD_OPERATOR_SECRET is not set: ASP writes are off, users stay pending");
        return Ok(Arc::new(UnconfiguredAspContract));
    };
    let contract = SorobanAspContract::new(
        SorobanAspConfig {
            rpc_url: config.soroban_rpc_url.clone(),
            network_passphrase: TESTNET_PASSPHRASE.to_owned(),
            operator_secret,
            asp_membership: deployment.asp_membership.clone(),
            asp_non_membership: deployment.asp_non_membership.clone(),
            validity: DEFAULT_VALIDITY,
        },
        clock,
    )?;
    tracing::info!(operator = %contract.operator_address(), "ASP writes signed by the operator");
    Ok(Arc::new(contract))
}

/// The ramp's background loop (intake watcher + anchor sync), when ramp
/// mode talks to a real anchor.
fn background_loop(
    config: &Config,
    deployment: &PoolDeployment,
    pool: &sqlx::PgPool,
    anchor: Option<&Arc<dyn AnchorClient>>,
    clock: &Arc<dyn Clock>,
) -> Result<Option<Background>, BoxError> {
    let (Some(anchor), Some(secret)) = (anchor, config.intake_secret.as_ref()) else {
        return Ok(None);
    };
    let intake = account_of(secret).ok_or("SHIELD_INTAKE_SECRET is not a valid secret seed")?;
    let ledger = HorizonLedger::new(&format!("{}/", config.horizon_url), intake)?;
    let asset = ExpectedAsset {
        code: deployment.asset.code.clone(),
        issuer: deployment.asset.issuer.as_str().to_owned(),
    };
    tracing::info!("intake watcher and anchor sync enabled");
    Ok(Some(Background {
        watcher: IntakeWatcher::new(pool.clone(), Arc::new(ledger), asset),
        anchor_sync: AnchorSync::new(pool.clone(), anchor.clone(), clock.clone()),
    }))
}

/// The SEP-24 anchor client when ramp mode is configured; `None` disables
/// ramp mode.
fn anchor_client(
    config: &Config,
    deployment: &PoolDeployment,
    clock: Arc<dyn Clock>,
) -> Result<Option<Arc<dyn AnchorClient>>, BoxError> {
    let Some(home_domain) = config.ramp_anchor_home_domain.clone() else {
        tracing::info!("RAMP_ANCHOR_HOME_DOMAIN is not set: ramp mode is disabled");
        return Ok(None);
    };
    let Some(intake_secret) = config.intake_secret.clone() else {
        tracing::warn!("SHIELD_INTAKE_SECRET is not set: ramp intents cannot reach the anchor");
        return Ok(Some(Arc::new(UnconfiguredAnchorClient { home_domain })));
    };
    let client = Sep24AnchorClient::new(
        Sep24AnchorConfig {
            home_domain,
            intake_secret,
            asset_issuer: deployment.asset.issuer.clone(),
            network_passphrase: TESTNET_PASSPHRASE.to_owned(),
        },
        clock,
    )?;
    tracing::info!(anchor = client.home_domain(), "ramp mode enabled");
    Ok(Some(Arc::new(client)))
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
