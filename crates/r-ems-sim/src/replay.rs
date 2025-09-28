//! ---
//! ems_section: "11-simulation-test-harness"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Simulation runtime helpers and scenario engines."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use csv::ReaderBuilder;
use serde::Deserialize;

use crate::frames::TelemetryFrame;

/// Raw frame representation when deserializing scenarios.
#[derive(Debug, Deserialize)]
pub struct ScenarioFrame {
    pub grid_id: String,
    pub controller_id: String,
    pub timestamp: String,
    pub voltage_v: f64,
    pub frequency_hz: f64,
    pub load_kw: f64,
    #[serde(default)]
    pub scenario_label: Option<String>,
}

/// In-memory scenario replay helper that iterates deterministic telemetry.
#[derive(Debug, Default, Clone)]
pub struct ReplayEngine {
    frames: Vec<TelemetryFrame>,
    cursor: usize,
}

impl ReplayEngine {
    pub fn from_path(path: &Path) -> Result<Self> {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => Self::from_json(path),
            Some("csv") => Self::from_csv(path),
            _ => anyhow::bail!("unsupported scenario format: {}", path.display()),
        }
    }

    pub fn next_frame(&mut self) -> Option<TelemetryFrame> {
        if self.frames.is_empty() {
            return None;
        }
        let frame = self.frames[self.cursor].clone();
        self.cursor = (self.cursor + 1) % self.frames.len();
        Some(frame)
    }

    fn from_json(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("unable to read scenario file {}", path.display()))?;
        let raw_frames: Vec<ScenarioFrame> = serde_json::from_str(&contents)
            .with_context(|| format!("invalid scenario JSON {}", path.display()))?;
        Ok(Self {
            frames: raw_frames
                .into_iter()
                .map(|raw| TelemetryFrame {
                    grid_id: raw.grid_id,
                    controller_id: raw.controller_id,
                    timestamp: raw.timestamp.parse().unwrap_or_else(|_| chrono::Utc::now()),
                    voltage_v: raw.voltage_v,
                    frequency_hz: raw.frequency_hz,
                    load_kw: raw.load_kw,
                    synthetic: false,
                    scenario_label: raw.scenario_label,
                })
                .collect(),
            cursor: 0,
        })
    }

    fn from_csv(path: &Path) -> Result<Self> {
        let file = fs::File::open(path)
            .with_context(|| format!("unable to open scenario csv {}", path.display()))?;
        let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);
        let mut frames = Vec::new();
        for row in reader.deserialize::<ScenarioFrame>() {
            let raw = row.with_context(|| format!("invalid scenario row in {}", path.display()))?;
            frames.push(TelemetryFrame {
                grid_id: raw.grid_id,
                controller_id: raw.controller_id,
                timestamp: raw.timestamp.parse().unwrap_or_else(|_| chrono::Utc::now()),
                voltage_v: raw.voltage_v,
                frequency_hz: raw.frequency_hz,
                load_kw: raw.load_kw,
                synthetic: false,
                scenario_label: raw.scenario_label.clone(),
            });
        }
        Ok(Self { frames, cursor: 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn loads_json_scenarios() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(
            file,
            "{}",
            r#"[{"grid_id":"grid","controller_id":"a","timestamp":"2024-01-01T00:00:00Z","voltage_v":230.0,"frequency_hz":50.0,"load_kw":20.0}]"#
        )?;
        file.flush()?;
        let path = file.into_temp_path();
        let mut replay = ReplayEngine::from_path(path.as_ref())?;
        let frame = replay.next_frame().expect("frame expected");
        assert_eq!(frame.grid_id, "grid");
        assert!(!frame.synthetic);
        path.close()?;
        Ok(())
    }

    #[test]
    fn loads_csv_scenarios() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(
            file,
            "grid_id,controller_id,timestamp,voltage_v,frequency_hz,load_kw"
        )?;
        writeln!(file, "grid,a,2024-01-01T00:00:00Z,231.0,50.1,25.0")?;
        file.flush()?;
        let path = file.into_temp_path();
        let mut replay = ReplayEngine::from_path(path.as_ref())?;
        let frame = replay.next_frame().expect("frame expected");
        assert_eq!(frame.voltage_v, 231.0);
        assert!(!frame.synthetic);
        path.close()?;
        Ok(())
    }

    #[test]
    fn next_frame_cycles_through_frames() {
        let mut replay = ReplayEngine {
            frames: vec![
                TelemetryFrame::synthetic("grid", "a", 1.0, 1.0, 1.0),
                TelemetryFrame::synthetic("grid", "a", 2.0, 2.0, 2.0),
            ],
            cursor: 0,
        };
        let first = replay.next_frame().unwrap();
        let second = replay.next_frame().unwrap();
        let third = replay.next_frame().unwrap();
        assert_ne!(first.voltage_v, second.voltage_v);
        assert_eq!(first.voltage_v, third.voltage_v);
    }
}
