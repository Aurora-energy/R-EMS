mod config;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use clap::{Parser, Subcommand};
use config::{load_config, validate_config, SystemConfig, ValidationReport};
use serde::Serialize;
use tokio::{net::TcpListener, signal};
use tracing::{info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const DEFAULT_ADDR: &str = "0.0.0.0:7300";
const DEFAULT_CONFIG_PATH: &str = "configs/system.yaml";
const DEFAULT_LOG_DIR: &str = "logs";

#[derive(Parser, Debug)]
#[command(
    name = "r-ems-configd",
    about = "R-EMS configuration distribution service"
)]
struct Cli {
    /// Path to the structured system configuration document.
    #[arg(long, env = "REMS_CONFIG_PATH", default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,

    /// Address to bind the HTTP API to.
    #[arg(long, env = "REMS_CONFIGD_BIND", default_value = DEFAULT_ADDR)]
    bind: SocketAddr,

    /// Directory where runtime logs should be written.
    #[arg(long, env = "REMS_LOG_DIR", default_value = DEFAULT_LOG_DIR)]
    log_dir: PathBuf,

    /// Optional command controlling startup behaviour. Defaults to `serve`.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
    /// Launch the HTTP API and serve the validated configuration.
    Serve,
    /// Perform validation checks and exit.
    Validate,
}

impl Default for Command {
    fn default() -> Self {
        Command::Serve
    }
}

#[derive(Clone)]
struct AppState {
    config: Arc<SystemConfig>,
    summary: ValidationReport,
}

#[derive(Serialize)]
struct SummaryResponse {
    grids: usize,
    controllers: usize,
    devices: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let _guard = init_tracing(&cli.log_dir)?;

    let command = cli.command.unwrap_or_default();

    match command {
        Command::Validate => {
            info!(path = %cli.config.display(), "validating configuration");
            let config = load_config(&cli.config)?;
            let summary = validate_config(&config)?;
            info!(
                grids = summary.grids,
                controllers = summary.controllers,
                devices = summary.devices,
                "configuration is valid"
            );
            return Ok(());
        }
        Command::Serve => {
            info!(path = %cli.config.display(), "loading configuration for service");
        }
    }

    let config = Arc::new(load_config(&cli.config)?);
    let summary = validate_config(&config)?;

    info!(
        grids = summary.grids,
        controllers = summary.controllers,
        devices = summary.devices,
        allow_interop = ?config
            .system
            .grids
            .iter()
            .map(|grid| (&grid.id, grid.allow_interop))
            .collect::<Vec<_>>(),
        "configuration loaded successfully"
    );

    let state = AppState {
        config: Arc::clone(&config),
        summary: summary.clone(),
    };

    let app = Router::new()
        .route("/api/config", get(get_config))
        .route("/api/config/summary", get(get_summary))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(state);

    info!(%cli.bind, "starting configd server");

    let listener = TcpListener::bind(cli.bind).await?;

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing(log_dir: &Path) -> anyhow::Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;

    let file_appender = tracing_appender::rolling::daily(log_dir, "configd.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let stdout_layer = fmt::layer().with_target(false);
    let file_layer = fmt::layer().with_writer(file_writer).with_ansi(false);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    Ok(guard)
}

async fn get_config(State(state): State<AppState>) -> Json<SystemConfig> {
    Json((*state.config).clone())
}

async fn get_summary(State(state): State<AppState>) -> Json<SummaryResponse> {
    Json(SummaryResponse {
        grids: state.summary.grids,
        controllers: state.summary.controllers,
        devices: state.summary.devices,
    })
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        tokio::select! {
            _ = ctrl_c() => {},
            _ = terminate() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c().await;
    }
}

async fn ctrl_c() {
    if let Err(err) = signal::ctrl_c().await {
        warn!(?err, "failed to install Ctrl+C handler");
    }
}

#[cfg(unix)]
async fn terminate() {
    use tokio::signal::unix::{signal, SignalKind};

    match signal(SignalKind::terminate()) {
        Ok(mut term) => {
            term.recv().await;
        }
        Err(err) => warn!(?err, "failed to install SIGTERM handler"),
    }
}

#[cfg(not(unix))]
async fn terminate() {}
