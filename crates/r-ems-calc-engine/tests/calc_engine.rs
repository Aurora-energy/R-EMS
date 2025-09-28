//! ---
//! ems_section: "08-energy-models-optimization"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Optimisation and calculation routines for energy planning."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;

use r_ems_calc_engine::{
    analyze_system_with_options,
    model::{
        CableMaterial, CableSpec, ComponentKind, ComponentStatus, Connection, GridComponent,
        Impedance, ProtectionDevice, SystemModel,
    },
    telemetry::TelemetryFrame,
};
use tempfile::tempdir;
use uuid::Uuid;

fn sample_model() -> SystemModel {
    let slack = Uuid::new_v4();
    let inverter = Uuid::new_v4();
    let load = Uuid::new_v4();
    let storage = Uuid::new_v4();

    SystemModel {
        version: Some("integration-test".into()),
        components: vec![
            GridComponent {
                id: slack,
                name: "Diesel Genset".into(),
                kind: ComponentKind::Source,
                nominal_voltage_kv: 0.48,
                rated_power_kw: 800.0,
                short_circuit_ratio: Some(7.0),
                impedance: Some(Impedance {
                    resistance_ohm: 0.015,
                    reactance_ohm: 0.007,
                }),
                status: ComponentStatus::Online,
                is_faulted: false,
            },
            GridComponent {
                id: inverter,
                name: "Battery Inverter".into(),
                kind: ComponentKind::Inverter,
                nominal_voltage_kv: 0.48,
                rated_power_kw: 300.0,
                short_circuit_ratio: Some(4.0),
                impedance: Some(Impedance {
                    resistance_ohm: 0.02,
                    reactance_ohm: 0.01,
                }),
                status: ComponentStatus::Faulted,
                is_faulted: true,
            },
            GridComponent {
                id: load,
                name: "Process Load".into(),
                kind: ComponentKind::Load,
                nominal_voltage_kv: 0.48,
                rated_power_kw: 250.0,
                short_circuit_ratio: None,
                impedance: Some(Impedance {
                    resistance_ohm: 0.08,
                    reactance_ohm: 0.03,
                }),
                status: ComponentStatus::Online,
                is_faulted: false,
            },
            GridComponent {
                id: storage,
                name: "Battery Storage".into(),
                kind: ComponentKind::Storage,
                nominal_voltage_kv: 0.48,
                rated_power_kw: 150.0,
                short_circuit_ratio: Some(3.0),
                impedance: Some(Impedance {
                    resistance_ohm: 0.04,
                    reactance_ohm: 0.015,
                }),
                status: ComponentStatus::Online,
                is_faulted: false,
            },
        ],
        connections: vec![
            Connection {
                id: Uuid::new_v4(),
                from: slack,
                to: inverter,
                cable: CableSpec {
                    name: "Feeder A".into(),
                    length_m: 40.0,
                    cross_section_mm2: 50.0,
                    material: CableMaterial::Copper,
                    ampacity_a: 320.0,
                    resistance_per_km_ohm: 0.386,
                    reactance_per_km_ohm: 0.07,
                    voltage_drop_limit_percent: 5.0,
                    withstand_ka: 8.0,
                    breaker: Some(ProtectionDevice {
                        breaker_id: Uuid::new_v4(),
                        rating_ka: 5.0,
                    }),
                },
            },
            Connection {
                id: Uuid::new_v4(),
                from: inverter,
                to: load,
                cable: CableSpec {
                    name: "Feeder B".into(),
                    length_m: 60.0,
                    cross_section_mm2: 16.0,
                    material: CableMaterial::Copper,
                    ampacity_a: 110.0,
                    resistance_per_km_ohm: 1.15,
                    reactance_per_km_ohm: 0.09,
                    voltage_drop_limit_percent: 5.0,
                    withstand_ka: 4.0,
                    breaker: None,
                },
            },
            Connection {
                id: Uuid::new_v4(),
                from: inverter,
                to: storage,
                cable: CableSpec {
                    name: "Battery Tie".into(),
                    length_m: 20.0,
                    cross_section_mm2: 35.0,
                    material: CableMaterial::Copper,
                    ampacity_a: 180.0,
                    resistance_per_km_ohm: 0.524,
                    reactance_per_km_ohm: 0.08,
                    voltage_drop_limit_percent: 5.0,
                    withstand_ka: 6.0,
                    breaker: Some(ProtectionDevice {
                        breaker_id: Uuid::new_v4(),
                        rating_ka: 6.0,
                    }),
                },
            },
        ],
    }
}

fn sample_telemetry(model: &SystemModel) -> Vec<TelemetryFrame> {
    model
        .components
        .iter()
        .map(|component| TelemetryFrame {
            timestamp: "2024-05-01T00:00:00Z".into(),
            component_id: component.id,
            voltage: component.nominal_voltage_kv * 1000.0,
            current: if matches!(component.kind, ComponentKind::Load) {
                140.0
            } else {
                90.0
            },
            power_kw: component.rated_power_kw * 0.4,
            temperature: 30.0,
            status: if component.is_faulted {
                "fault".into()
            } else {
                "online".into()
            },
        })
        .collect()
}

#[test]
fn run_full_calculation_pipeline() {
    let model = sample_model();
    let telemetry = sample_telemetry(&model);
    let temp = tempdir().expect("temp dir");

    let summary =
        analyze_system_with_options(&model, &telemetry, Some(temp.path())).expect("analysis");

    assert!(summary.short_circuit.ik > 0.1 && summary.short_circuit.ik < 30.0);
    assert_eq!(summary.short_circuit.fault_location, model.components[1].id);

    assert_eq!(summary.load_flow.bus_voltages.len(), model.components.len());
    assert!(summary
        .load_flow
        .line_currents
        .iter()
        .any(|line| line.current.abs() > 0.0));

    assert!(!summary.cable_check.undersized.is_empty());

    let short_path = temp.path().join("short_circuit.json");
    let load_path = temp.path().join("load_flow.json");
    let cable_path = temp.path().join("cable_check.json");

    let short_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(short_path).unwrap()).unwrap();
    assert!(short_json["data"]["ik"].as_f64().unwrap() > 0.1);

    let load_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(load_path).unwrap()).unwrap();
    assert!(load_json["data"]["bus_voltages"].as_array().unwrap().len() >= 3);

    let cable_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(cable_path).unwrap()).unwrap();
    assert!(cable_json["data"]["undersized"].as_array().unwrap().len() >= 1);
}
