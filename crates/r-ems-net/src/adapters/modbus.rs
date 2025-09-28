//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Network connectivity and edge adapters."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use super::{AdapterEvent, DeviceAdapter};

/// Simple configuration describing the Modbus endpoint. In this in-memory adapter the
/// endpoint is just a logical device identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModbusConfig {
    /// Logical identifier (e.g. IP:port in real deployments).
    pub device_id: String,
}

impl ModbusConfig {
    /// Create a new configuration referencing the supplied logical device id.
    pub fn new(device_id: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
        }
    }
}

/// In-memory Modbus adapter that simulates holding registers.
#[derive(Debug, Clone)]
pub struct ModbusAdapter {
    config: ModbusConfig,
    registers: Arc<Mutex<HashMap<u16, u16>>>,
}

impl ModbusAdapter {
    /// Build an in-memory adapter that simulates holding registers for the provided config.
    pub fn new(config: ModbusConfig) -> Self {
        Self {
            config,
            registers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Read a range of holding registers.
    pub async fn read_holding_registers(&self, start: u16, count: u16) -> anyhow::Result<Vec<u16>> {
        let registers = self.registers.lock().await;
        let mut values = Vec::with_capacity(count as usize);
        for offset in 0..count {
            let addr = start + offset;
            values.push(*registers.get(&addr).unwrap_or(&0));
        }
        Ok(values)
    }

    /// Write a single holding register.
    pub async fn write_holding_register(&self, address: u16, value: u16) -> anyhow::Result<()> {
        let mut registers = self.registers.lock().await;
        registers.insert(address, value);
        Ok(())
    }
}

#[async_trait]
impl DeviceAdapter for ModbusAdapter {
    async fn read(&self) -> anyhow::Result<Vec<AdapterEvent>> {
        let registers = self.registers.lock().await;
        let mut events = Vec::with_capacity(registers.len());
        for (address, value) in registers.iter() {
            events.push(AdapterEvent {
                tag: format!("{}:holding:{}", self.config.device_id, address),
                value: Value::from(*value),
            });
        }
        Ok(events)
    }

    async fn write(&self, tag: &str, value: Value) -> anyhow::Result<()> {
        let Some(address) = tag
            .rsplit(':')
            .next()
            .and_then(|part| part.parse::<u16>().ok())
        else {
            anyhow::bail!("invalid modbus tag: {tag}");
        };
        let numeric = value
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("modbus register writes require integer payloads"))?;
        if !(0..=u16::MAX as i64).contains(&numeric) {
            anyhow::bail!("value out of range for 16-bit register");
        }
        let mut registers = self.registers.lock().await;
        registers.insert(address, numeric as u16);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn modbus_adapter_read_write_cycle() {
        let adapter = ModbusAdapter::new(ModbusConfig::new("device-1"));
        adapter.write_holding_register(1, 123).await.unwrap();
        adapter.write_holding_register(2, 456).await.unwrap();

        let snapshot = adapter.read().await.unwrap();
        assert_eq!(snapshot.len(), 2);

        adapter
            .write("device-1:holding:3", json!(789))
            .await
            .unwrap();
        let values = adapter.read_holding_registers(1, 3).await.unwrap();
        assert_eq!(values, vec![123, 456, 789]);
    }
}
