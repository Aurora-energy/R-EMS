//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Runtime helpers supporting the orchestrator."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
//! Real-time scheduling helpers for the R-EMS runtime.

pub mod scheduling;

pub use scheduling::{DeterministicExecutor, RateLimiter};
