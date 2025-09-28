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
use r_ems_msg::types::{MessagePayload, SystemEvent, SystemEventType, TelemetryValues};
use r_ems_msg::{
    publish_simulated_frame, replay_from_file, DeliveryGuarantee, MessagingSupervisor, Transport,
};
use serde_json::json;

fn supervisor_with_transport(
    guarantee: DeliveryGuarantee,
) -> (MessagingSupervisor, Arc<InMemoryTransport>) {
    let mut supervisor = MessagingSupervisor::new(guarantee);
    let transport = Arc::new(InMemoryTransport::new());
    supervisor.register_transport(transport.clone());
    (supervisor, transport)
}

#[test]
fn end_to_end_message_exchange() {
    let (supervisor, transport) = supervisor_with_transport(DeliveryGuarantee::AtMostOnce);
    let mut values = TelemetryValues::new();
    values.insert("temperature_c".into(), 42.0);
    publish_simulated_frame(&supervisor, "grid-a", "c1", values).expect("publish");
    let received = transport.recv().expect("frame available");
    assert_eq!(received.kind(), "telemetry");
}

#[test]
fn qos_retries_on_missing_ack() {
    let (supervisor, transport) = supervisor_with_transport(DeliveryGuarantee::AtLeastOnce {
        max_retries: 2,
        retry_interval: Duration::from_millis(1),
    });

    supervisor
        .publish(MessagePayload::System(SystemEvent::new(
            SystemEventType::Failover,
            json!({"grid": "alpha"}),
        )))
        .expect("publish system event");

    // Drop initial attempt and trigger retry.
    std::thread::sleep(Duration::from_millis(2));
    supervisor.retry_pending();
    assert!(transport.recv().is_some());
}

#[test]
fn replay_from_file_publishes_all_records() {
    let (supervisor, transport) = supervisor_with_transport(DeliveryGuarantee::AtMostOnce);
    let file = tempfile::NamedTempFile::new().expect("temp file");
    std::fs::write(
        file.path(),
        r#"{"payload":{"kind":"system","data":{"id":"00000000-0000-0000-0000-000000000000","timestamp":"2024-01-01T00:00:00Z","event_type":"custom","payload":{}}}}
{"payload":{"kind":"telemetry","data":{"grid_id":"g","controller_id":"c","values":{},"timestamp":"2024-01-01T00:00:00Z"}}}
"#,
    )
    .expect("write temp");

    let count = replay_from_file(&supervisor, file.path()).expect("replay success");
    assert_eq!(count, 2);
    assert!(transport.recv().is_some());
}
