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

use petgraph::{algo::astar, graph::NodeIndex, Graph};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    errors::{CalcEngineError, Result},
    model::{ComponentKind, GridComponent, Impedance, ProtectionDevice, SystemModel},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShortCircuitReport {
    pub fault_location: Uuid,
    pub ik: f32,
    pub cable_trip: Option<Uuid>,
    pub breaker_trip: Option<Uuid>,
}

pub fn calculate_short_circuit(model: &SystemModel) -> Result<ShortCircuitReport> {
    let faulted = model
        .faulted_component()
        .or_else(|| {
            model
                .components
                .iter()
                .find(|c| matches!(c.status, crate::model::ComponentStatus::Faulted))
        })
        .ok_or(CalcEngineError::MissingFault)?;

    let mut graph = Graph::<Uuid, Uuid>::new();
    let mut indices: HashMap<Uuid, NodeIndex> = HashMap::new();

    for component in &model.components {
        let index = graph.add_node(component.id);
        indices.insert(component.id, index);
    }

    for connection in &model.connections {
        if let (Some(from), Some(to)) = (indices.get(&connection.from), indices.get(&connection.to))
        {
            graph.add_edge(*from, *to, connection.id);
            graph.add_edge(*to, *from, connection.id);
        }
    }

    let fault_idx = *indices
        .get(&faulted.id)
        .ok_or_else(|| CalcEngineError::UnknownComponent(faulted.id))?;

    let sources: Vec<&GridComponent> = model
        .components
        .iter()
        .filter(|c| {
            matches!(
                c.kind,
                ComponentKind::Source | ComponentKind::Inverter | ComponentKind::Storage
            )
        })
        .collect();

    let mut best_path: Option<(f32, Vec<Uuid>)> = None;

    for source in sources {
        if let Some(source_idx) = indices.get(&source.id) {
            if let Some((_cost, path)) = astar(
                &graph,
                *source_idx,
                |finish| finish == fault_idx,
                |_edge| 1.0,
                |_| 0.0,
            ) {
                let mut sequence = Vec::new();
                for window in path.windows(2) {
                    let from = window[0];
                    let to = window[1];
                    if let Some(edge) = graph.find_edge(from, to) {
                        let conn_id = graph.edge_weight(edge).copied().unwrap();
                        sequence.push(conn_id);
                    }
                }

                let total_impedance = total_path_impedance(model, source, faulted, &sequence);
                if total_impedance.magnitude() > 0.0 {
                    let ik = fault_current_kiloamps(
                        faulted.nominal_voltage_kv * 1000.0,
                        &total_impedance,
                    );
                    match &mut best_path {
                        Some((current_best, _)) if ik <= *current_best => {}
                        _ => {
                            best_path = Some((ik, sequence));
                        }
                    }
                }
            }
        }
    }

    let (ik, sequence) = best_path.unwrap_or_else(|| {
        warn!("No valid source path located, defaulting to local impedance only");
        let impedance = faulted.impedance.unwrap_or(Impedance {
            resistance_ohm: 0.1,
            reactance_ohm: 0.05,
        });
        let ik = fault_current_kiloamps(faulted.nominal_voltage_kv * 1000.0, &impedance);
        (ik, Vec::new())
    });

    let mut cable_trip = None;
    let mut breaker_trip = None;

    for connection_id in &sequence {
        if let Some(connection) = model.connections.iter().find(|c| c.id == *connection_id) {
            let ampacity_limit = connection.cable.ampacity_a;
            let ik_a = ik * 1000.0;
            if cable_trip.is_none() && ik_a > ampacity_limit {
                info!(
                    "[WARN] Cable {} undersized under fault: {:.2} A > {:.2} A",
                    connection.cable.name, ik_a, ampacity_limit
                );
                cable_trip = Some(connection.id);
            }

            if breaker_trip.is_none() {
                if let Some(ProtectionDevice {
                    breaker_id,
                    rating_ka,
                }) = &connection.cable.breaker
                {
                    if ik > *rating_ka {
                        info!("[INFO] Breaker {} would trip at {:.2} kA", breaker_id, ik);
                        breaker_trip = Some(*breaker_id);
                    }
                }
            }
        }
    }

    info!("Fault current at {} = {:.2} kA", faulted.name, ik);

    Ok(ShortCircuitReport {
        fault_location: faulted.id,
        ik,
        cable_trip,
        breaker_trip,
    })
}

fn total_path_impedance(
    model: &SystemModel,
    source: &GridComponent,
    faulted: &GridComponent,
    sequence: &[Uuid],
) -> Impedance {
    let mut total = source_impedance(source, faulted.nominal_voltage_kv * 1000.0);
    for conn_id in sequence {
        if let Some(connection) = model.connections.iter().find(|c| c.id == *conn_id) {
            total = total.add(&connection.cable.impedance());
        }
    }
    if let Some(component_impedance) = faulted.impedance {
        total = total.add(&component_impedance);
    }
    total
}

fn source_impedance(source: &GridComponent, fault_voltage_v: f32) -> Impedance {
    let ratio = source.short_circuit_ratio.unwrap_or(5.0);
    let sc_mva = (source.rated_power_kw / 1000.0) * ratio;
    let base_voltage = fault_voltage_v;
    let impedance = if sc_mva > 0.0 {
        let z = (base_voltage.powi(2) / (sc_mva * 1_000_000.0)).max(1e-4);
        z
    } else {
        0.05
    };
    Impedance {
        resistance_ohm: impedance,
        reactance_ohm: impedance * 0.4,
    }
}

fn fault_current_kiloamps(voltage_v: f32, impedance: &Impedance) -> f32 {
    if impedance.magnitude() < 1e-6 {
        return 0.0;
    }
    let ik = (voltage_v / impedance.magnitude()) * (3.0f32).sqrt();
    ik / 1000.0
}
