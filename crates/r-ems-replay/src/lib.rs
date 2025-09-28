//! ---
//! ems_section: "11-simulation-test-harness"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Replay pipeline for simulation and test harnesses."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
//! Replay utilities for the R-EMS messaging workspace.
//!
//! The replay subsystem will eventually record envelopes to durable storage and
//! re-inject them into transports for simulation and deterministic testing.

use r_ems_messaging::Envelope;

/// Placeholder log entry wrapper.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplayRecord<T> {
    /// Stored envelope to be replayed.
    pub envelope: Envelope<T>,
}

/// Placeholder replay engine.
pub struct ReplayEngine;

impl ReplayEngine {
    /// Creates a new placeholder replay engine.
    pub fn new() -> Self {
        Self
    }

    /// Stores a replay record; currently a no-op.
    pub fn store<T: Clone>(&self, _record: ReplayRecord<T>) {
        tracing::trace!("store replay record (noop)");
    }
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new()
    }
}
