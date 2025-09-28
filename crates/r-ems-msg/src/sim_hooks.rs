//! ---
//! ems_section: "02-messaging-ipc-data-model"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Message schema helpers and protocol codecs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use crate::types::{MessagePayload, TelemetryFrame, TelemetryValues};
use crate::{MessagingSupervisor, Result};

/// Publish a telemetry frame on behalf of a simulated controller.
pub fn publish_simulated_frame(
    supervisor: &MessagingSupervisor,
    grid_id: impl Into<String>,
    controller_id: impl Into<String>,
    values: TelemetryValues,
) -> Result<u64> {
    let frame = TelemetryFrame::new(grid_id, controller_id, values);
    supervisor.publish(MessagePayload::Telemetry(frame))
}

#[derive(Debug, Deserialize)]
struct ReplayRecord {
    #[serde(default)]
    delay_ms: Option<u64>,
    payload: MessagePayload,
}

/// Replay messages from a newline-delimited JSON file.
///
/// Each line must contain an object with a `payload` field containing a
/// `MessagePayload` (e.g., telemetry frame) and an optional `delay_ms` field
/// to simulate time between messages.
pub fn replay_from_file<P: AsRef<Path>>(
    supervisor: &MessagingSupervisor,
    path: P,
) -> Result<usize> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut count = 0usize;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: ReplayRecord = serde_json::from_str(&line)?;
        if let Some(delay) = record.delay_ms {
            std::thread::sleep(Duration::from_millis(delay));
        }
        supervisor.publish(record.payload)?;
        count += 1;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::InMemoryTransport;
    use crate::{DeliveryGuarantee, Transport};
    use std::sync::Arc;

    #[test]
    fn publish_simulated_frame_enqueues_messages() {
        let mut supervisor = MessagingSupervisor::new(DeliveryGuarantee::AtMostOnce);
        let transport = Arc::new(InMemoryTransport::new());
        supervisor.register_transport(transport.clone());

        let mut values = TelemetryValues::new();
        values.insert("soc".into(), 0.75);
        publish_simulated_frame(&supervisor, "grid-a", "controller-b", values)
            .expect("publish succeeds");
        assert!(transport.recv().is_some());
    }

    #[test]
    fn replay_from_file_streams_records() {
        let mut supervisor = MessagingSupervisor::new(DeliveryGuarantee::AtMostOnce);
        let transport = Arc::new(InMemoryTransport::new());
        supervisor.register_transport(transport.clone());

        let temp = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(
            temp.path(),
            r#"{"payload":{"kind":"system","data":{"id":"00000000-0000-0000-0000-000000000000","timestamp":"2024-01-01T00:00:00Z","event_type":"custom","payload":{}}}}
{"delay_ms":1,"payload":{"kind":"telemetry","data":{"grid_id":"g","controller_id":"c","values":{},"timestamp":"2024-01-01T00:00:00Z"}}}
"#,
        )
        .expect("write temp file");

        let replayed = replay_from_file(&supervisor, temp.path()).expect("replay works");
        assert_eq!(replayed, 2);
        assert!(transport.recv().is_some());
    }
}
