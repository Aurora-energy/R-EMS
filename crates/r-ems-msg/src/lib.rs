//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Message schema helpers and protocol codecs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
#![warn(missing_docs)]

pub mod logging;
pub mod qos;
pub mod sim_hooks;
pub mod supervisor;
pub mod transport;
pub mod types;

/// Shared result type for messaging operations.
pub type Result<T> = std::result::Result<T, MessagingError>;

/// Lightweight error enumeration that will expand alongside subsystem
/// development.
#[derive(Debug, thiserror::Error)]
pub enum MessagingError {
    /// Raised when a feature is not yet implemented.
    #[error("messaging subsystem not yet implemented: {0}")]
    Unimplemented(&'static str),
    /// Wrapper for IO errors encountered during messaging operations.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Wrapper for JSON serialization or deserialization problems.
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

pub use logging::{log_message, MessageDirection, MessagingMetricsExporter};
pub use qos::{DeliveryGuarantee, QoSManager};
pub use sim_hooks::{publish_simulated_frame, replay_from_file};
pub use supervisor::{MessagingMetrics, MessagingSupervisor};
pub use transport::{InMemoryTransport, Transport, TransportConfig, TransportKind};
pub use types::{ControlCommand, Message, MessagePayload, Snapshot, SystemEvent, TelemetryFrame};
