//! ---
//! ems_section: "11-simulation"
//! ems_subsection: "01-bootstrap"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Simulation runtime module exports and shared types."
//! ems_version: "v0.1.0"
//! ems_owner: "tbd"
//! ---
//! Simulation engines and telemetry helpers for the R-EMS project.
//!
//! For release guidelines refer to `/docs/VERSIONING.md`. The subsystem overview
//! is documented in `/docs/SIMULATION.md`.

pub mod frames;
pub mod generator;
pub mod replay;
pub mod visual;

pub use frames::TelemetryFrame;
pub use generator::{SimulationEngine as TelemetrySimulationEngine, SimulationMode};
pub use replay::{ReplayEngine, ScenarioFrame};
pub use visual::{
    ComponentKind, ComponentState, ComponentTelemetryFrame, FaultKind, GridComponent,
    GridSimulationEngine, SimulationControl as GridSimulationControl, TelemetrySink,
};
