//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "binary"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Control CLI for administrators interacting with R-EMS."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use indexmap::IndexMap;
use r_ems_common::config::{
    AppConfig, ControllerConfig, ControllerRole, GridConfig, LicenseConfig, Mode,
};
use r_ems_common::logging::LogFormat;
use r_ems_config::{persist_manifest, slugify_name, InstallationManifest, DEFAULT_CONFIG_ROOT};
use r_ems_logging::{log_system_event, SystemEventOutcome};

/// Dispatch entry point for setup-related subcommands.
pub fn run(command: SetupCommand) -> Result<()> {
    match command {
        SetupCommand::New(cmd) => cmd.execute(),
        SetupCommand::Wizard(cmd) => cmd.execute(),
    }
}

#[derive(Debug, Subcommand)]
pub enum SetupCommand {
    /// Create a new installation configuration and activate it.
    #[command(name = "new")]
    New(NewInstallationCommand),
    /// Launch an interactive first-run setup wizard.
    #[command(name = "wizard")]
    Wizard(WizardCommand),
}

#[derive(Debug, Args)]
pub struct NewInstallationCommand {
    /// Friendly name for the installation (used in manifests and log prefixes).
    #[arg(long = "installation-name", value_name = "NAME")]
    installation_name: String,

    /// Target root for persisted configuration (defaults to /etc/r-ems or R_EMS_CONFIG_ROOT).
    #[arg(
        long = "config-root",
        value_name = "DIR",
        env = "R_EMS_CONFIG_ROOT",
        default_value = DEFAULT_CONFIG_ROOT
    )]
    config_root: PathBuf,

    /// Operating mode for the generated configuration.
    #[arg(long, value_enum, default_value_t = ModeArg::Production)]
    mode: ModeArg,

    /// Directory for log files.
    #[arg(
        long = "log-directory",
        value_name = "DIR",
        default_value = "/var/log/r-ems"
    )]
    log_directory: PathBuf,

    /// Preferred log format.
    #[arg(long = "log-format", value_enum, default_value_t = LogFormatArg::StructuredJson)]
    log_format: LogFormatArg,

    /// Optional license file path baked into the configuration.
    #[arg(long = "license-path", value_name = "FILE")]
    license_path: Option<PathBuf>,

    /// Grid/controller topology expressed as GRID:role=id[,role=id].
    #[arg(long = "grid", value_name = "SPEC", required = true, num_args = 1..)]
    grids: Vec<String>,

    /// Optional template file to seed defaults before applying flags.
    #[arg(long = "template", value_name = "FILE")]
    template: Option<PathBuf>,

    /// Override the metrics listener (ip:port).
    #[arg(long = "metrics-listen", value_name = "ADDR")]
    metrics_listen: Option<SocketAddr>,

    /// Disable the embedded metrics exporter.
    #[arg(long = "metrics-disable", action = clap::ArgAction::SetTrue)]
    metrics_disable: bool,

    /// Override the REST API listener (ip:port).
    #[arg(long = "api-listen", value_name = "ADDR")]
    api_listen: Option<SocketAddr>,

    /// Disable the embedded REST API.
    #[arg(long = "api-disable", action = clap::ArgAction::SetTrue)]
    api_disable: bool,

    /// Directory served as static assets for the setup wizard.
    #[arg(long = "api-static-dir", value_name = "DIR")]
    api_static_dir: Option<PathBuf>,

    /// Add one or more simulation scenario files (CSV/JSON).
    #[arg(long = "simulation-scenario", value_name = "FILE", num_args = 0..)]
    simulation_scenario: Vec<PathBuf>,

    /// Override the simulation random seed.
    #[arg(long = "simulation-seed", value_name = "SEED")]
    simulation_seed: Option<u64>,

    /// Force runtime mode when entering simulation (overrides config).
    #[arg(long = "simulation-force-mode", value_enum)]
    simulation_force_mode: Option<ModeArg>,
}

#[derive(Debug, Args)]
pub struct WizardCommand {
    /// Target root for persisted configuration (defaults to /etc/r-ems or R_EMS_CONFIG_ROOT).
    #[arg(
        long = "config-root",
        value_name = "DIR",
        env = "R_EMS_CONFIG_ROOT",
        default_value = DEFAULT_CONFIG_ROOT
    )]
    config_root: PathBuf,

    /// Optional template file to seed defaults before prompting.
    #[arg(long = "template", value_name = "FILE")]
    template: Option<PathBuf>,
}

impl NewInstallationCommand {
    pub fn execute(self) -> Result<()> {
        let slug = slugify_name(&self.installation_name);
        if slug.is_empty() {
            return Err(anyhow!(
                "installation name must contain at least one alphanumeric character"
            ));
        }

        let mut app = load_base_config(self.template.as_ref())?;
        app.mode = self.mode.into();
        app.logging.directory = self.log_directory.clone();
        app.logging.format = self.log_format.into();
        app.logging.file_prefix = Some(slug.clone());
        if let Some(path) = &self.license_path {
            app.license = LicenseConfig {
                path: Some(path.clone()),
                ..app.license.clone()
            };
        }
        if let Some(addr) = self.metrics_listen {
            app.metrics.listen = addr;
        }
        if self.metrics_disable {
            app.metrics.enabled = false;
        }
        if let Some(addr) = self.api_listen {
            app.api.listen = addr;
        }
        if self.api_disable {
            app.api.enabled = false;
        }
        if let Some(dir) = &self.api_static_dir {
            app.api.static_dir = Some(dir.clone());
        }
        if let Some(seed) = self.simulation_seed {
            app.simulation.random_seed = seed;
        }
        if !self.simulation_scenario.is_empty() {
            app.simulation.scenario_files = self.simulation_scenario.clone();
        }
        if let Some(force_mode) = self.simulation_force_mode {
            app.simulation.force_mode = Some(force_mode.into());
        }

        app.grids = parse_grids(&self.grids)?;
        app.validate()?;

        let manifest = InstallationManifest::new(&self.installation_name, app)?;
        let persisted = persist_manifest(manifest, &self.config_root)?;

        println!(
            "Installation '{}' persisted to {}",
            persisted.manifest.installation.name,
            persisted.manifest_path.display()
        );
        println!("Current symlink: {}", persisted.current_path.display());
        println!(
            "Configuration hash: {}",
            persisted.manifest.installation.config_hash
        );

        Ok(())
    }
}

impl WizardCommand {
    pub fn execute(self) -> Result<()> {
        println!(
            "Starting interactive setup. Press Ctrl+C at any time to exit without writing files."
        );
        let install_root = self.prompt_installation_root()?;
        let (installation_name, app) = self.collect_config()?;
        let manifest = InstallationManifest::new(&installation_name, app)?;

        let preview = serde_yaml::to_string(&manifest.app)
            .context("failed to serialise configuration preview")?;
        println!("\nConfiguration preview (YAML):\n---\n{}---", preview);

        if !prompt_yes_no("Persist this configuration?", true)? {
            log_system_event(
                None,
                "setup.wizard.abort",
                "operator aborted before persisting configuration",
                SystemEventOutcome::Fault,
            );
            println!("Aborted. No files were written.");
            return Ok(());
        }

        let persisted = match persist_manifest(manifest, &install_root) {
            Ok(persisted) => persisted,
            Err(error) => {
                log_system_event(
                    None,
                    "setup.wizard.persist",
                    &format!("failed to persist installation: {}", error),
                    SystemEventOutcome::Fault,
                );
                return Err(error);
            }
        };

        log_system_event(
            None,
            "setup.wizard.persist",
            &format!(
                "installation '{}' saved to {}",
                persisted.manifest.installation.name,
                persisted.manifest_path.display()
            ),
            SystemEventOutcome::Success,
        );

        println!(
            "Installation '{}' persisted to {}",
            persisted.manifest.installation.name,
            persisted.manifest_path.display()
        );
        println!("Current symlink: {}", persisted.current_path.display());
        println!(
            "Configuration hash: {}",
            persisted.manifest.installation.config_hash
        );

        Ok(())
    }

    fn prompt_installation_root(&self) -> Result<PathBuf> {
        println!("\n=== Installation directory ===");
        let mut default_dir = self.config_root.display().to_string();
        loop {
            let chosen = prompt_text("Configuration root directory", Some(&default_dir), false)?;
            let candidate = PathBuf::from(chosen);
            default_dir = candidate.display().to_string();
            if candidate.exists() && !candidate.is_dir() {
                println!(
                    "The selected path '{}' exists but is not a directory. Please choose another location.",
                    candidate.display()
                );
                continue;
            }
            if !candidate.exists()
                && !prompt_yes_no(
                    &format!(
                        "Directory '{}' does not exist. Create it when persisting the configuration?",
                        candidate.display()
                    ),
                    true,
                )?
            {
                continue;
            }
            return Ok(candidate);
        }
    }

    fn collect_config(&self) -> Result<(String, AppConfig)> {
        let mut app = load_base_config(self.template.as_ref())?;

        println!("\n=== Installation identity ===");
        let installation_name = prompt_text("Installation name", None, false)?;
        let slug = slugify_name(&installation_name);
        if slug.is_empty() {
            return Err(anyhow!(
                "installation name must contain at least one alphanumeric character"
            ));
        }

        println!("\n=== Operating mode ===");
        let mode_items = [
            (
                "Production",
                Mode::Production,
                "Primary controllers drive real hardware",
            ),
            (
                "Simulation",
                Mode::Simulation,
                "All controllers run in sandboxed simulation",
            ),
            (
                "Hybrid",
                Mode::Hybrid,
                "Primary hardware with simulation failover",
            ),
        ];
        let mode_labels: Vec<String> = mode_items
            .iter()
            .map(|(label, _, desc)| format!("{label} — {desc}"))
            .collect();
        let default_mode_index = mode_items
            .iter()
            .position(|(_, mode, _)| *mode == app.mode)
            .unwrap_or(0);
        let mode_choice = prompt_choice("Select operating mode", &mode_labels, default_mode_index)?;
        app.mode = mode_items[mode_choice].1;

        println!("\n=== Logging ===");
        let log_dir_default = app.logging.directory.display().to_string();
        let log_directory = prompt_text("Log directory", Some(&log_dir_default), false)?;
        app.logging.directory = PathBuf::from(log_directory);

        let log_format_items = [
            (
                "Structured JSON (best for aggregators)",
                LogFormat::StructuredJson,
            ),
            ("Pretty (human readable)", LogFormat::Pretty),
        ];
        let log_format_labels: Vec<String> = log_format_items
            .iter()
            .map(|(label, _)| (*label).to_owned())
            .collect();
        let default_format_index = log_format_items
            .iter()
            .position(|(_, format)| *format == app.logging.format)
            .unwrap_or(0);
        let log_format_choice = prompt_choice(
            "Select log format",
            &log_format_labels,
            default_format_index,
        )?;
        app.logging.format = log_format_items[log_format_choice].1;
        app.logging.file_prefix = Some(slug.clone());

        println!("\n=== Grid topology ===");
        let grids = self.prompt_grids()?;
        app.grids = grids;

        app.validate()
            .with_context(|| "configuration validation failed")?;

        Ok((installation_name, app))
    }

    fn prompt_grids(&self) -> Result<IndexMap<String, GridConfig>> {
        let mut grids = IndexMap::new();
        loop {
            let grid_id = loop {
                let candidate = prompt_text("Grid identifier", None, false)?;
                if grids.contains_key(&candidate) {
                    println!(
                        "Grid '{}' already exists in this configuration. Choose a different identifier.",
                        candidate
                    );
                    continue;
                }
                break candidate;
            };

            let description = if prompt_yes_no("Add a description for this grid?", false)? {
                let desc = prompt_text("Grid description", None, true)?;
                let trimmed = desc.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                }
            } else {
                None
            };

            let controllers = self.prompt_controllers(&grid_id)?;
            let mut grid = GridConfig::default();
            grid.description = description;
            grid.controllers = controllers;

            println!(
                "Configured grid '{}' with {} controller(s).",
                grid_id,
                grid.controllers.len()
            );
            grids.insert(grid_id.clone(), grid);

            if !prompt_yes_no("Add another grid?", false)? {
                break;
            }
        }
        Ok(grids)
    }

    fn prompt_controllers(&self, grid_id: &str) -> Result<IndexMap<String, ControllerConfig>> {
        let mut controllers = IndexMap::new();
        loop {
            let controller_id = loop {
                let candidate = prompt_text("Controller identifier", None, false)?;
                if controllers.contains_key(&candidate) {
                    println!(
                        "Controller '{}' already exists for grid '{}'. Choose a different identifier.",
                        candidate, grid_id
                    );
                    continue;
                }
                break candidate;
            };

            let role_labels = vec![
                "Primary — leads the grid".to_owned(),
                "Secondary — hot standby".to_owned(),
                "Follower — load sharing".to_owned(),
                "Observer — telemetry only".to_owned(),
            ];
            let role_choice = prompt_choice(
                &format!("Role for controller '{controller_id}'"),
                &role_labels,
                0,
            )?;
            let mut controller = ControllerConfig::default();
            controller.role = match role_choice {
                0 => ControllerRole::Primary,
                1 => ControllerRole::Secondary,
                2 => ControllerRole::Follower,
                _ => ControllerRole::Observer,
            };
            let order = controllers.len() as u32;
            controller.failover_order = order;
            controllers.insert(controller_id.clone(), controller);
            println!(
                "Added {} controller '{}' (failover order {}).",
                role_labels[role_choice], controller_id, order
            );

            let has_primary = controllers
                .values()
                .any(|c| matches!(c.role, ControllerRole::Primary));
            if !has_primary {
                println!(
                    "Grid '{}' still requires a primary controller. Please add another controller.",
                    grid_id
                );
                continue;
            }

            if prompt_yes_no(
                &format!("Add another controller to grid '{grid_id}'?"),
                false,
            )? {
                continue;
            }
            break;
        }
        Ok(controllers)
    }
}

fn load_base_config(template: Option<&PathBuf>) -> Result<AppConfig> {
    if let Some(path) = template {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read template {}", path.display()))?;
        raw.parse::<AppConfig>()
    } else {
        Ok(AppConfig::default())
    }
}

fn prompt_text(prompt: &str, default: Option<&str>, allow_empty: bool) -> Result<String> {
    loop {
        if let Some(default) = default {
            print!("{prompt} [{default}]: ");
        } else {
            print!("{prompt}: ");
        }
        io::stdout()
            .flush()
            .context("failed to flush prompt to stdout")?;

        let mut input = String::new();
        let read = io::stdin()
            .read_line(&mut input)
            .context("failed to read response from stdin")?;
        if read == 0 {
            return Err(anyhow!("input stream closed"));
        }
        let trimmed = input.trim();
        if trimmed.is_empty() {
            if let Some(default) = default {
                return Ok(default.to_owned());
            }
            if allow_empty {
                return Ok(String::new());
            }
            println!("Input cannot be empty. Please try again.");
            continue;
        }
        return Ok(trimmed.to_owned());
    }
}

fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    loop {
        let suffix = if default { "[Y/n]" } else { "[y/N]" };
        print!("{prompt} {suffix}: ");
        io::stdout()
            .flush()
            .context("failed to flush prompt to stdout")?;

        let mut input = String::new();
        let read = io::stdin()
            .read_line(&mut input)
            .context("failed to read response from stdin")?;
        if read == 0 {
            return Err(anyhow!("input stream closed"));
        }
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default);
        }
        match trimmed.to_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please enter 'y' or 'n'."),
        }
    }
}

fn prompt_choice(prompt: &str, options: &[String], default_index: usize) -> Result<usize> {
    if options.is_empty() {
        return Err(anyhow!("prompt_choice requires at least one option"));
    }
    let default_index = default_index.min(options.len() - 1);
    loop {
        println!("{prompt}");
        for (idx, option) in options.iter().enumerate() {
            println!("  {}. {}", idx + 1, option);
        }
        let default_prompt = format!("{}", default_index + 1);
        let input = prompt_text("Select option", Some(&default_prompt), false)?;
        match input.parse::<usize>() {
            Ok(value) if (1..=options.len()).contains(&value) => return Ok(value - 1),
            _ => println!("Enter a number between 1 and {}.", options.len()),
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ModeArg {
    Production,
    Simulation,
    Hybrid,
}

impl From<ModeArg> for Mode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Production => Mode::Production,
            ModeArg::Simulation => Mode::Simulation,
            ModeArg::Hybrid => Mode::Hybrid,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LogFormatArg {
    StructuredJson,
    Pretty,
}

impl From<LogFormatArg> for LogFormat {
    fn from(value: LogFormatArg) -> Self {
        match value {
            LogFormatArg::StructuredJson => LogFormat::StructuredJson,
            LogFormatArg::Pretty => LogFormat::Pretty,
        }
    }
}

fn parse_grids(specs: &[String]) -> Result<IndexMap<String, GridConfig>> {
    let mut grids = IndexMap::new();
    for spec in specs {
        let (grid_id, controller_block) = spec
            .split_once(':')
            .ok_or_else(|| anyhow!("grid spec '{}' is missing ':' separator", spec))?;
        let grid_id = grid_id.trim();
        if grid_id.is_empty() {
            return Err(anyhow!("grid identifier cannot be empty (spec '{}')", spec));
        }
        let mut grid = GridConfig {
            controllers: IndexMap::new(),
            ..GridConfig::default()
        };
        let mut saw_primary = false;
        for (order, entry) in controller_block.split(',').enumerate() {
            let (role_str, controller_id) = entry
                .split_once('=')
                .ok_or_else(|| anyhow!("controller entry '{}' must include '='", entry))?;
            let controller_id = controller_id.trim();
            if controller_id.is_empty() {
                return Err(anyhow!("controller id cannot be empty in spec '{}'", entry));
            }
            let role = parse_controller_role(role_str.trim())?;
            if matches!(role, ControllerRole::Primary) {
                saw_primary = true;
            }
            if grid.controllers.contains_key(controller_id) {
                return Err(anyhow!(
                    "controller '{}' defined multiple times for grid '{}'",
                    controller_id,
                    grid_id
                ));
            }
            grid.controllers.insert(
                controller_id.to_owned(),
                ControllerConfig {
                    role,
                    failover_order: order as u32,
                    ..Default::default()
                },
            );
        }
        if !saw_primary {
            return Err(anyhow!(
                "grid '{}' must declare at least one primary controller",
                grid_id
            ));
        }
        grids.insert(grid_id.to_owned(), grid);
    }
    if grids.is_empty() {
        return Err(anyhow!("at least one grid must be specified"));
    }
    Ok(grids)
}

fn parse_controller_role(raw: &str) -> Result<ControllerRole> {
    match raw.to_lowercase().as_str() {
        "primary" => Ok(ControllerRole::Primary),
        "secondary" => Ok(ControllerRole::Secondary),
        "follower" => Ok(ControllerRole::Follower),
        "observer" => Ok(ControllerRole::Observer),
        other => Err(anyhow!("unknown controller role '{}'", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_grid_spec_happy_path() {
        let grids = parse_grids(&["grid-a:primary=a1,secondary=a2".to_string()]).unwrap();
        assert!(grids.contains_key("grid-a"));
        let controllers = &grids["grid-a"].controllers;
        assert_eq!(controllers.len(), 2);
        assert!(controllers["a1"].role == ControllerRole::Primary);
        assert!(controllers["a2"].role == ControllerRole::Secondary);
    }

    #[test]
    fn parse_grid_spec_requires_primary() {
        let err = parse_grids(&["grid-a:secondary=a2".to_string()]).unwrap_err();
        assert!(err.to_string().contains("primary"));
    }
}
