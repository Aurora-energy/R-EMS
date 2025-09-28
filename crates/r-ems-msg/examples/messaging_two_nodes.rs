//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Message schema helpers and protocol codecs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::sync::Arc;
use std::time::Duration;

use r_ems_msg::transport::InMemoryTransport;
use r_ems_msg::types::{MessagePayload, TelemetryFrame, TelemetryValues};
use r_ems_msg::{DeliveryGuarantee, MessagingSupervisor};

fn main() -> anyhow::Result<()> {
    // Node A publishes telemetry via the messaging supervisor.
    let mut supervisor_a = MessagingSupervisor::new(DeliveryGuarantee::AtLeastOnce {
        max_retries: 3,
        retry_interval: Duration::from_millis(100),
    });
    let shared_transport = Arc::new(InMemoryTransport::new());
    supervisor_a.register_transport(shared_transport.clone());

    // Node B consumes telemetry from the same transport.
    let mut supervisor_b = MessagingSupervisor::new(DeliveryGuarantee::AtMostOnce);
    supervisor_b.register_transport(shared_transport.clone());

    let mut values = TelemetryValues::new();
    values.insert("power_kw".into(), 125.4);
    let payload = MessagePayload::Telemetry(TelemetryFrame::new("grid-a", "controller-a", values));
    let sequence = supervisor_a.publish(payload)?;

    // Node B polls for incoming messages.
    let received = supervisor_b.poll();
    println!("Node B received {} message(s)", received.len());
    for message in received.iter() {
        println!("  kind={} id={}", message.kind(), message.id);
    }

    supervisor_a.acknowledge(sequence);
    Ok(())
}
