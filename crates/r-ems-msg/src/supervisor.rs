//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Message schema helpers and protocol codecs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::{DeliveryGuarantee, Message, MessagePayload, QoSManager, Result, Transport};

/// Snapshot of messaging metrics used by dashboards and monitoring.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MessagingMetrics {
    /// Number of messages successfully handed to transports.
    pub sent: u64,
    /// Number of messages received from transports.
    pub received: u64,
    /// Number of messages dropped due to transport errors.
    pub dropped: u64,
}

struct Counters {
    sent: AtomicU64,
    received: AtomicU64,
    dropped: AtomicU64,
}

impl Counters {
    fn new() -> Self {
        Self {
            sent: AtomicU64::new(0),
            received: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> MessagingMetrics {
        MessagingMetrics {
            sent: self.sent.load(Ordering::Relaxed),
            received: self.received.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
        }
    }
}

/// Coordinates transports, quality-of-service, and metrics emission.
pub struct MessagingSupervisor {
    transports: Vec<Arc<dyn Transport>>,
    qos: QoSManager,
    counters: Counters,
}

impl MessagingSupervisor {
    /// Construct a supervisor with the provided QoS policy.
    pub fn new(guarantee: DeliveryGuarantee) -> Self {
        Self {
            transports: Vec::new(),
            qos: QoSManager::new(guarantee),
            counters: Counters::new(),
        }
    }

    /// Register a transport for publish/receive operations.
    pub fn register_transport<T>(&mut self, transport: Arc<T>)
    where
        T: Transport + 'static,
    {
        self.transports.push(transport as Arc<dyn Transport>);
    }

    /// Publish a payload to all registered transports.
    pub fn publish(&self, payload: MessagePayload) -> Result<u64> {
        let message = Message::new(payload);
        let (sequence, message_with_sequence) = self.qos.register(message);

        for transport in &self.transports {
            if let Err(err) = transport.send(message_with_sequence.clone()) {
                tracing::warn!(transport = transport.name(), error = %err, "transport send failed");
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            } else {
                self.counters.sent.fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(sequence)
    }

    /// Poll transports for any available messages.
    pub fn poll(&self) -> Vec<Message> {
        let mut collected = Vec::new();
        for transport in &self.transports {
            while let Some(message) = transport.recv() {
                self.counters.received.fetch_add(1, Ordering::Relaxed);
                collected.push(message);
            }
        }
        collected
    }

    /// Acknowledge a sequence after it has been processed by consumers.
    pub fn acknowledge(&self, sequence: u64) {
        self.qos.acknowledge(sequence);
    }

    /// Retry outstanding messages according to the QoS policy.
    pub fn retry_pending(&self) {
        for (sequence, message) in self.qos.pending_for_retry() {
            tracing::debug!(sequence, "retrying pending message");
            for transport in &self.transports {
                if let Err(err) = transport.send(message.clone()) {
                    tracing::warn!(transport = transport.name(), error = %err, "retry send failed");
                    self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                } else {
                    self.counters.sent.fetch_add(1, Ordering::Relaxed);
                }
            }
            // keep message pending until positive acknowledgement arrives
            // (QoSManager retains it internally).
        }
    }

    /// Drain all pending messages (e.g., on shutdown) and return them to the caller.
    pub fn drain_pending(&self) -> Vec<(u64, Message)> {
        self.qos.drain_pending()
    }

    /// Return the current metrics snapshot.
    pub fn metrics(&self) -> MessagingMetrics {
        self.counters.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::InMemoryTransport;
    use crate::types::{MessagePayload, TelemetryFrame, TelemetryValues};

    fn supervisor_with_in_memory() -> (MessagingSupervisor, Arc<InMemoryTransport>) {
        let mut supervisor = MessagingSupervisor::new(DeliveryGuarantee::AtLeastOnce {
            max_retries: 1,
            retry_interval: std::time::Duration::from_millis(1),
        });
        let transport = Arc::new(InMemoryTransport::new());
        supervisor.register_transport(transport.clone());
        (supervisor, transport)
    }

    #[test]
    fn publish_and_poll_cycle() {
        let (supervisor, transport) = supervisor_with_in_memory();

        let mut values = TelemetryValues::new();
        values.insert("frequency".into(), 50.0);
        let payload = MessagePayload::Telemetry(TelemetryFrame::new("grid-a", "c1", values));
        let sequence = supervisor.publish(payload).expect("publish succeeds");

        // simulate another component reading from the same transport
        let received = transport.recv().expect("message available");
        assert_eq!(received.kind(), "telemetry");
        supervisor.acknowledge(sequence);

        let metrics = supervisor.metrics();
        assert_eq!(metrics.sent, 1);
    }

    #[test]
    fn retry_pending_messages() {
        let (supervisor, transport) = supervisor_with_in_memory();
        supervisor
            .publish(MessagePayload::System(crate::types::SystemEvent::new(
                crate::types::SystemEventType::Failover,
                serde_json::json!({ "grid": "a" }),
            )))
            .expect("publish succeeds");

        // drop the message (simulate loss) by not draining transport.
        // After retry interval, retry_pending should resend.
        std::thread::sleep(std::time::Duration::from_millis(2));
        supervisor.retry_pending();

        let retry = transport.recv().expect("retry delivered");
        assert_eq!(retry.kind(), "system");
    }
}
