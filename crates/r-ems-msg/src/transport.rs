//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Message schema helpers and protocol codecs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::{Message, MessagingError, Result};

/// Transport abstraction used by all messaging backends.
pub trait Transport: Send + Sync {
    /// Send a message into the transport.
    fn send(&self, msg: Message) -> Result<()>;
    /// Receive the next message from the transport, if available.
    fn recv(&self) -> Option<Message>;
    /// Human-readable transport name for logging/metrics.
    fn name(&self) -> &'static str;
}

/// Transport kinds supported by the subsystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// Local in-memory channel, primarily for tests and single-process integration.
    InMemory,
    /// TCP transport (future implementation).
    Tcp,
    /// WebSocket transport (future implementation).
    WebSocket,
}

/// Configuration describing transports enabled for a runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Enable the in-memory transport.
    #[serde(default = "TransportConfig::default_in_memory")]
    pub in_memory_enabled: bool,
    /// Optional TCP listener address.
    #[serde(default)]
    pub tcp_listen: Option<SocketAddr>,
    /// Optional WebSocket listener address.
    #[serde(default)]
    pub websocket_listen: Option<SocketAddr>,
}

impl TransportConfig {
    const fn default_in_memory() -> bool {
        true
    }
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            in_memory_enabled: true,
            tcp_listen: None,
            websocket_listen: None,
        }
    }
}

/// In-memory transport backed by a mutex protected queue.
#[derive(Clone, Default)]
pub struct InMemoryTransport {
    queue: Arc<Mutex<VecDeque<Message>>>,
}

impl InMemoryTransport {
    /// Create a new in-memory transport channel.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Transport for InMemoryTransport {
    fn send(&self, msg: Message) -> Result<()> {
        let mut guard = self.queue.lock().expect("queue poisoned");
        guard.push_back(msg);
        Ok(())
    }

    fn recv(&self) -> Option<Message> {
        let mut guard = self.queue.lock().expect("queue poisoned");
        guard.pop_front()
    }

    fn name(&self) -> &'static str {
        "in_memory"
    }
}

/// Placeholder TCP transport.
pub struct TcpTransport;

impl Transport for TcpTransport {
    fn send(&self, _msg: Message) -> Result<()> {
        Err(MessagingError::Unimplemented("tcp transport"))
    }

    fn recv(&self) -> Option<Message> {
        None
    }

    fn name(&self) -> &'static str {
        "tcp"
    }
}

/// Placeholder WebSocket transport.
pub struct WebSocketTransport;

impl Transport for WebSocketTransport {
    fn send(&self, _msg: Message) -> Result<()> {
        Err(MessagingError::Unimplemented("websocket transport"))
    }

    fn recv(&self) -> Option<Message> {
        None
    }

    fn name(&self) -> &'static str {
        "websocket"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        MessagePayload, SystemEvent, SystemEventType, TelemetryFrame, TelemetryValues,
    };

    #[test]
    fn in_memory_transport_send_and_recv() {
        let transport = InMemoryTransport::default();

        let mut values = TelemetryValues::new();
        values.insert("power".into(), 12.5);
        let frame = TelemetryFrame::new("grid-a", "controller-a", values);
        let message = Message::new(MessagePayload::Telemetry(frame));

        transport.send(message.clone()).expect("send succeeds");
        let received = transport.recv().expect("message available");
        assert_eq!(received.kind(), message.kind());
    }

    #[test]
    fn placeholder_transports_return_unimplemented() {
        let tcp = TcpTransport;
        let ws = WebSocketTransport;
        let message = Message::new(MessagePayload::System(SystemEvent::new(
            SystemEventType::Custom,
            serde_json::json!({}),
        )));

        assert!(matches!(
            tcp.send(message.clone()),
            Err(MessagingError::Unimplemented("tcp transport"))
        ));
        assert!(matches!(
            ws.send(message),
            Err(MessagingError::Unimplemented("websocket transport"))
        ));
    }
}
