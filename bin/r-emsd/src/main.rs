//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "binary"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Binary entrypoint for the R-EMS daemon."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use r_ems_api::{spawn_api_server, ApiServer, ApiState};
use r_ems_common::config::{AppConfig, Mode};
use r_ems_common::license::{FeatureMatrix, LicenseTier, LicenseValidation, LicenseValidator};
use r_ems_common::logging::init_tracing;
use r_ems_common::version::VersionInfo;
use r_ems_config::{hash_app_config, load_active_manifest, DEFAULT_CONFIG_ROOT};
use r_ems_core::orchestrator::RemsOrchestrator;
use r_ems_core::update::UpdateClient;
use r_ems_metrics::{new_registry, spawn_http_server, DaemonMetrics, SharedRegistry};
use tokio::signal;
use tracing::{info, warn};

#[derive(Debug, Parser)]
#[command(
    author,
    disable_version_flag = true,
    version = concat!("R-EMS ", env!("CARGO_PKG_VERSION"), " (", env!("VERGEN_GIT_SHA"), ")"),
    about = "R-EMS daemon",
    long_about = None
)]
struct Cli {
    #[arg(long, value_name = "FILE", help = "Path to configuration file")]
    config: Option<PathBuf>,

    #[arg(long, help = "Allow license bypass in development mode")]
    dev_allow_license_bypass: bool,

    #[arg(
        short = 'V',
        long = "version",
        action = ArgAction::SetTrue,
        help = "Print extended version information and exit"
    )]
    version: bool,

    #[arg(long, value_enum, help = "Override application mode")]
    mode: Option<CliMode>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliMode {
    Production,
    Simulation,
    Hybrid,
}

impl From<CliMode> for Mode {
    fn from(value: CliMode) -> Self {
        match value {
            CliMode::Production => Mode::Production,
            CliMode::Simulation => Mode::Simulation,
            CliMode::Hybrid => Mode::Hybrid,
        }
    }
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Run the orchestrator")]
    Run,
    #[command(about = "Check for updates but do not apply them")]
    UpdateCheck,
    #[command(about = "Check and apply updates in development")]
    UpdateApply,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let version = VersionInfo::current();
    if cli.version {
        println!("{}", version.extended());
        return Ok(());
    }
    let mut candidates = Vec::new();
    if let Some(path) = &cli.config {
        candidates.push(path.clone());
    }
    candidates.push(PathBuf::from("configs/example.prod.toml"));
    candidates.push(PathBuf::from("configs/example.dev.toml"));

    let load_started = Instant::now();
    let loaded_config = AppConfig::load_with_source(&candidates)?;
    let mut config = loaded_config.config;
    let config_path = loaded_config.source;
    let load_duration = load_started.elapsed();
    let config_hash = hash_app_config(&config)?;
    let active_manifest = load_active_manifest(DEFAULT_CONFIG_ROOT).ok().flatten();

    let metrics_registry = new_registry();
    let daemon_metrics = DaemonMetrics::new(metrics_registry.clone())?;
    daemon_metrics.observe_config_load(load_duration.as_secs_f64());
    daemon_metrics.inc_start();
    daemon_metrics.set_build_info(&version.semver, &version.git_sha, &version.profile);

    if let Some(mode) = cli.mode {
        config.mode = mode.into();
    }
    init_tracing("r-emsd", &config.logging)?;

    if let Some(manifest) = &active_manifest {
        info!(installation = %manifest.installation.name, slug = %manifest.installation.slug, manifest_hash = %manifest.installation.config_hash, manifest_version = %manifest.installation.source_version, "active installation manifest detected");
    }
    info!(config_hash = %config_hash, "configuration loaded");

    let update_client = UpdateClient::from_config(&config.update, version.clone())?;

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            run_daemon(
                config,
                config_path,
                update_client,
                cli.dev_allow_license_bypass,
                Some(metrics_registry.clone()),
                version.clone(),
            )
            .await?
        }
        Commands::UpdateCheck => {
            let result = update_client.check().await?;
            render_update_result(&result);
        }
        Commands::UpdateApply => {
            let result = update_client.check().await?;
            render_update_result(&result);
            if result.update_available() {
                update_client.apply(&result).await?;
            } else {
                warn!("no update to apply");
            }
        }
    }

    Ok(())
}

async fn run_daemon(
    config: AppConfig,
    config_path: PathBuf,
    update_client: UpdateClient,
    bypass: bool,
    mut metrics_registry: Option<SharedRegistry>,
    version: VersionInfo,
) -> Result<()> {
    let metrics_settings = config.metrics.clone();
    let api_settings = config.api.clone();
    let validator = LicenseValidator::new(&config.license);
    let license = validator.validate(bypass)?;
    if let LicenseValidation::Bypassed { .. } = &license {
        warn!("license bypass engaged");
    }
    let feature_map = match &license {
        LicenseValidation::Valid(meta) => meta.features.to_map(),
        LicenseValidation::Bypassed { .. } => {
            FeatureMatrix::from_payload(&Vec::new(), LicenseTier::Development).to_map()
        }
    };

    let metrics_server = if metrics_settings.enabled {
        match metrics_registry.clone() {
            Some(registry) => {
                info!(address = %metrics_settings.listen, "metrics exporter enabled");
                Some(spawn_http_server(registry, metrics_settings.listen)?)
            }
            None => {
                warn!("metrics exporter requested but no registry available");
                None
            }
        }
    } else {
        metrics_registry = None;
        info!("metrics exporter disabled by configuration");
        None
    };

    let orchestrator =
        RemsOrchestrator::new(config, license, update_client, metrics_registry.clone());
    let handle = orchestrator.start().await?;

    let mut api_server: Option<ApiServer> = None;
    if api_settings.enabled {
        let static_dir = api_settings.static_dir.clone().and_then(|dir| {
            if dir.is_dir() {
                Some(dir)
            } else {
                warn!(static_dir = %dir.display(), "api static_dir not found; serving API without assets");
                None
            }
        });
        let log_directory = handle.config().logging.directory.clone();
        let state = Arc::new(ApiState::new(
            handle.config().clone(),
            handle.mode(),
            version.clone(),
            feature_map,
            config_path,
            log_directory,
            None,
        ));
        match spawn_api_server(state, api_settings.listen, static_dir) {
            Ok(server) => {
                info!(address = %server.addr(), "api server listening");
                api_server = Some(server);
            }
            Err(err) => {
                warn!(error = %err, "failed to start api server");
            }
        }
    } else {
        info!("api server disabled by configuration");
    }

    info!(mode = ?handle.mode(), "daemon running; waiting for termination signal");
    signal::ctrl_c().await?;
    info!("ctrl-c received; shutting down");
    handle.shutdown().await?;

    if let Some(server) = metrics_server {
        server.shutdown().await?;
    }

    if let Some(server) = api_server {
        server.shutdown().await?;
    }

    Ok(())
}

fn render_update_result(result: &r_ems_core::update::UpdateResult) {
    if let Some(latest) = &result.latest {
        info!(
            current = %result.current.semver,
            latest = %latest.version,
            available = result.update_available(),
            "update check complete"
        );
        if result.update_available() {
            info!(
                current = %result.current.semver,
                latest = %latest.version,
                "newer release detected"
            );
        }
        println!(
            "Current: {}\nLatest: {}\nUpdate Available: {}",
            result.current.cli_string(),
            latest.version,
            result.update_available()
        );
    } else {
        info!(current = %result.current.semver, "no updates found");
        println!("Current: {}\nLatest: none", result.current.cli_string());
    }
}
