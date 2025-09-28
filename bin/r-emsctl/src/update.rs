//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "binary"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Control CLI for administrators interacting with R-EMS."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};
use r_ems_common::config::AppConfig;
use r_ems_versioning::semver::VersionInfo;
use r_ems_versioning::update::{UpdateClient, UpdateResult, UpdateSettings};
use tokio::runtime::Runtime;

/// Top-level update commands.
#[derive(Debug, Subcommand)]
pub enum UpdateCommand {
    /// Perform a read-only update check.
    Check(UpdateOptions),
    /// Perform a check and apply when an update is available.
    Apply(UpdateOptions),
}

impl UpdateCommand {
    fn options(&self) -> &UpdateOptions {
        match self {
            UpdateCommand::Check(opts) | UpdateCommand::Apply(opts) => opts,
        }
    }
}

/// Shared options for update operations.
#[derive(Debug, Args)]
pub struct UpdateOptions {
    /// Path to the configuration file to use for update settings.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
}

/// Execute the supplied update command.
pub fn run(command: UpdateCommand) -> Result<()> {
    let options = command.options();
    let config = load_config(options)?;
    let version = VersionInfo::current();
    let settings = UpdateSettings {
        feed_path: config.update.feed_path.clone(),
        github_owner: config.update.github_owner.clone(),
        github_repo: config.update.github_repo.clone(),
        allow_apply_in_dev: config.update.allow_apply_in_dev,
    };
    let client = UpdateClient::new(settings, version);
    let runtime = Runtime::new()?;
    let result = runtime.block_on(client.check())?;
    render_update_result(&result);

    if matches!(command, UpdateCommand::Apply(_)) {
        if result.update_available() {
            runtime.block_on(client.apply(&result))?;
            println!("Update apply completed");
        } else {
            println!("No update available to apply");
        }
    }

    Ok(())
}

fn load_config(options: &UpdateOptions) -> Result<AppConfig> {
    let mut candidates = Vec::new();
    if let Some(path) = &options.config {
        candidates.push(path.clone());
    }
    candidates.push(PathBuf::from("configs/example.prod.toml"));
    candidates.push(PathBuf::from("configs/example.dev.toml"));
    AppConfig::load(&candidates)
}

fn render_update_result(result: &UpdateResult) {
    if let Some(latest) = &result.latest {
        println!(
            "Current: {}\nLatest: {}\nUpdate Available: {}",
            result.current.cli_string(),
            latest.version,
            result.update_available()
        );
    } else {
        println!("Current: {}\nLatest: none", result.current.cli_string());
    }
}
