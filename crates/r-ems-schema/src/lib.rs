//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Shared schema definitions and validation logic."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
//! Schema definitions for R-EMS messaging layer.
//!
//! This crate hosts strongly typed data models that carry sensor telemetry,
//! actuator commands, supervisory messages, and simulation frames. All schema
//! types are version-tagged to support evolution and compatibility checks.

/// Placeholder schema version constant used until concrete versions are defined.
pub const SCHEMA_VERSION: u16 = 1;

/// Shared result type for schema validation routines.
pub type SchemaResult<T> = Result<T, SchemaError>;

/// Placeholder error type representing schema issues.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// Raised when a schema version is incompatible with the current runtime.
    #[error("schema version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: u16, found: u16 },
}

/// Placeholder sensor frame structure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SensorFrame {
    /// Schema version attached to the frame.
    pub schema_version: u16,
    /// Placeholder voltage values.
    pub voltages: Vec<f32>,
}

impl SensorFrame {
    /// Construct a placeholder frame using the canonical schema version.
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            voltages: Vec::new(),
        }
    }
}

impl Default for SensorFrame {
    fn default() -> Self {
        Self::new()
    }
}
