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
use tracing::warn;

use super::{AdapterEvent, DeviceAdapter};

/// Stub implementation for IEC 60870-5-104 integration.
#[derive(Debug, Default, Clone)]
pub struct Iec104Adapter;

#[async_trait]
impl DeviceAdapter for Iec104Adapter {
    async fn read(&self) -> anyhow::Result<Vec<AdapterEvent>> {
        warn!("iec104 adapter invoked but not yet implemented");
        Ok(Vec::new())
    }

    async fn write(&self, _tag: &str, _value: Value) -> anyhow::Result<()> {
        warn!("iec104 adapter write requested but not yet implemented");
        anyhow::bail!("iec104 adapter not implemented")
    }
}
