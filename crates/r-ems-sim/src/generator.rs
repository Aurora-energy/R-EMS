//! ---
//! ems_section: "11-simulation-test-harness"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Simulation runtime helpers and scenario engines."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::f64::consts::PI;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rand::prelude::*;
use rand_distr::Normal;

use crate::frames::TelemetryFrame;
use crate::replay::ReplayEngine;

/// Simulation mode describing telemetry behaviour.
#[derive(Debug, Clone)]
pub enum SimulationMode {
    Randomized,
    Scenario(PathBuf),
    Hybrid { scenario: PathBuf, noise_sigma: f64 },
}

/// Generates telemetry streams for controllers, supporting random and replay modes.
#[derive(Debug)]
pub struct SimulationEngine {
    mode: SimulationMode,
    rng: StdRng,
    noise: Normal<f64>,
    start: Instant,
    replay: Option<ReplayEngine>,
}

impl SimulationEngine {
    pub fn new(mode: SimulationMode, seed: u64) -> Result<Self> {
        let sigma = match &mode {
            SimulationMode::Hybrid { noise_sigma, .. } => *noise_sigma,
            _ => 0.2,
        };
        let replay = match &mode {
            SimulationMode::Scenario(path) | SimulationMode::Hybrid { scenario: path, .. } => Some(
                ReplayEngine::from_path(path)
                    .with_context(|| format!("unable to load scenario {}", path.display()))?,
            ),
            SimulationMode::Randomized => None,
        };
        Ok(Self {
            mode,
            rng: StdRng::seed_from_u64(seed),
            noise: Normal::new(0.0, sigma).expect("sigma must be positive"),
            start: Instant::now(),
            replay,
        })
    }

    pub fn next_frame(&mut self, grid_id: &str, controller_id: &str, tick: u64) -> TelemetryFrame {
        if let Some(replay) = &mut self.replay {
            if let Some(frame) = replay.next_frame() {
                if matches!(self.mode, SimulationMode::Hybrid { .. }) {
                    return self.hybridise(frame);
                }
                return frame;
            }
        }
        self.synthetic_frame(grid_id, controller_id, tick)
    }

    fn synthetic_frame(&mut self, grid_id: &str, controller_id: &str, tick: u64) -> TelemetryFrame {
        let elapsed = self.start.elapsed() + Duration::from_millis(tick % 1000);
        let t = elapsed.as_secs_f64();
        let voltage = 230.0 + 5.0 * (2.0 * PI * 0.2 * t).sin() + self.noise_sample();
        let frequency = 50.0 + 0.05 * (2.0 * PI * 0.05 * t).cos() + self.noise_sample() * 0.01;
        let mut load = 25.0 + 15.0 * (2.0 * PI * 0.01 * t).sin();
        if matches!(self.mode, SimulationMode::Hybrid { .. }) {
            load *= 1.05;
        }
        TelemetryFrame::synthetic(grid_id, controller_id, voltage, frequency, load)
    }

    fn hybridise(&mut self, mut frame: TelemetryFrame) -> TelemetryFrame {
        let noise = self.noise_sample();
        frame.voltage_v += noise;
        frame.frequency_hz += noise * 0.01;
        frame.load_kw += noise * 0.5;
        frame.synthetic = true;
        frame
    }

    fn noise_sample(&mut self) -> f64 {
        self.noise.sample(&mut self.rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde_json::json;
    use tempfile::NamedTempFile;

    #[test]
    fn randomized_mode_produces_synthetic_frames() {
        let mut engine = SimulationEngine::new(SimulationMode::Randomized, 42).unwrap();
        let frame = engine.next_frame("grid-a", "controller-1", 0);
        assert!(frame.synthetic);
        assert!(frame.voltage_v > 200.0 && frame.voltage_v < 260.0);
        assert!(frame.frequency_hz > 49.0 && frame.frequency_hz < 51.0);
        assert!(frame.load_kw > 0.0);
    }

    #[test]
    fn scenario_mode_replays_frames() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        let scenario = vec![json!({
            "grid_id": "grid-a",
            "controller_id": "primary",
            "timestamp": "2024-01-01T00:00:00Z",
            "voltage_v": 228.0,
            "frequency_hz": 49.95,
            "load_kw": 40.0
        })];
        serde_json::to_writer(file.as_file_mut(), &scenario)?;
        file.flush()?;

        let path = file.into_temp_path();
        let mut engine = SimulationEngine::new(SimulationMode::Scenario(path.to_path_buf()), 1337)?;
        let frame = engine.next_frame("grid-a", "primary", 0);
        assert!(!frame.synthetic);
        assert_eq!(frame.grid_id, "grid-a");
        assert!((frame.voltage_v - 228.0).abs() < 1.0);
        path.close()?;
        Ok(())
    }

    #[test]
    fn hybrid_mode_marks_frames_synthetic() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        let scenario = vec![json!({
            "grid_id": "grid-a",
            "controller_id": "primary",
            "timestamp": "2024-01-01T00:00:00Z",
            "voltage_v": 228.0,
            "frequency_hz": 49.95,
            "load_kw": 40.0
        })];
        serde_json::to_writer(file.as_file_mut(), &scenario)?;
        file.flush()?;

        let path = file.into_temp_path();
        let mut engine = SimulationEngine::new(
            SimulationMode::Hybrid {
                scenario: path.to_path_buf(),
                noise_sigma: 0.5,
            },
            99,
        )?;
        let frame = engine.next_frame("grid-a", "primary", 0);
        assert!(frame.synthetic);
        assert!(frame.voltage_v > 200.0);
        path.close()?;
        Ok(())
    }
}
