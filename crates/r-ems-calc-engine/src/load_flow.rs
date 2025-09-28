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

use nalgebra::{DMatrix, DVector};
use tracing::info;
use uuid::Uuid;

use crate::{
    errors::{CalcEngineError, Result},
    model::{ComponentKind, Impedance, SystemModel},
    telemetry::TelemetryFrame,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LoadFlowReport {
    pub bus_voltages: Vec<BusVoltage>,
    pub line_currents: Vec<LineCurrent>,
    pub total_losses_kw: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BusVoltage {
    pub component_id: Uuid,
    pub voltage: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LineCurrent {
    pub connection_id: Uuid,
    pub from: Uuid,
    pub to: Uuid,
    pub current: f32,
}

pub fn run_load_flow(model: &SystemModel, telemetry: &[TelemetryFrame]) -> Result<LoadFlowReport> {
    let n = model.components.len();
    if n == 0 {
        return Ok(LoadFlowReport {
            bus_voltages: Vec::new(),
            line_currents: Vec::new(),
            total_losses_kw: 0.0,
        });
    }

    let mut index_map = HashMap::new();
    for (idx, component) in model.components.iter().enumerate() {
        index_map.insert(component.id, idx);
    }

    let slack_index = model
        .components
        .iter()
        .position(|c| {
            matches!(c.kind, ComponentKind::Source | ComponentKind::Inverter)
                && !matches!(c.status, crate::model::ComponentStatus::Offline)
        })
        .ok_or(CalcEngineError::MissingSlack)?;

    let mut y = DMatrix::<f64>::zeros(n, n);
    for connection in &model.connections {
        if let (Some(&from_idx), Some(&to_idx)) = (
            index_map.get(&connection.from),
            index_map.get(&connection.to),
        ) {
            let Impedance { resistance_ohm, .. } = connection.cable.impedance();
            if resistance_ohm <= 0.0 {
                continue;
            }
            let conductance = 1.0 / resistance_ohm as f64;
            y[(from_idx, from_idx)] += conductance;
            y[(to_idx, to_idx)] += conductance;
            y[(from_idx, to_idx)] -= conductance;
            y[(to_idx, from_idx)] -= conductance;
        }
    }

    let mut current_injections = vec![0.0f64; n];
    let mut counts = vec![0u32; n];
    for frame in telemetry {
        if let Some(&idx) = index_map.get(&frame.component_id) {
            current_injections[idx] += frame.current as f64;
            counts[idx] += 1;
        }
    }

    for (idx, count) in counts.iter().enumerate() {
        if *count > 0 {
            current_injections[idx] /= *count as f64;
        } else {
            let component = &model.components[idx];
            let voltage = (component.nominal_voltage_kv * 1000.0) as f64;
            if voltage > 0.0 {
                let apparent_power = component.rated_power_kw as f64;
                let current = apparent_power * 1000.0 / (voltage * (3.0f64).sqrt());
                current_injections[idx] = current;
            }
        }
    }

    let slack_voltage = (model.components[slack_index].nominal_voltage_kv * 1000.0) as f64;

    let reduced_size = n - 1;
    let mut y_nn = DMatrix::<f64>::zeros(reduced_size, reduced_size);
    let mut rhs = DVector::<f64>::zeros(reduced_size);
    let mut reduced_to_full = Vec::with_capacity(reduced_size);

    let mut row = 0;
    for global_i in 0..n {
        if global_i == slack_index {
            continue;
        }
        reduced_to_full.push(global_i);
        let mut col = 0;
        for global_j in 0..n {
            if global_j == slack_index {
                continue;
            }
            y_nn[(row, col)] = y[(global_i, global_j)];
            col += 1;
        }
        let mut injection = current_injections[global_i];
        injection -= y[(global_i, slack_index)] * slack_voltage;
        rhs[row] = injection;
        row += 1;
    }

    let solution = y_nn
        .lu()
        .solve(&rhs)
        .ok_or(CalcEngineError::LoadFlowDidNotConverge)?;

    let mut voltages = vec![slack_voltage; n];
    for (idx, &global_index) in reduced_to_full.iter().enumerate() {
        voltages[global_index] = solution[idx];
    }

    let bus_voltages = model
        .components
        .iter()
        .zip(&voltages)
        .map(|(component, &voltage)| BusVoltage {
            component_id: component.id,
            voltage: voltage as f32,
        })
        .collect::<Vec<_>>();

    let mut line_currents = Vec::new();
    let mut total_losses_kw = 0.0f32;

    for connection in &model.connections {
        if let (Some(&from_idx), Some(&to_idx)) = (
            index_map.get(&connection.from),
            index_map.get(&connection.to),
        ) {
            let Impedance { resistance_ohm, .. } = connection.cable.impedance();
            if resistance_ohm <= 0.0 {
                continue;
            }
            let conductance = 1.0 / resistance_ohm as f64;
            let current = (voltages[from_idx] - voltages[to_idx]) * conductance;
            let loss_kw = (current.powi(2) * resistance_ohm as f64) / 1000.0;
            total_losses_kw += loss_kw as f32;
            line_currents.push(LineCurrent {
                connection_id: connection.id,
                from: connection.from,
                to: connection.to,
                current: current as f32,
            });
        }
    }

    info!("Load flow completed with {:.2} kW losses", total_losses_kw);

    Ok(LoadFlowReport {
        bus_voltages,
        line_currents,
        total_losses_kw,
    })
}
