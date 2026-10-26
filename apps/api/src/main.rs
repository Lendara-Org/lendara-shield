//! Binary entry point: configuration, tracing, database, HTTP server and
//! graceful shutdown. No business logic lives here.
//!
//! Usage: `shield-api` serves the API (applying pending migrations first);
//! `shield-api migrate` only applies migrations and exits;
//! `shield-api init-intake` derives the intake's pool keys and allow-lists
//! it if needed (see `shield_api::intake`), then exits.

use std::{net::SocketAddr, process::ExitCode, sync::Arc};

use shield_api::{
    HttpOptions,
    adapters::{
        account_of,
        anchor::{Sep24AnchorClient, Sep24AnchorConfig},
        explorer::HorizonExplorer,
        horizon::HorizonLedger,
        pool_cli::{self, CliPoolClient},
        soroban::{DEFAULT_VALIDITY, SorobanAspConfig, SorobanAspContract, rpc::SorobanRpc},
    },
    asp::service::AspService,
    audit::AuditService,
    background::{self, Background},
    build_app,
    config::Config,
    db,
    deployment::PoolDeployment,
    http::InfoResponse,
    intake::{self, IntakeInit},
    intent::{
        anchor_sync::AnchorSync, matching::ExpectedAsset, service::IntentService,
        shielding::ShieldingWorker, watcher::IntakeWatcher,
    },
    ports::{
        AnchorClient, AspContract, Clock, Explorer, PoolAuditor, SystemClock,
        UnconfiguredAnchorClient, UnconfiguredAspContract,
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
    InitIntake,
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
        Some("init-intake") => Command::InitIntake,
        Some(other) => {
            return Err(format!(
                "unknown command `{other}` (expected serve, migrate or init-intake)"
            )
            .into());
        }
    };

    let config = Config::from_env()?;
    telemetry::init(config.log_format)?;
    let deployment = PoolDeployment::load(&config.deployment_file)?;
    tracing::info!(pool = %deployment.pool, asset = %deployment.asset.code, "pool deployment loaded");

    if command == Command::InitIntake {
        return init_intake(&config, &deployment).await;
    }
    if config.admin_token.is_none() {
        tracing::warn!("ADMIN_TOKEN is not set: /v1/asp/* answers 401");
    }

    let pool = db::connect(config.database_url()?).await?;
    db::MIGRATOR.run(&pool).await?;
    tracing::info!("database migrations applied");
    if command == Command::Migrate {
        return Ok(());
    }

    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let asp_contract = asp_contract(&config, &deployment, clock.clone())?;
    let anchor = anchor_client(&config, &deployment, clock.clone())?;
    // `pool-cli` runs for the worker and the auditor share the intake's wallet.
    let wallet = Arc::new(tokio::sync::Mutex::new(()));
    let background = background_loop(
        &config,
        &deployment,
        &pool,
        anchor.as_ref(),
        &clock,
        &wallet,
    )?;

    let app = build_app(
        AppState {
            db: pool.clone(),
            info: Arc::new(InfoResponse::new(
                &deployment,
                config.ramp_anchor_home_domain.clone(),
            )),
            asp: AspService::new(pool.clone(), asp_contract, clock.clone()),
            intents: IntentService::new(pool.clone(), anchor, deployment.asset.code.clone()),
            audit: audit_service(&config, &deployment, &pool, &wallet)?,
            admin_token: Arc::new(config.admin_token),
            auditor_token: Arc::new(config.auditor_token),
            clock: clock.clone(),
        },
        HttpOptions {
            cors_allowed_origins: config.cors_allowed_origins,
            rate_limit: config.rate_limit,
        },
    );
    let addr = SocketAddr::from(([0, 0, 0, 0], config.api_port));
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "shield-api listening");

    let shutdown = CancellationToken::new();
    let background = background.map(|b| tokio::spawn(b.run(background::TICK, shutdown.clone())));

    let stop = shutdown.clone();
    // The peer address keys the rate limit.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        stop.cancel();
    })
    .await?;

    shutdown.cancel();
    if let Some(handle) = background {
        // Waits for the loops to stop. A round still running is dropped: its
        // open database transaction rolls back and `pool-cli` is killed, and
        // every step stored its state before any network call.
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
    wallet: &Arc<tokio::sync::Mutex<()>>,
) -> Result<Option<Background>, BoxError> {
    let (Some(anchor), Some(secret)) = (anchor, config.intake_secret.as_ref()) else {
        return Ok(None);
    };
    let intake = account_of(secret).ok_or("SHIELD_INTAKE_SECRET is not a valid secret seed")?;
    let ledger = HorizonLedger::new(&format!("{}/", config.horizon_url), intake.clone())?;
    let asset = ExpectedAsset {
        code: deployment.asset.code.clone(),
        issuer: deployment.asset.issuer.as_str().to_owned(),
    };
    tracing::info!("intake watcher and anchor sync enabled");
    let shielding = shielding_worker(config, deployment, pool, &intake, clock, wallet)?;
    Ok(Some(Background {
        watcher: IntakeWatcher::new(pool.clone(), Arc::new(ledger), asset),
        anchor_sync: AnchorSync::new(pool.clone(), anchor.clone(), clock.clone()),
        shielding,
    }))
}

/// The shielding worker over `apps/pool-cli`, when `POOL_CLI_PATH` is set.
fn shielding_worker(
    config: &Config,
    deployment: &PoolDeployment,
    pool: &sqlx::PgPool,
    intake: &shield_api::domain::AccountId,
    clock: &Arc<dyn Clock>,
    wallet: &Arc<tokio::sync::Mutex<()>>,
) -> Result<Option<ShieldingWorker>, BoxError> {
    let Some(binary) = config.pool_cli_path.clone() else {
        tracing::warn!("POOL_CLI_PATH is not set: funded intents will not be shielded");
        return Ok(None);
    };
    let pool_client = CliPoolClient::new(
        binary.clone(),
        CliPoolClient::forwarded_env(),
        pool_cli::DEFAULT_TIMEOUT,
        SorobanRpc::new(config.soroban_rpc_url.clone())?,
        stellar_strkey::ed25519::PublicKey::from_string(intake.as_str())?.0,
    )
    .sharing_wallet(wallet.clone());
    tracing::info!(binary = %binary.display(), "shielding worker enabled");
    tokio::spawn(check_intake(pool_client.clone(), deployment.clone()));
    Ok(Some(ShieldingWorker::new(
        pool.clone(),
        Arc::new(pool_client),
        clock.clone(),
    )))
}

/// Derives the intake's pool keys if missing and checks it is allow-listed:
/// without either, every funded intent fails to shield and ends in `error`.
/// Logs only; `scripts/init-intake.sh` fixes both.
async fn check_intake(client: CliPoolClient, deployment: PoolDeployment) {
    match client
        .check_intake(&deployment.asp_membership, deployment.deployment_ledger)
        .await
    {
        Ok(check) if check.allow_listed => {
            tracing::info!(asp_leaf = %check.asp_leaf, "intake ready: pool keys derived and allow-listed");
        }
        Ok(check) => tracing::error!(
            asp_leaf = %check.asp_leaf,
            "the intake is NOT in the allow-list: pool deposits will fail; run scripts/init-intake.sh"
        ),
        Err(err) => tracing::error!(
            code = %err.code,
            message = %err.message,
            "cannot check the intake (pool keys / allow-list)"
        ),
    }
}

/// `shield-api init-intake`: needs `POOL_CLI_PATH`, the intake and the
/// operator (who signs `insert_leaf`). No database.
async fn init_intake(config: &Config, deployment: &PoolDeployment) -> Result<(), BoxError> {
    let (Some(binary), Some(secret)) =
        (config.pool_cli_path.clone(), config.intake_secret.as_ref())
    else {
        return Err("init-intake needs POOL_CLI_PATH and SHIELD_INTAKE_SECRET".into());
    };
    if config.operator_secret.is_none() {
        return Err("init-intake needs SHIELD_OPERATOR_SECRET (it signs insert_leaf)".into());
    }
    let intake = account_of(secret).ok_or("SHIELD_INTAKE_SECRET is not a valid secret seed")?;
    let client = CliPoolClient::new(
        binary,
        CliPoolClient::forwarded_env(),
        pool_cli::DEFAULT_TIMEOUT,
        SorobanRpc::new(config.soroban_rpc_url.clone())?,
        stellar_strkey::ed25519::PublicKey::from_string(intake.as_str())?.0,
    );
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let asp = asp_contract(config, deployment, clock)?;
    match intake::init_intake(
        &client,
        asp.as_ref(),
        &deployment.asp_membership,
        deployment.deployment_ledger,
        intake::Confirmation::default(),
    )
    .await?
    {
        IntakeInit::AlreadyAllowListed { asp_leaf } => {
            tracing::info!(%intake, %asp_leaf, "intake ready: pool keys derived, already allow-listed");
        }
        IntakeInit::Inserted { asp_leaf, tx_hash } => {
            tracing::info!(%intake, %asp_leaf, %tx_hash, "intake ready: pool keys derived, leaf inserted into the allow-list");
        }
    }
    Ok(())
}

/// The auditor's view (`pool-cli audit`, needs `POOL_CLI_PATH`, the intake
/// and `AUDITOR_TOKEN`) and the public view (Horizon, always on).
fn audit_service(
    config: &Config,
    deployment: &PoolDeployment,
    pool: &sqlx::PgPool,
    wallet: &Arc<tokio::sync::Mutex<()>>,
) -> Result<AuditService, BoxError> {
    let explorer: Arc<dyn Explorer> = Arc::new(HorizonExplorer::new(
        &format!("{}/", config.horizon_url),
        deployment.pool.clone(),
    )?);
    let intake = config.intake_secret.as_ref().and_then(account_of);
    let (Some(binary), Some(intake), Some(_)) =
        (&config.pool_cli_path, intake, &config.auditor_token)
    else {
        tracing::warn!(
            "auditor disabled: needs POOL_CLI_PATH, SHIELD_INTAKE_SECRET and AUDITOR_TOKEN"
        );
        return Ok(AuditService::new(pool.clone(), None, Some(explorer)));
    };
    tracing::info!("auditor enabled");
    let auditor: Arc<dyn PoolAuditor> = Arc::new(
        CliPoolClient::new(
            binary.clone(),
            CliPoolClient::auditor_env(),
            pool_cli::DEFAULT_TIMEOUT,
            SorobanRpc::new(config.soroban_rpc_url.clone())?,
            stellar_strkey::ed25519::PublicKey::from_string(intake.as_str())?.0,
        )
        .sharing_wallet(wallet.clone()),
    );
    Ok(AuditService::new(
        pool.clone(),
        Some(auditor),
        Some(explorer),
    ))
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
