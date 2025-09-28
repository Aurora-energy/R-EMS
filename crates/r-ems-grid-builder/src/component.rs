//! ---
//! ems_section: "09-integration-interoperability"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Grid modelling helpers for partner integrations."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use serde::{Deserialize, Serialize};

/// Types of components supported by the visual grid builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    Motor,
    Inverter,
    Battery,
    Cable,
    Breaker,
    Busbar,
    Load,
}

impl ComponentKind {
    /// Returns all component kinds supported by the renderer.
    pub const fn all() -> &'static [ComponentKind] {
        &[
            ComponentKind::Motor,
            ComponentKind::Inverter,
            ComponentKind::Battery,
            ComponentKind::Cable,
            ComponentKind::Breaker,
            ComponentKind::Busbar,
            ComponentKind::Load,
        ]
    }

    /// Canonical slug used in the icon mapping file.
    pub fn slug(self) -> &'static str {
        match self {
            ComponentKind::Motor => "motor",
            ComponentKind::Inverter => "inverter",
            ComponentKind::Battery => "battery",
            ComponentKind::Cable => "cable",
            ComponentKind::Breaker => "breaker",
            ComponentKind::Busbar => "busbar",
            ComponentKind::Load => "load",
        }
    }
}

/// Operational state information for a component node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentStatus {
    Healthy,
    Fault,
    Offline,
}

impl Default for ComponentStatus {
    fn default() -> Self {
        ComponentStatus::Healthy
    }
}

/// Runtime state passed to the GUI renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentState {
    pub kind: ComponentKind,
    #[serde(default)]
    pub status: ComponentStatus,
}

impl ComponentState {
    pub fn new(kind: ComponentKind) -> Self {
        Self {
            kind,
            status: ComponentStatus::Healthy,
        }
    }

    pub fn with_status(kind: ComponentKind, status: ComponentStatus) -> Self {
        Self { kind, status }
    }
}
