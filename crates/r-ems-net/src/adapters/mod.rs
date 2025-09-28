//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Network connectivity and edge adapters."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use async_trait::async_trait;
use serde_json::Value;

/// Metadata describing a field update produced by an adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct AdapterEvent {
    /// Logical tag identifier (e.g. `grid-a.voltage`).
    pub tag: String,
    /// Value reported by the device.
    pub value: Value,
}

/// Unified interface implemented by protocol-specific adapters.
#[async_trait]
pub trait DeviceAdapter: Send + Sync {
    /// Connect to a device using the adapter-specific endpoint/connection info.
    async fn connect(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Read data from the device and translate into a generic event list.
    async fn read(&self) -> anyhow::Result<Vec<AdapterEvent>>;

    /// Write a control value to the device.
    async fn write(&self, tag: &str, value: Value) -> anyhow::Result<()>;
}

pub mod iec104;
pub mod modbus;
pub mod opcua;
