//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Message schema helpers and protocol codecs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Schema version broadcast alongside every message payload.
pub const SCHEMA_VERSION: u16 = 1;

/// Convenience alias for strongly-typed metric/value collections.
pub type TelemetryValues = BTreeMap<String, f64>;

/// Message envelope describing the payload carried on the bus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum MessagePayload {
    /// Lifecycle or system level event.
    System(SystemEvent),
    /// Telemetry sample emitted by a controller.
    Telemetry(TelemetryFrame),
    /// Command dispatched towards a controller or subsystem.
    Command(ControlCommand),
    /// Snapshot of orchestrator/controller state.
    Snapshot(Snapshot),
}

/// Unified message structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for deduplication and tracing.
    pub id: Uuid,
    /// Version of the schema used by the payload.
    pub schema_version: u16,
    /// Timestamp when the message was created.
    pub timestamp: DateTime<Utc>,
    /// Actual payload carried by the message.
    pub payload: MessagePayload,
}

impl Message {
    /// Construct a new message envelope around the provided payload.
    pub fn new(payload: MessagePayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            schema_version: SCHEMA_VERSION,
            timestamp: Utc::now(),
            payload,
        }
    }

    /// Convenience accessor returning the payload kind as a static string.
    pub fn kind(&self) -> &'static str {
        match &self.payload {
            MessagePayload::System(_) => "system",
            MessagePayload::Telemetry(_) => "telemetry",
            MessagePayload::Command(_) => "command",
            MessagePayload::Snapshot(_) => "snapshot",
        }
    }
}

/// Classification of system events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemEventType {
    /// Orchestrator lifecycle milestone (startup/shutdown).
    Lifecycle,
    /// Failover or redundancy change.
    Failover,
    /// Operator action was performed.
    OperatorAction,
    /// Arbitrary custom event.
    Custom,
}

/// System level event model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemEvent {
    /// Event identifier.
    pub id: Uuid,
    /// Timestamp when the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Event classification.
    pub event_type: SystemEventType,
    /// Arbitrary payload attached to the event.
    #[serde(default)]
    pub payload: JsonValue,
}

impl SystemEvent {
    /// Constructs a system event with the supplied metadata.
    pub fn new(event_type: SystemEventType, payload: JsonValue) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            payload,
        }
    }
}

/// Telemetry sample emitted by a controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryFrame {
    /// Logical grid identifier.
    pub grid_id: String,
    /// Controller identifier within the grid.
    pub controller_id: String,
    /// Numeric telemetry values keyed by name.
    #[serde(default)]
    pub values: TelemetryValues,
    /// Timestamp of the sample.
    pub timestamp: DateTime<Utc>,
}

impl TelemetryFrame {
    /// Construct a telemetry frame from the provided values.
    pub fn new(
        grid_id: impl Into<String>,
        controller_id: impl Into<String>,
        values: TelemetryValues,
    ) -> Self {
        Self {
            grid_id: grid_id.into(),
            controller_id: controller_id.into(),
            values,
            timestamp: Utc::now(),
        }
    }
}

/// Target of a control command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandTarget {
    /// Grid identifier.
    pub grid_id: String,
    /// Optional controller within the grid. `None` targets the entire grid.
    pub controller_id: Option<String>,
}

impl CommandTarget {
    /// Construct a target for an entire grid.
    pub fn grid(grid_id: impl Into<String>) -> Self {
        Self {
            grid_id: grid_id.into(),
            controller_id: None,
        }
    }

    /// Construct a target for a specific controller.
    pub fn controller(grid_id: impl Into<String>, controller_id: impl Into<String>) -> Self {
        Self {
            grid_id: grid_id.into(),
            controller_id: Some(controller_id.into()),
        }
    }
}

/// Control command dispatched to the runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlCommand {
    /// Target grid/controller combo.
    pub target: CommandTarget,
    /// Action identifier.
    pub action: String,
    /// Arbitrary parameters encoded as JSON.
    #[serde(default)]
    pub params: JsonValue,
    /// Timestamp when the command was issued.
    pub timestamp: DateTime<Utc>,
}

impl ControlCommand {
    /// Construct a new control command.
    pub fn new(target: CommandTarget, action: impl Into<String>, params: JsonValue) -> Self {
        Self {
            target,
            action: action.into(),
            params,
            timestamp: Utc::now(),
        }
    }
}

/// Snapshot of orchestrator/controller state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Snapshot identifier for dedupe.
    pub id: Uuid,
    /// Serialized state representation.
    pub state: JsonValue,
    /// Integrity hash for the snapshot payload.
    pub hash: String,
    /// Timestamp of the snapshot.
    pub timestamp: DateTime<Utc>,
}

impl Snapshot {
    /// Construct a new snapshot payload.
    pub fn new(state: JsonValue, hash: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            state,
            hash: hash.into(),
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_cbor(message: &Message) -> Message {
        let bytes = serde_cbor::to_vec(message).expect("serialize cbor");
        serde_cbor::from_slice(&bytes).expect("deserialize cbor")
    }

    fn roundtrip_json(message: &Message) -> Message {
        let json = serde_json::to_string(message).expect("serialize json");
        serde_json::from_str(&json).expect("deserialize json")
    }

    #[test]
    fn json_and_cbor_roundtrip_preserve_payloads() {
        let mut values = TelemetryValues::new();
        values.insert("voltage".to_string(), 418.2);
        values.insert("current".to_string(), 12.4);

        let telemetry = TelemetryFrame::new("grid-a", "controller-1", values);
        let message = Message::new(MessagePayload::Telemetry(telemetry.clone()));

        let json_roundtrip = roundtrip_json(&message);
        assert_eq!(message.kind(), json_roundtrip.kind());
        match json_roundtrip.payload {
            MessagePayload::Telemetry(frame) => assert_eq!(frame, telemetry),
            _ => panic!("unexpected payload"),
        }

        let cbor_roundtrip = roundtrip_cbor(&message);
        assert_eq!(message.kind(), cbor_roundtrip.kind());
        match cbor_roundtrip.payload {
            MessagePayload::Telemetry(frame) => assert_eq!(frame, telemetry),
            _ => panic!("unexpected payload"),
        }
    }

    #[test]
    fn system_event_helpers_populate_defaults() {
        let payload = {
            let mut map = serde_json::Map::new();
            map.insert("note".to_string(), JsonValue::from("testing"));
            JsonValue::Object(map)
        };
        let event = SystemEvent::new(SystemEventType::Lifecycle, payload.clone());
        assert_eq!(event.event_type, SystemEventType::Lifecycle);
        assert_eq!(event.payload, payload);
    }

    #[test]
    fn control_command_target_builders() {
        let grid = CommandTarget::grid("grid-a");
        assert_eq!(grid.grid_id, "grid-a");
        assert!(grid.controller_id.is_none());

        let controller = CommandTarget::controller("grid-a", "c1");
        assert_eq!(controller.grid_id, "grid-a");
        assert_eq!(controller.controller_id.as_deref(), Some("c1"));
    }
}
