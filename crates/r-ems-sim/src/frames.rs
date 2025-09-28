//! ---
//! ems_section: "11-simulation-test-harness"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Simulation runtime helpers and scenario engines."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Synthetic or replayed telemetry frame produced for a controller tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFrame {
    pub grid_id: String,
    pub controller_id: String,
    pub timestamp: DateTime<Utc>,
    pub voltage_v: f64,
    pub frequency_hz: f64,
    pub load_kw: f64,
    pub synthetic: bool,
    #[serde(default)]
    pub scenario_label: Option<String>,
}

impl TelemetryFrame {
    pub fn synthetic(
        grid_id: &str,
        controller_id: &str,
        voltage_v: f64,
        frequency_hz: f64,
        load_kw: f64,
    ) -> Self {
        Self {
            grid_id: grid_id.to_owned(),
            controller_id: controller_id.to_owned(),
            timestamp: Utc::now(),
            voltage_v,
            frequency_hz,
            load_kw,
            synthetic: true,
            scenario_label: None,
        }
    }
}
