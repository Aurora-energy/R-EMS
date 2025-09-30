use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub system: SystemTopology,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemTopology {
    pub grids: Vec<GridConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridConfig {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub controllers: Vec<ControllerConfig>,
    #[serde(default)]
    pub devices: Vec<DeviceConfig>,
    #[serde(default)]
    pub allow_interop: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerRole {
    Primary,
    Backup,
    Standalone,
}

impl Default for ControllerRole {
    fn default() -> Self {
        ControllerRole::Standalone
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerConfig {
    pub id: String,
    #[serde(default)]
    pub role: ControllerRole,
    #[serde(default)]
    pub redundancy_group: Option<String>,
    #[serde(default)]
    pub heartbeat_interval_ms: Option<u64>,
    #[serde(default)]
    pub failover_timeout_ms: Option<u64>,
    #[serde(default)]
    pub sync_channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BusKind {
    Rs485,
    Can,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceProtocolConfig {
    #[serde(default)]
    pub dbc_file: Option<PathBuf>,
    #[serde(default)]
    pub register_map: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub id: String,
    pub bus: BusKind,
    pub address: String,
    #[serde(default)]
    pub protocol: DeviceProtocolConfig,
    #[serde(default)]
    pub telemetry: Vec<TelemetryPoint>,
    #[serde(default)]
    pub commands: Vec<DeviceCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPoint {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCommand {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub grids: usize,
    pub controllers: usize,
    pub devices: usize,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read configuration from {path:?}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse configuration at {path:?}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("configuration validation failed:\n{details}")]
    Validation { details: String },
}

pub fn load_config(path: impl AsRef<Path>) -> Result<SystemConfig, ConfigError> {
    let path = path.as_ref();
    let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    serde_yaml::from_str(&contents).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

pub fn validate_config(config: &SystemConfig) -> Result<ValidationReport, ConfigError> {
    let mut errors = Vec::new();

    let grids = &config.system.grids;
    if grids.is_empty() {
        errors.push("system must define at least one grid".to_string());
    }

    let mut grid_ids = HashSet::new();
    let mut controller_total = 0usize;
    let mut device_total = 0usize;

    for grid in grids {
        if grid.id.trim().is_empty() {
            errors.push("grid id may not be empty".to_string());
        }
        if !grid_ids.insert(grid.id.clone()) {
            errors.push(format!("duplicate grid id '{}'", grid.id));
        }

        if grid.controllers.is_empty() {
            errors.push(format!(
                "grid '{}' must define at least one controller",
                grid.id
            ));
        }

        let mut controller_ids = HashSet::new();
        let mut redundancy_groups: HashMap<String, (usize, usize)> = HashMap::new();

        for controller in &grid.controllers {
            if controller.id.trim().is_empty() {
                errors.push(format!("grid '{}' has controller with empty id", grid.id));
            }

            if !controller_ids.insert(controller.id.clone()) {
                errors.push(format!(
                    "grid '{}' has duplicate controller id '{}'",
                    grid.id, controller.id
                ));
            }

            match controller.role {
                ControllerRole::Primary | ControllerRole::Backup => {
                    let group = controller.redundancy_group.clone().unwrap_or_else(|| {
                        errors.push(format!(
                            "controller '{}' in grid '{}' must specify a redundancy_group",
                            controller.id, grid.id
                        ));
                        String::new()
                    });

                    if controller.heartbeat_interval_ms.is_none() {
                        errors.push(format!(
                            "controller '{}' in grid '{}' must define heartbeat_interval_ms",
                            controller.id, grid.id
                        ));
                    }

                    if controller.failover_timeout_ms.is_none() {
                        errors.push(format!(
                            "controller '{}' in grid '{}' must define failover_timeout_ms",
                            controller.id, grid.id
                        ));
                    }

                    if !group.is_empty() {
                        let entry = redundancy_groups.entry(group).or_insert((0, 0));
                        match controller.role {
                            ControllerRole::Primary => entry.0 += 1,
                            ControllerRole::Backup => entry.1 += 1,
                            ControllerRole::Standalone => {}
                        }
                    }
                }
                ControllerRole::Standalone => {
                    if controller.redundancy_group.is_some() {
                        errors.push(format!(
                            "standalone controller '{}' in grid '{}' must not set redundancy_group",
                            controller.id, grid.id
                        ));
                    }
                }
            }
        }

        for (group, (primaries, backups)) in redundancy_groups {
            if primaries == 0 {
                errors.push(format!(
                    "grid '{}' redundancy group '{}' must define exactly one primary controller",
                    grid.id, group
                ));
            } else if primaries > 1 {
                errors.push(format!(
                    "grid '{}' redundancy group '{}' defines multiple primary controllers",
                    grid.id, group
                ));
            }

            if backups == 0 {
                errors.push(format!(
                    "grid '{}' redundancy group '{}' must define at least one backup controller",
                    grid.id, group
                ));
            }
        }

        controller_total += grid.controllers.len();

        if grid.devices.is_empty() {
            errors.push(format!(
                "grid '{}' must define at least one device",
                grid.id
            ));
        }

        let mut device_ids = HashSet::new();
        for device in &grid.devices {
            if device.id.trim().is_empty() {
                errors.push(format!("grid '{}' has device with empty id", grid.id));
            }

            if !device_ids.insert(device.id.clone()) {
                errors.push(format!(
                    "grid '{}' has duplicate device id '{}'",
                    grid.id, device.id
                ));
            }

            if device.address.trim().is_empty() {
                errors.push(format!(
                    "device '{}' in grid '{}' must define a bus address",
                    device.id, grid.id
                ));
            }

            match device.bus {
                BusKind::Can => {
                    if device.protocol.dbc_file.is_none() {
                        errors.push(format!(
                            "CAN device '{}' in grid '{}' must specify protocol.dbc_file",
                            device.id, grid.id
                        ));
                    }
                }
                BusKind::Rs485 => {
                    if device.protocol.register_map.is_none() {
                        errors.push(format!(
                            "RS-485 device '{}' in grid '{}' must specify protocol.register_map",
                            device.id, grid.id
                        ));
                    }
                }
            }

            if device.telemetry.is_empty() {
                errors.push(format!(
                    "device '{}' in grid '{}' must declare at least one telemetry point",
                    device.id, grid.id
                ));
            }

            let mut telemetry_names = HashSet::new();
            for telemetry in &device.telemetry {
                if telemetry.name.trim().is_empty() {
                    errors.push(format!(
                        "device '{}' in grid '{}' has telemetry entry with empty name",
                        device.id, grid.id
                    ));
                }
                if !telemetry_names.insert(telemetry.name.clone()) {
                    errors.push(format!(
                        "device '{}' in grid '{}' has duplicate telemetry name '{}'",
                        device.id, grid.id, telemetry.name
                    ));
                }
            }

            let mut command_names = HashSet::new();
            for command in &device.commands {
                if command.name.trim().is_empty() {
                    errors.push(format!(
                        "device '{}' in grid '{}' has command with empty name",
                        device.id, grid.id
                    ));
                }
                if !command_names.insert(command.name.clone()) {
                    errors.push(format!(
                        "device '{}' in grid '{}' has duplicate command name '{}'",
                        device.id, grid.id, command.name
                    ));
                }
            }
        }

        device_total += grid.devices.len();
    }

    if errors.is_empty() {
        Ok(ValidationReport {
            grids: grids.len(),
            controllers: controller_total,
            devices: device_total,
        })
    } else {
        Err(ConfigError::Validation {
            details: errors.join("\n"),
        })
    }
}
