//! ---
//! ems_section: "08-energy-models-optimization"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Optimisation and calculation routines for energy planning."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFrame {
    pub timestamp: String,
    pub component_id: Uuid,
    pub voltage: f32,
    pub current: f32,
    pub power_kw: f32,
    pub temperature: f32,
    pub status: String,
}

impl TelemetryFrame {
    pub fn is_fault(&self) -> bool {
        self.status.eq_ignore_ascii_case("fault") || self.status.eq_ignore_ascii_case("faulted")
    }
}
