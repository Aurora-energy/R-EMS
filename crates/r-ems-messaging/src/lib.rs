//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Messaging orchestrators and IPC bindings."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
//! Messaging primitives for the R-EMS workspace.
//!
//! This crate will eventually provide message bus abstractions, envelope types,
//! and convenience helpers for publishing telemetry and control frames.

use r_ems_schema::SCHEMA_VERSION;
use uuid::Uuid;

/// Placeholder envelope type illustrating the shared structure for messages.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Envelope<T> {
    /// Unique message identifier.
    pub id: Uuid,
    /// Schema version carried by the payload.
    pub schema_version: u16,
    /// Embedded payload; concrete types live in `r-ems-schema`.
    pub payload: T,
}

impl<T> Envelope<T> {
    /// Construct a new placeholder envelope with the shared schema version.
    pub fn new(payload: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            schema_version: SCHEMA_VERSION,
            payload,
        }
    }
}
