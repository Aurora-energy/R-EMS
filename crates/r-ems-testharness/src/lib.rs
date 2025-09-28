//! ---
//! ems_section: "11-simulation"
//! ems_subsection: "01-bootstrap"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Test harness orchestrator scaffolding and shared exports."
//! ems_version: "v0.1.0"
//! ems_owner: "tbd"
//! ---
//! The Simulation Test Harness crate coordinates scenario orchestration and
//! validation.
//!
//! Refer to `/docs/VERSIONING.md` for release coordination and
//! `/docs/TEST-HARNESS.md` for subsystem documentation.

/// Placeholder type ensuring the crate compiles while the orchestrator is
/// developed over subsequent implementation steps.
#[derive(Debug, Default, Clone)]
pub struct HarnessBootstrap;

impl HarnessBootstrap {
    /// Create a new bootstrap marker instance.
    pub fn new() -> Self {
        Self::default()
    }
}
