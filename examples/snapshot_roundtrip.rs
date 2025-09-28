//! ---
//! ems_section: "03-persistence-logging"
//! ems_subsection: "example"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Example verifying persistence snapshot round-trips."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
/*! ---
ems_section: "03-persistence"
ems_subsection: "08-examples"
ems_type: "code"
ems_scope: "core"
ems_description: "Demonstrates saving and loading a controller snapshot with integrity verification."
ems_version: "v0.1.0"
ems_owner: "tbd"
reference: docs/VERSIONING.md
--- */

use anyhow::Result;
use r_ems_persistence::snapshot::{load_snapshot, save_snapshot, verify_snapshot, ControllerState};
use serde_json::json;
use tempfile::tempdir;

fn main() -> Result<()> {
    let dir = tempdir()?;
    let snapshot_path = dir.path().join("grid-a_ctrl-1.json");

    let state = ControllerState::new(
        "grid-a",
        "ctrl-1",
        json!({
            "voltage": 416.2,
            "current": 12.3,
            "setpoint_kw": 85.0
        }),
    );

    save_snapshot(&state, &snapshot_path)?;
    assert!(verify_snapshot(&snapshot_path));

    let restored = load_snapshot(&snapshot_path)?;
    println!(
        "Restored snapshot for {}:{} at {}",
        restored.grid_id,
        restored.controller_id,
        restored.captured_at
    );
    Ok(())
}
