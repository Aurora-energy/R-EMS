//! ---
//! ems_section: "11-simulation-test-harness"
//! ems_subsection: "example"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Example demonstrating replaying event logs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
/*! ---
ems_section: "03-persistence"
ems_subsection: "08-examples"
ems_type: "code"
ems_scope: "core"
ems_description: "Shows how to append controller events and replay them in order."
ems_version: "v0.1.0"
ems_owner: "tbd"
reference: docs/VERSIONING.md
--- */

use anyhow::Result;
use r_ems_persistence::event_log::{EventLogEntry, EventLogWriter};
use r_ems_persistence::replay_event_log;
use serde_json::json;
use tempfile::tempdir;

fn main() -> Result<()> {
    let dir = tempdir()?;
    let log_path = dir.path().join("events.log");
    let mut writer = EventLogWriter::open(&log_path)?;

    let (sequence, bytes) = writer.append(EventLogEntry::new(json!({
        "grid_id": "grid-a",
        "controller_id": "ctrl-1",
        "kind": "controller_tick",
        "tick": 1,
        "active": true
    })))?;
    println!("wrote event #{sequence} ({bytes} bytes)");

    writer.append(EventLogEntry::new(json!({
        "grid_id": "grid-a",
        "controller_id": "ctrl-1",
        "kind": "controller_tick",
        "tick": 2,
        "active": true
    })))?;

    replay_event_log(&log_path, |entry| {
        println!("replayed #{:>02} {:?}", entry.sequence, entry.payload);
        Ok(())
    })?;

    Ok(())
}
