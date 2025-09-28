//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Primary orchestration and lifecycle management."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
//! Core orchestrator, runtime lifecycle, and update management for R-EMS.

pub mod integration_persistence;
pub mod orchestrator;
pub mod state;
pub mod update;

pub use orchestrator::{GridHandle, OrchestratorHandle, RemsOrchestrator};
pub use state::{ControllerSnapshot, SnapshotStore};
pub use update::{UpdateClient, UpdateCommand, UpdateResult, UpdateSource};
