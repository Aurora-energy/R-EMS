//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Transport implementations for messaging layers."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
//! Transport layer stubs for R-EMS messaging.
//!
//! Real implementations will provide TCP, Unix socket, and in-process channel
//! transports for the messaging envelope. This crate currently exposes a
//! placeholder trait to allow compilation while the detailed transports are
//! developed in future prompts.

use r_ems_messaging::Envelope;

/// Placeholder transport trait demonstrating the intended API surface.
pub trait Transport {
    /// Publishes a message to the underlying transport.
    fn send<T: Clone>(&self, message: Envelope<T>);
}

/// No-op transport used for compile-time scaffolding.
pub struct NullTransport;

impl Transport for NullTransport {
    fn send<T: Clone>(&self, _message: Envelope<T>) {
        tracing::debug!("null transport drop message");
    }
}
