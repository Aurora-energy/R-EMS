//! ---
//! ems_section: "07-resilience-fault-tolerance"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Redundancy planning and failover coordinators."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
//! Redundancy management and failover supervisor for R-EMS controllers.

mod controller;
mod supervisor;

pub use controller::{ControllerContext, ControllerRuntimeState, HeartbeatStatus};
pub use supervisor::{FailoverEvent, FailoverReason, Promotion, RedundancySupervisor};
