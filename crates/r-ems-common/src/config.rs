//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Shared primitives and utilities for the core runtime."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationSeconds};
use tracing::debug;

use crate::logging::LogFormat;

fn default_mode() -> Mode {
    Mode::Production
}

fn default_env_license_var() -> String {
    "R_EMS_LICENSE".to_owned()
}

fn default_heartbeat_interval() -> Duration {
    Duration::from_millis(500)
}

fn default_watchdog_timeout() -> Duration {
    Duration::from_secs(2)
}

fn default_snapshot_path() -> PathBuf {
    PathBuf::from("target/snapshots")
}

fn default_logging_directory() -> PathBuf {
    PathBuf::from("target/logs")
}

fn default_log_format() -> LogFormat {
    LogFormat::StructuredJson
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_listen() -> SocketAddr {
    "0.0.0.0:9898"
        .parse()
        .expect("valid default metrics address")
}

fn default_api_enabled() -> bool {
    true
}

fn default_api_listen() -> SocketAddr {
    "0.0.0.0:8080".parse().expect("valid default api address")
}

fn default_update_feed() -> PathBuf {
    PathBuf::from("configs/update_feed.json")
}

fn default_update_interval() -> Duration {
    Duration::from_secs(3600)
}

fn default_simulation_seed() -> u64 {
    0xA11CEu64
}

/// Primary configuration object for the R-EMS runtime.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_mode")]
    pub mode: Mode,
    #[serde(default)]
    pub grids: IndexMap<String, GridConfig>,
    #[serde(default)]
    pub license: LicenseConfig,
    #[serde(default)]
    pub update: UpdateConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub simulation: SimulationConfig,
}

/// Metadata describing where an [`AppConfig`] was loaded from.
#[derive(Debug, Clone)]
pub struct LoadedAppConfig {
    pub config: AppConfig,
    pub source: PathBuf,
}

impl AppConfig {
    pub const ENV_CONFIG_PATH: &str = "R_EMS_CONFIG";

    /// Load configuration from disk, respecting the `R_EMS_CONFIG` override.
    pub fn load<P: AsRef<Path>>(candidates: &[P]) -> Result<Self> {
        Ok(Self::load_with_source(candidates)?.config)
    }

    /// Load configuration from disk together with the effective source path.
    pub fn load_with_source<P: AsRef<Path>>(candidates: &[P]) -> Result<LoadedAppConfig> {
        if let Ok(env_path) = std::env::var(Self::ENV_CONFIG_PATH) {
            if !env_path.trim().is_empty() {
                let path = PathBuf::from(env_path);
                let config = Self::from_path(path.clone())?;
                return Ok(LoadedAppConfig {
                    config,
                    source: path,
                });
            }
        }

        for candidate in candidates {
            if candidate.as_ref().exists() {
                let path = candidate.as_ref().to_path_buf();
                let config = Self::from_path(path.clone())?;
                return Ok(LoadedAppConfig {
                    config,
                    source: path,
                });
            }
        }

        Err(anyhow!(
            "no configuration files found. inspected: {}",
            candidates
                .iter()
                .map(|p| p.as_ref().display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    fn from_path(path: PathBuf) -> Result<Self> {
        debug!(config_path = %path.display(), "loading configuration");
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("unable to read config file {}", path.display()))?;
        let config = toml::from_str::<AppConfig>(&contents)
            .with_context(|| format!("failed to parse config file {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    /// Derive the effective orchestration mode, considering simulation overrides.
    pub fn effective_mode(&self) -> Mode {
        if let Some(force) = self.simulation.force_mode {
            return force;
        }
        self.mode
    }

    /// Retrieve a grid configuration by identifier.
    pub fn grid(&self, grid_id: &str) -> Option<&GridConfig> {
        self.grids.get(grid_id)
    }

    /// Validate structural invariants.
    pub fn validate(&self) -> Result<()> {
        if self.grids.is_empty() {
            return Err(anyhow!("configuration must contain at least one grid"));
        }
        for (grid_id, grid) in &self.grids {
            grid.validate(grid_id)?;
        }
        self.api.validate()?;
        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            grids: IndexMap::new(),
            license: LicenseConfig::default(),
            update: UpdateConfig::default(),
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            api: ApiConfig::default(),
            simulation: SimulationConfig::default(),
        }
    }
}

impl std::str::FromStr for AppConfig {
    type Err = anyhow::Error;

    fn from_str(content: &str) -> std::result::Result<Self, Self::Err> {
        let config: AppConfig =
            toml::from_str(content).with_context(|| "failed to parse configuration")?;
        config.validate()?;
        Ok(config)
    }
}

/// Operating mode for the orchestrator.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Production,
    Simulation,
    Hybrid,
}

impl Mode {
    pub fn is_simulation(&self) -> bool {
        matches!(self, Mode::Simulation)
    }

    pub fn is_hybrid(&self) -> bool {
        matches!(self, Mode::Hybrid)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GridConfig {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub controllers: IndexMap<String, ControllerConfig>,
    #[serde(default)]
    pub snapshot: SnapshotConfig,
}

impl GridConfig {
    pub fn validate(&self, grid_id: &str) -> Result<()> {
        if self.controllers.is_empty() {
            return Err(anyhow!(
                "grid '{}' must declare at least one controller",
                grid_id
            ));
        }
        if !self
            .controllers
            .values()
            .any(|c| matches!(c.role, ControllerRole::Primary))
        {
            return Err(anyhow!(
                "grid '{}' must define a primary controller",
                grid_id
            ));
        }
        Ok(())
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerConfig {
    #[serde(default)]
    pub role: ControllerRole,
    #[serde(default = "default_heartbeat_interval")]
    #[serde_as(as = "DurationSeconds<u64>")]
    pub heartbeat_interval: Duration,
    #[serde(default = "default_watchdog_timeout")]
    #[serde_as(as = "DurationSeconds<u64>")]
    pub watchdog_timeout: Duration,
    #[serde(default)]
    pub failover_order: u32,
    #[serde(default)]
    pub sensor_inputs: Vec<String>,
    #[serde(default)]
    pub metadata: IndexMap<String, String>,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            role: ControllerRole::Follower,
            heartbeat_interval: default_heartbeat_interval(),
            watchdog_timeout: default_watchdog_timeout(),
            failover_order: 0,
            sensor_inputs: Vec::new(),
            metadata: IndexMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ControllerRole {
    Primary,
    Secondary,
    #[default]
    Follower,
    Observer,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_snapshot_path")]
    pub directory: PathBuf,
    #[serde(default)]
    pub retain_last: usize,
    #[serde(default)]
    pub auto_replay: bool,
    #[serde(default)]
    #[serde_as(as = "Option<DurationSeconds<u64>>")]
    pub interval: Option<Duration>,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: default_snapshot_path(),
            retain_last: 5,
            auto_replay: true,
            interval: Some(Duration::from_secs(30)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseConfig {
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default = "default_env_license_var")]
    pub env_var: String,
    #[serde(default)]
    pub allow_bypass: bool,
}

impl Default for LicenseConfig {
    fn default() -> Self {
        Self {
            path: None,
            env_var: default_env_license_var(),
            allow_bypass: false,
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    #[serde(default = "default_update_feed")]
    pub feed_path: PathBuf,
    #[serde(default)]
    pub github_owner: Option<String>,
    #[serde(default)]
    pub github_repo: Option<String>,
    #[serde(default)]
    pub allow_apply_in_dev: bool,
    #[serde(default = "default_update_interval")]
    #[serde_as(as = "DurationSeconds<u64>")]
    pub poll_interval: Duration,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            feed_path: default_update_feed(),
            github_owner: Some("kentthoresen".to_owned()),
            github_repo: Some("R-EMS".to_owned()),
            allow_apply_in_dev: true,
            poll_interval: default_update_interval(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_logging_directory")]
    pub directory: PathBuf,
    #[serde(default = "default_log_format")]
    pub format: LogFormat,
    #[serde(default)]
    pub file_prefix: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            directory: default_logging_directory(),
            format: default_log_format(),
            file_prefix: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    #[serde(default = "default_metrics_enabled")]
    pub enabled: bool,
    #[serde(default = "default_metrics_listen")]
    pub listen: SocketAddr,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: default_metrics_enabled(),
            listen: default_metrics_listen(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_api_enabled")]
    pub enabled: bool,
    #[serde(default = "default_api_listen")]
    pub listen: SocketAddr,
    #[serde(default)]
    pub static_dir: Option<PathBuf>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: default_api_enabled(),
            listen: default_api_listen(),
            static_dir: Some(PathBuf::from("ui/setup-wizard/public")),
        }
    }
}

impl ApiConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if let Some(dir) = &self.static_dir {
                if !dir.is_dir() {
                    return Err(anyhow!(
                        "api static_dir {} does not exist or is not a directory",
                        dir.display()
                    ));
                }
            }
        }
        Ok(())
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    #[serde(default)]
    pub enable_randomized_inputs: bool,
    #[serde(default)]
    pub scenario_files: Vec<PathBuf>,
    #[serde(default = "default_simulation_seed")]
    pub random_seed: u64,
    #[serde(default)]
    pub force_mode: Option<Mode>,
    #[serde(default)]
    pub replay_speedup: Option<f64>,
    #[serde(default)]
    #[serde_as(as = "Option<DurationSeconds<u64>>")]
    pub tick_interval: Option<Duration>,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            enable_randomized_inputs: true,
            scenario_files: Vec::new(),
            random_seed: default_simulation_seed(),
            force_mode: None,
            replay_speedup: None,
            tick_interval: None,
        }
    }
}

impl std::str::FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "production" => Ok(Mode::Production),
            "simulation" => Ok(Mode::Simulation),
            "hybrid" => Ok(Mode::Hybrid),
            other => Err(format!("unknown mode: {}", other)),
        }
    }
}
