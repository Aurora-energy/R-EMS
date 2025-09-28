//! ---
//! ems_section: "14-versioning-licensing-system"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Licensing enforcement and entitlement checks."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
#![warn(missing_docs)]

//! R-EMS licensing crate encapsulating license formats, certificate
//! verification, feature flag enforcement, and associated telemetry.

pub mod certificates;
pub mod core;
pub mod features;
pub mod logging;
