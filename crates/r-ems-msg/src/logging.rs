//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Message schema helpers and protocol codecs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::time::Duration;

use prometheus::{Histogram, HistogramOpts, IntCounter, Opts, Registry};
use tracing::debug;

use crate::types::Message;

/// Direction of the message movement, used for consistent logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageDirection {
    /// Message sent out via a transport.
    Outbound,
    /// Message received from a transport.
    Inbound,
    /// Message retried after a QoS timeout.
    Retry,
}

/// Emit a structured log entry for message activity.
pub fn log_message(direction: MessageDirection, message: &Message) {
    debug!(
        message_id = %message.id,
        timestamp = %message.timestamp,
        kind = message.kind(),
        schema_version = message.schema_version,
        direction = ?direction,
        "messaging activity"
    );
}

/// Prometheus metric handles for messaging activity.
pub struct MessagingMetricsExporter {
    sent: IntCounter,
    received: IntCounter,
    dropped: IntCounter,
    latency: Histogram,
}

impl MessagingMetricsExporter {
    /// Register messaging metrics with the provided registry.
    pub fn register(registry: &Registry) -> Result<Self, prometheus::Error> {
        let sent = IntCounter::with_opts(Opts::new(
            "messages_sent_total",
            "Messages published via transports",
        ))?;
        let received = IntCounter::with_opts(Opts::new(
            "messages_received_total",
            "Messages consumed from transports",
        ))?;
        let dropped = IntCounter::with_opts(Opts::new(
            "messages_dropped_total",
            "Messages that failed to deliver",
        ))?;
        let latency = Histogram::with_opts(HistogramOpts::new(
            "message_roundtrip_latency_seconds",
            "Observed latency between publish and acknowledgement",
        ))?;

        registry.register(Box::new(sent.clone()))?;
        registry.register(Box::new(received.clone()))?;
        registry.register(Box::new(dropped.clone()))?;
        registry.register(Box::new(latency.clone()))?;

        Ok(Self {
            sent,
            received,
            dropped,
            latency,
        })
    }

    /// Record a sent message.
    pub fn observe_sent(&self) {
        self.sent.inc();
    }

    /// Record a received message.
    pub fn observe_received(&self) {
        self.received.inc();
    }

    /// Record a dropped message.
    pub fn observe_dropped(&self) {
        self.dropped.inc();
    }

    /// Record message latency.
    pub fn observe_latency(&self, duration: Duration) {
        self.latency.observe(duration.as_secs_f64());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_exporter_records_counts() {
        let registry = Registry::new();
        let metrics = MessagingMetricsExporter::register(&registry).expect("register metrics");
        metrics.observe_sent();
        metrics.observe_received();
        metrics.observe_dropped();
        metrics.observe_latency(Duration::from_millis(10));

        let families = registry.gather();
        assert!(families
            .iter()
            .any(|f| f.get_name() == "messages_sent_total"));
    }
}
