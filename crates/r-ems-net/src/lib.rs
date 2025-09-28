//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Network connectivity and edge adapters."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
#![warn(missing_docs)]

pub mod adapters;
pub mod grpc;
pub mod rest;
pub mod websocket;

/// Placeholder type that advertises the networking crate identity.
#[derive(Debug, Default)]
pub struct NetworkingPlaceholder;

impl NetworkingPlaceholder {
    /// Return the crate identifier for logging or diagnostics.
    pub fn name() -> &'static str {
        "r-ems-net"
    }
}

pub use rest::{
    CommandAuthoriser, CommandError, CommandHandler, CommandRequest, CommandResponse,
    ControllerStatus, GridStatus, RestApiBuilder, RestApiHandle, StaticApiKeyAuthoriser,
    StatusProvider, StatusSnapshot,
};

pub use adapters::{modbus::ModbusAdapter, modbus::ModbusConfig, AdapterEvent, DeviceAdapter};
pub use grpc::{proto, GrpcServerBuilder, GrpcServerHandle};
pub use websocket::{
    TelemetryBroadcaster, TelemetryFrame, WebSocketServerBuilder, WebSocketServerHandle,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_name_matches() {
        assert_eq!(NetworkingPlaceholder::name(), "r-ems-net");
    }
}
