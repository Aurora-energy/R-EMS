//! ---
//! ems_section: "08-energy-models-optimization"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Optimisation and calculation routines for energy planning."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
pub mod api;
pub mod cable_check;
pub mod errors;
pub mod io;
pub mod load_flow;
pub mod model;
pub mod reports;
pub mod short_circuit;
pub mod telemetry;

use chrono::{DateTime, Utc};
use model::SystemModel;
use telemetry::TelemetryFrame;
use tracing::info;

use crate::{
    cable_check::{validate_cables, CableCheckReport},
    load_flow::{run_load_flow, LoadFlowReport},
    reports::ReportExporter,
    short_circuit::{calculate_short_circuit, ShortCircuitReport},
};

pub use errors::{CalcEngineError, Result};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CalcSummary {
    pub timestamp: DateTime<Utc>,
    pub model_version: Option<String>,
    pub short_circuit: ShortCircuitReport,
    pub load_flow: LoadFlowReport,
    pub cable_check: CableCheckReport,
}

impl CalcSummary {
    pub fn exporter(&self) -> ReportExporter<'_> {
        ReportExporter::new(self)
    }
}

/// Runs the full analysis suite and writes reports to the default `reports/` directory.
///
/// For fallible usage, prefer [`analyze_system_with_options`].
pub fn analyze_system(model: &SystemModel, telemetry: &[TelemetryFrame]) -> CalcSummary {
    analyze_system_with_options(model, telemetry, None)
        .expect("calculation engine execution should succeed")
}

/// Runs the calculation engine with configurable export directory.
/// When `output_dir` is `None`, the default `reports/` directory at the workspace root is used.
pub fn analyze_system_with_options(
    model: &SystemModel,
    telemetry: &[TelemetryFrame],
    output_dir: Option<&std::path::Path>,
) -> Result<CalcSummary> {
    info!("Running short-circuit analysis...");
    let short_circuit = calculate_short_circuit(model)?;

    info!("Running load-flow analysis...");
    let load_flow = run_load_flow(model, telemetry)?;

    info!("Running cable validation checks...");
    let cable_check = validate_cables(model, telemetry, &load_flow)?;

    let summary = CalcSummary {
        timestamp: Utc::now(),
        model_version: model.version.clone(),
        short_circuit,
        load_flow,
        cable_check,
    };

    let default_dir = std::path::Path::new("reports");
    let output_dir = output_dir.unwrap_or(default_dir);
    summary.exporter().export_all(output_dir)?;

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        load_flow::BusVoltage,
        model::{
            CableMaterial, CableSpec, ComponentKind, Connection, GridComponent, Impedance,
            ProtectionDevice,
        },
        telemetry::TelemetryFrame,
    };
    use uuid::Uuid;

    #[test]
    fn analyze_system_pipeline() {
        let source_id = Uuid::new_v4();
        let bus_id = Uuid::new_v4();
        let load_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();

        let model = SystemModel {
            version: Some("test-1".to_string()),
            components: vec![
                GridComponent {
                    id: source_id,
                    name: "Source".into(),
                    kind: ComponentKind::Source,
                    nominal_voltage_kv: 0.48,
                    rated_power_kw: 500.0,
                    short_circuit_ratio: Some(6.0),
                    impedance: Some(Impedance {
                        resistance_ohm: 0.02,
                        reactance_ohm: 0.01,
                    }),
                    status: crate::model::ComponentStatus::Online,
                    is_faulted: false,
                },
                GridComponent {
                    id: bus_id,
                    name: "Main Bus".into(),
                    kind: ComponentKind::Bus,
                    nominal_voltage_kv: 0.48,
                    rated_power_kw: 500.0,
                    short_circuit_ratio: None,
                    impedance: Some(Impedance {
                        resistance_ohm: 0.01,
                        reactance_ohm: 0.005,
                    }),
                    status: crate::model::ComponentStatus::Faulted,
                    is_faulted: true,
                },
                GridComponent {
                    id: load_id,
                    name: "Load".into(),
                    kind: ComponentKind::Load,
                    nominal_voltage_kv: 0.48,
                    rated_power_kw: 200.0,
                    short_circuit_ratio: None,
                    impedance: Some(Impedance {
                        resistance_ohm: 0.05,
                        reactance_ohm: 0.02,
                    }),
                    status: crate::model::ComponentStatus::Online,
                    is_faulted: false,
                },
            ],
            connections: vec![
                Connection {
                    id: connection_id,
                    from: source_id,
                    to: bus_id,
                    cable: CableSpec {
                        name: "Feeder".into(),
                        length_m: 50.0,
                        cross_section_mm2: 35.0,
                        material: CableMaterial::Copper,
                        ampacity_a: 400.0,
                        resistance_per_km_ohm: 0.524,
                        reactance_per_km_ohm: 0.08,
                        voltage_drop_limit_percent: 5.0,
                        withstand_ka: 10.0,
                        breaker: Some(ProtectionDevice {
                            breaker_id: Uuid::new_v4(),
                            rating_ka: 6.0,
                        }),
                    },
                },
                Connection {
                    id: Uuid::new_v4(),
                    from: bus_id,
                    to: load_id,
                    cable: CableSpec {
                        name: "Load Cable".into(),
                        length_m: 30.0,
                        cross_section_mm2: 25.0,
                        material: CableMaterial::Copper,
                        ampacity_a: 80.0,
                        resistance_per_km_ohm: 0.868,
                        reactance_per_km_ohm: 0.08,
                        voltage_drop_limit_percent: 2.5,
                        withstand_ka: 5.0,
                        breaker: None,
                    },
                },
            ],
        };

        let telemetry = vec![
            TelemetryFrame {
                timestamp: "2024-05-01T00:00:00Z".into(),
                component_id: load_id,
                voltage: 480.0,
                current: 120.0,
                power_kw: 55.0,
                temperature: 35.0,
                status: "online".into(),
            },
            TelemetryFrame {
                timestamp: "2024-05-01T00:00:00Z".into(),
                component_id: bus_id,
                voltage: 480.0,
                current: 150.0,
                power_kw: 65.0,
                temperature: 32.0,
                status: "fault".into(),
            },
        ];

        let summary = analyze_system_with_options(
            &model,
            &telemetry,
            Some(std::path::Path::new("reports/test")),
        )
        .unwrap();

        assert!(summary.short_circuit.ik > 0.0);
        assert_eq!(summary.short_circuit.fault_location, bus_id);
        assert_eq!(summary.model_version.as_deref(), Some("test-1"));
        assert!(summary
            .load_flow
            .bus_voltages
            .iter()
            .any(|BusVoltage { component_id, .. }| *component_id == bus_id));
        assert!(!summary.cable_check.undersized.is_empty());
    }
}
