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
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemModel {
    #[serde(default)]
    pub version: Option<String>,
    pub components: Vec<GridComponent>,
    pub connections: Vec<Connection>,
}

impl SystemModel {
    pub fn component_map(&self) -> HashMap<Uuid, &GridComponent> {
        self.components.iter().map(|c| (c.id, c)).collect()
    }

    pub fn find_component(&self, id: Uuid) -> Option<&GridComponent> {
        self.components.iter().find(|c| c.id == id)
    }

    pub fn faulted_component(&self) -> Option<&GridComponent> {
        self.components.iter().find(|c| c.is_faulted)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridComponent {
    pub id: Uuid,
    pub name: String,
    pub kind: ComponentKind,
    pub nominal_voltage_kv: f32,
    pub rated_power_kw: f32,
    #[serde(default)]
    pub short_circuit_ratio: Option<f32>,
    #[serde(default)]
    pub impedance: Option<Impedance>,
    pub status: ComponentStatus,
    #[serde(default)]
    pub is_faulted: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComponentKind {
    Source,
    Bus,
    Load,
    Storage,
    Breaker,
    Inverter,
    Transformer,
    Meter,
    Cable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComponentStatus {
    Online,
    Offline,
    Faulted,
    Maintenance,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Impedance {
    pub resistance_ohm: f32,
    pub reactance_ohm: f32,
}

impl Impedance {
    pub fn magnitude(&self) -> f32 {
        (self.resistance_ohm.powi(2) + self.reactance_ohm.powi(2)).sqrt()
    }

    pub fn add(&self, other: &Impedance) -> Impedance {
        Impedance {
            resistance_ohm: self.resistance_ohm + other.resistance_ohm,
            reactance_ohm: self.reactance_ohm + other.reactance_ohm,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: Uuid,
    pub from: Uuid,
    pub to: Uuid,
    pub cable: CableSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CableSpec {
    pub name: String,
    pub length_m: f32,
    pub cross_section_mm2: f32,
    pub material: CableMaterial,
    pub ampacity_a: f32,
    pub resistance_per_km_ohm: f32,
    pub reactance_per_km_ohm: f32,
    pub voltage_drop_limit_percent: f32,
    pub withstand_ka: f32,
    #[serde(default)]
    pub breaker: Option<ProtectionDevice>,
}

impl CableSpec {
    pub fn impedance(&self) -> Impedance {
        let length_km = self.length_m / 1000.0;
        Impedance {
            resistance_ohm: self.resistance_per_km_ohm * length_km,
            reactance_ohm: self.reactance_per_km_ohm * length_km,
        }
    }

    pub fn voltage_drop(&self, current_a: f32, nominal_voltage: f32) -> f32 {
        let impedance = self.impedance();
        let drop = current_a * impedance.resistance_ohm;
        (drop / nominal_voltage) * 100.0
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CableMaterial {
    Copper,
    Aluminium,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectionDevice {
    pub breaker_id: Uuid,
    pub rating_ka: f32,
}
