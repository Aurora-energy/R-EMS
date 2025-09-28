//! ---
//! ems_section: "11-simulation"
//! ems_subsection: "01-bootstrap"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Simulation generator utilities for scenario authoring."
//! ems_version: "v0.1.0"
//! ems_owner: "tbd"
//! ---
//! See `/docs/VERSIONING.md` for release coordination guidance and
//! `/docs/SIMULATION.md` for subsystem documentation.
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use r_ems_common::version::VersionInfo;
use r_ems_sim::{SimulationMode, TelemetrySimulationEngine};

#[derive(Debug, Clone, ValueEnum)]
enum SimulationKind {
    Randomized,
    Scenario,
    Hybrid,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Csv,
    Json,
}

#[derive(Debug, Parser)]
#[command(
    author,
    disable_version_flag = true,
    about = "Generate telemetry scenarios for R-EMS simulation mode",
    long_about = None
)]
struct Cli {
    /// Grid identifier to embed in generated frames
    #[arg(long, default_value = "grid-a")]
    grid: String,

    /// Controller identifier to embed in generated frames
    #[arg(long, default_value = "primary")]
    controller: String,

    /// Output file path. Use '-' for stdout.
    #[arg(long, default_value = "simulation.csv")]
    output: PathBuf,

    /// Explicit output format when extension is ambiguous
    #[arg(long, value_enum)]
    format: Option<OutputFormat>,

    /// Total duration in seconds to synthesise (ignored when --samples supplied)
    #[arg(long, default_value_t = 60)]
    duration_secs: u64,

    /// Interval between samples in milliseconds
    #[arg(long, default_value_t = 1000)]
    interval_ms: u64,

    /// Explicit number of samples to generate (overrides --duration/--interval)
    #[arg(long)]
    samples: Option<u64>,

    /// Simulation mode controlling the telemetry source
    #[arg(long, value_enum, default_value_t = SimulationKind::Randomized)]
    kind: SimulationKind,

    /// Path to a scenario file (CSV/JSON) when using scenario or hybrid modes
    #[arg(long = "scenario", value_name = "FILE")]
    scenario_file: Option<PathBuf>,

    /// Noise sigma used when hybridising scenario data
    #[arg(long, default_value_t = 0.2)]
    hybrid_noise: f64,

    /// Random seed for the generator
    #[arg(long)]
    seed: Option<u64>,

    /// Optional label recorded on each frame (useful when replaying mixed scenarios)
    #[arg(long = "label")]
    scenario_label: Option<String>,

    /// Print extended version information and exit
    #[arg(short = 'V', long = "version", action = ArgAction::SetTrue)]
    version: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.version {
        println!("{}", VersionInfo::current().extended());
        return Ok(());
    }
    if cli.interval_ms == 0 {
        return Err(anyhow!("interval-ms must be greater than zero"));
    }

    let format = determine_format(&cli.output, cli.format)?;
    let total_samples = compute_sample_count(&cli)?;
    let mut engine = build_engine(&cli)?;

    match format {
        OutputFormat::Csv => write_csv(&cli, total_samples, &mut engine)?,
        OutputFormat::Json => write_json(&cli, total_samples, &mut engine)?,
    }

    if cli.output.as_os_str() != "-" {
        eprintln!(
            "generated {} samples for {}/{} -> {}",
            total_samples,
            cli.grid,
            cli.controller,
            cli.output.display()
        );
    }

    Ok(())
}

fn compute_sample_count(cli: &Cli) -> Result<u64> {
    if let Some(samples) = cli.samples {
        return if samples == 0 {
            Err(anyhow!("samples must be greater than zero"))
        } else {
            Ok(samples)
        };
    }
    let ticks = (cli.duration_secs.saturating_mul(1000)) / cli.interval_ms;
    Ok(ticks.max(1))
}

fn determine_format(path: &Path, override_format: Option<OutputFormat>) -> Result<OutputFormat> {
    if let Some(format) = override_format {
        return Ok(format);
    }
    if path.as_os_str() == "-" {
        return Ok(OutputFormat::Json);
    }
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => Ok(OutputFormat::Json),
        Some("csv") => Ok(OutputFormat::Csv),
        _ => Ok(OutputFormat::Csv),
    }
}

fn build_engine(cli: &Cli) -> Result<TelemetrySimulationEngine> {
    let seed = cli.seed.unwrap_or(0x5EED_F00Du64);
    let mode = match cli.kind {
        SimulationKind::Randomized => SimulationMode::Randomized,
        SimulationKind::Scenario => SimulationMode::Scenario(
            cli.scenario_file
                .clone()
                .context("--scenario-file is required for scenario mode")?,
        ),
        SimulationKind::Hybrid => SimulationMode::Hybrid {
            scenario: cli
                .scenario_file
                .clone()
                .context("--scenario-file is required for hybrid mode")?,
            noise_sigma: cli.hybrid_noise.max(0.0),
        },
    };
    TelemetrySimulationEngine::new(mode, seed)
}

fn write_csv(cli: &Cli, samples: u64, engine: &mut TelemetrySimulationEngine) -> Result<()> {
    let writer: Box<dyn Write> =
        if cli.output.as_os_str() == "-" {
            Box::new(io::stdout())
        } else {
            Box::new(File::create(&cli.output).with_context(|| {
                format!("failed to create output file {}", cli.output.display())
            })?)
        };
    let mut writer = csv::Writer::from_writer(writer);
    for tick in 0..samples {
        let mut frame = engine.next_frame(&cli.grid, &cli.controller, tick);
        if let Some(label) = &cli.scenario_label {
            frame.scenario_label = Some(label.clone());
        }
        writer.serialize(&frame)?;
    }
    writer.flush()?;
    Ok(())
}

fn write_json(cli: &Cli, samples: u64, engine: &mut TelemetrySimulationEngine) -> Result<()> {
    let mut frames = Vec::with_capacity(samples as usize);
    for tick in 0..samples {
        let mut frame = engine.next_frame(&cli.grid, &cli.controller, tick);
        if let Some(label) = &cli.scenario_label {
            frame.scenario_label = Some(label.clone());
        }
        frames.push(frame);
    }
    if cli.output.as_os_str() == "-" {
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &frames)?;
        stdout.write_all(b"\n")?;
    } else {
        let file = File::create(&cli.output)
            .with_context(|| format!("failed to create output file {}", cli.output.display()))?;
        serde_json::to_writer_pretty(file, &frames)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::NamedTempFile;

    fn base_cli() -> Cli {
        Cli {
            grid: "grid-a".into(),
            controller: "primary".into(),
            output: PathBuf::from("out.csv"),
            format: None,
            duration_secs: 60,
            interval_ms: 1000,
            samples: None,
            kind: SimulationKind::Randomized,
            scenario_file: None,
            hybrid_noise: 0.2,
            seed: None,
            scenario_label: None,
            version: false,
        }
    }

    #[test]
    fn compute_sample_count_uses_duration() {
        let cli = base_cli();
        let samples = compute_sample_count(&cli).unwrap();
        assert_eq!(samples, 60);
    }

    #[test]
    fn compute_sample_count_prefers_explicit_samples() {
        let mut cli = base_cli();
        cli.samples = Some(5);
        assert_eq!(compute_sample_count(&cli).unwrap(), 5);
    }

    #[test]
    fn determine_format_defaults_csv() {
        let format = determine_format(Path::new("telemetry.data"), None).unwrap();
        assert!(matches!(format, OutputFormat::Csv));
    }

    #[test]
    fn determine_format_for_stdout_defaults_json() {
        let format = determine_format(Path::new("-"), None).unwrap();
        assert!(matches!(format, OutputFormat::Json));
    }

    #[test]
    fn build_engine_randomized_succeeds() {
        let cli = base_cli();
        build_engine(&cli).expect("randomized engine");
    }

    #[test]
    fn build_engine_requires_scenario_file() {
        let mut cli = base_cli();
        cli.kind = SimulationKind::Scenario;
        cli.scenario_file = None;
        assert!(build_engine(&cli).is_err());
    }

    #[test]
    fn build_engine_supports_scenario_mode() {
        let mut cli = base_cli();
        cli.kind = SimulationKind::Scenario;
        let mut file = NamedTempFile::new().expect("temp file");
        serde_json::to_writer(
            file.as_file_mut(),
            &vec![serde_json::json!({
                "grid_id": "grid-a",
                "controller_id": "primary",
                "timestamp": "2024-01-01T00:00:00Z",
                "voltage_v": 230.0,
                "frequency_hz": 50.0,
                "load_kw": 20.0
            })],
        )
        .unwrap();
        file.flush().unwrap();
        let path = file.into_temp_path();
        cli.scenario_file = Some(path.to_path_buf());
        let mut engine = build_engine(&cli).expect("scenario engine");
        let frame = engine.next_frame("grid-a", "primary", 0);
        assert_eq!(frame.grid_id, "grid-a");
        path.close().unwrap();
    }
}
