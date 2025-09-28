//! ---
//! ems_section: "03-persistence-logging"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Persistence abstractions and storage bindings."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
#![warn(missing_docs)]

/// Result alias used throughout the persistence crate.
pub type Result<T> = std::result::Result<T, PersistenceError>;

/// Error type for the persistence subsystem.
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    /// Wrapper for IO errors encountered while reading/writing persistence files.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Wrapper for JSON serialization issues.
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    /// Wrapper for CBOR serialization issues.
    #[error("cbor serialization error: {0}")]
    Cbor(#[from] serde_cbor::Error),
    /// Reported when a snapshot fails integrity verification.
    #[error("snapshot hash mismatch")]
    HashMismatch,
    /// Wrapper for Prometheus metrics registration failures.
    #[error("metrics error: {0}")]
    Metrics(#[from] prometheus::Error),
    /// Generic placeholder variant for unimplemented functionality.
    #[error("feature not yet implemented: {0}")]
    Unimplemented(&'static str),
}

pub mod event_log;
pub mod metrics;
pub mod snapshot;

pub use event_log::replay as replay_event_log;
pub use event_log::{EventLogEntry, EventLogReader, EventLogWriter};
pub use metrics::PersistenceMetrics;
pub use snapshot::{save_snapshot, verify_snapshot, ControllerState, SNAPSHOT_VERSION};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_error_debug() {
        let err = PersistenceError::Unimplemented("bootstrap");
        assert_eq!(format!("{err}"), "feature not yet implemented: bootstrap");
    }
}
