//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "binary"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Control CLI for administrators interacting with R-EMS."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand};
use r_ems_common::version::VersionInfo;
use r_ems_logging as logging;

mod setup;
mod update;

#[derive(Debug, Parser)]
#[command(
    author,
    disable_version_flag = true,
    about = "R-EMS administrative control utility",
    long_about = None
)]
struct Cli {
    #[arg(
        short = 'V',
        long = "version",
        action = ArgAction::SetTrue,
        help = "Print extended version information and exit"
    )]
    version: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(subcommand, about = "Installation setup and lifecycle actions")]
    Setup(setup::SetupCommand),
    #[command(subcommand, about = "Update management actions")]
    Update(update::UpdateCommand),
}

fn main() -> Result<()> {
    logging::init();
    let cli = Cli::parse();
    if cli.version {
        println!("{}", VersionInfo::current().extended());
        return Ok(());
    }
    match cli.command {
        Commands::Setup(cmd) => setup::run(cmd)?,
        Commands::Update(cmd) => update::run(cmd)?,
    }
    Ok(())
}
