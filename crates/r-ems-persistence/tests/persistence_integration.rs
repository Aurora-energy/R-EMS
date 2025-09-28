//! ---
//! ems_section: "03-persistence-logging"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Persistence abstractions and storage bindings."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::sync::Arc;

use prometheus::Registry;
use r_ems_persistence::event_log::{EventLogEntry, EventLogWriter};
use r_ems_persistence::metrics::PersistenceMetrics;
use r_ems_persistence::replay_event_log;
use r_ems_persistence::snapshot::{load_snapshot, save_snapshot, verify_snapshot, ControllerState};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn snapshot_roundtrip_succeeds() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("snapshot.json");
    let state = ControllerState::new(
        "grid-x",
        "ctrl-a",
        json!({
            "active": true,
            "tick": 42u64,
        }),
    );

    save_snapshot(&state, &path).unwrap();
    assert!(verify_snapshot(&path));

    let restored = load_snapshot(&path).unwrap();
    assert_eq!(restored.grid_id, state.grid_id);
    assert_eq!(restored.controller_id, state.controller_id);
    assert_eq!(restored.state, state.state);
}

#[test]
fn event_log_replay_orders_entries() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.log");
    let mut writer = EventLogWriter::open(&path).unwrap();

    let (_, bytes_a) = writer
        .append(EventLogEntry::new(json!({
            "sequence": "a",
            "value": 1
        })))
        .unwrap();
    assert!(bytes_a > 0);

    let (_, bytes_b) = writer
        .append(EventLogEntry::new(json!({
            "sequence": "b",
            "value": 2
        })))
        .unwrap();
    assert!(bytes_b > 0);

    let mut replayed = Vec::new();
    replay_event_log(&path, |entry| {
        replayed.push(entry.payload.clone());
        Ok(())
    })
    .unwrap();

    assert_eq!(replayed.len(), 2);
    assert_eq!(replayed[0]["sequence"], json!("a"));
    assert_eq!(replayed[1]["sequence"], json!("b"));
}

#[test]
fn persistence_metrics_capture_activity() {
    let registry = Arc::new(Registry::new());
    let metrics = PersistenceMetrics::new(registry.clone()).unwrap();

    metrics.record_snapshot_saved("grid-1", "ctrl-1");
    metrics.record_snapshot_failed("grid-1", "ctrl-1");
    metrics.record_event_bytes("grid-1", "ctrl-1", 128);
    metrics.observe_replay_duration("grid-1", "ctrl-1", 0.25);

    let families = registry.gather();
    assert_eq!(metric_total(&families, "r_ems_snapshots_saved_total"), 1.0);
    assert_eq!(metric_total(&families, "r_ems_snapshots_failed_total"), 1.0);
    assert_eq!(
        metric_total(&families, "r_ems_event_log_bytes_total"),
        128.0
    );
    assert!(metric_histogram_count(&families, "r_ems_replay_duration_seconds") >= 1.0);
}

fn metric_total(families: &[prometheus::proto::MetricFamily], name: &str) -> f64 {
    families
        .iter()
        .find(|family| family.get_name() == name)
        .and_then(|family| family.get_metric().first())
        .map(|metric| metric.get_counter().get_value())
        .unwrap_or_default()
}

fn metric_histogram_count(families: &[prometheus::proto::MetricFamily], name: &str) -> f64 {
    families
        .iter()
        .find(|family| family.get_name() == name)
        .and_then(|family| family.get_metric().first())
        .map(|metric| metric.get_histogram().get_sample_count() as f64)
        .unwrap_or_default()
}
