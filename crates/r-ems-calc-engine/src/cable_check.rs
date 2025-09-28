//! ---
//! ems_section: "08-energy-models-optimization"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Optimisation and calculation routines for energy planning."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::HashMap;

use tracing::warn;
use uuid::Uuid;

use crate::{
    errors::Result,
    load_flow::LoadFlowReport,
    model::{Connection, SystemModel},
    telemetry::TelemetryFrame,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CableIssue {
    pub connection_id: Uuid,
    pub cable_name: String,
    pub reasons: Vec<String>,
    pub measured_current_a: f32,
    pub voltage_drop_percent: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CableCheckReport {
    pub undersized: Vec<CableIssue>,
}

impl CableCheckReport {
    pub fn is_ok(&self) -> bool {
        self.undersized.is_empty()
    }
}

pub fn validate_cables(
    model: &SystemModel,
    telemetry: &[TelemetryFrame],
    load_flow: &LoadFlowReport,
) -> Result<CableCheckReport> {
    let mut undersized = Vec::new();

    let mut telemetry_currents: HashMap<Uuid, Vec<f32>> = HashMap::new();
    for frame in telemetry {
        telemetry_currents
            .entry(frame.component_id)
            .or_default()
            .push(frame.current);
    }

    let bus_voltage_map: HashMap<Uuid, f32> = load_flow
        .bus_voltages
        .iter()
        .map(|bus| (bus.component_id, bus.voltage))
        .collect();

    let line_current_map: HashMap<Uuid, f32> = load_flow
        .line_currents
        .iter()
        .map(|line| (line.connection_id, line.current.abs()))
        .collect();

    for connection in &model.connections {
        let current = line_current_map
            .get(&connection.id)
            .copied()
            .unwrap_or_else(|| {
                estimate_connection_current(connection, telemetry_currents.get(&connection.to))
            });
        let nominal_voltage = model
            .find_component(connection.from)
            .map(|c| c.nominal_voltage_kv * 1000.0)
            .unwrap_or(400.0);
        let drop_percent = bus_voltage_map
            .get(&connection.from)
            .zip(bus_voltage_map.get(&connection.to))
            .map(|(from_v, to_v)| ((from_v - to_v).abs() / from_v.max(1.0)) * 100.0)
            .unwrap_or_else(|| connection.cable.voltage_drop(current, nominal_voltage));

        let mut reasons = Vec::new();
        if current > connection.cable.ampacity_a {
            reasons.push(format!(
                "Current {:.1} A exceeds ampacity {:.1} A",
                current, connection.cable.ampacity_a
            ));
        }
        if drop_percent > connection.cable.voltage_drop_limit_percent {
            reasons.push(format!(
                "Voltage drop {:.2}% exceeds limit {:.2}%",
                drop_percent, connection.cable.voltage_drop_limit_percent
            ));
        }

        if let Some(breaker) = &connection.cable.breaker {
            let breaker_limit_a = breaker.rating_ka * 1000.0;
            let withstand_limit_a = connection.cable.withstand_ka * 1000.0;
            if withstand_limit_a < breaker_limit_a {
                reasons.push(format!(
                    "Cable withstand {:.1} A²s insufficient for breaker {:.1} A",
                    withstand_limit_a, breaker_limit_a
                ));
            }
        }

        if !reasons.is_empty() {
            warn!(
                "[WARN] Cable {} undersized: {}",
                connection.cable.name,
                reasons.join(", ")
            );
            undersized.push(CableIssue {
                connection_id: connection.id,
                cable_name: connection.cable.name.clone(),
                reasons,
                measured_current_a: current,
                voltage_drop_percent: drop_percent,
            });
        }
    }

    Ok(CableCheckReport { undersized })
}

fn estimate_connection_current(connection: &Connection, telemetry: Option<&Vec<f32>>) -> f32 {
    if let Some(samples) = telemetry {
        if !samples.is_empty() {
            return samples.iter().copied().sum::<f32>() / samples.len() as f32;
        }
    }
    let impedance = connection.cable.impedance();
    let resistance = impedance.resistance_ohm.max(1e-3);
    let nominal_voltage = 400.0;
    (nominal_voltage / resistance) / (3.0f32).sqrt()
}
