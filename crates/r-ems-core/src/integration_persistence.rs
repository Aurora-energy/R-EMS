//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Primary orchestration and lifecycle management."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use r_ems_persistence::event_log::{EventLogEntry, EventLogWriter};
use r_ems_persistence::metrics::PersistenceMetrics;
use r_ems_persistence::replay_event_log;
use r_ems_persistence::snapshot::{load_snapshot, save_snapshot, ControllerState};
use r_ems_persistence::{PersistenceError, Result as PersistenceResult};
use serde_json::{json, Map, Value};
use tracing::debug;

use crate::state::ControllerSnapshot;

/// Convert an orchestrator snapshot into a persistence `ControllerState`.
pub fn snapshot_to_state(snapshot: &ControllerSnapshot) -> ControllerState {
    ControllerState {
        grid_id: snapshot.grid_id.clone(),
        controller_id: snapshot.controller_id.clone(),
        state: snapshot.payload.clone(),
        captured_at: snapshot.captured_at,
    }
}

/// Convert a persistence `ControllerState` back into an orchestrator snapshot.
pub fn state_to_snapshot(state: ControllerState) -> ControllerSnapshot {
    ControllerSnapshot {
        grid_id: state.grid_id,
        controller_id: state.controller_id,
        captured_at: state.captured_at,
        payload: state.state,
    }
}

/// Persist a controller snapshot to disk using the persistence crate.
pub fn persist_snapshot(snapshot: &ControllerSnapshot, path: &Path) -> PersistenceResult<()> {
    let controller_state = snapshot_to_state(snapshot);
    save_snapshot(&controller_state, path)
}

/// Load a controller snapshot from disk using the persistence crate.
pub fn restore_snapshot(path: &Path) -> PersistenceResult<ControllerSnapshot> {
    load_snapshot(path).map(state_to_snapshot)
}

/// Coordinates access to the event log for a grid.
pub struct PersistenceBridge {
    event_log_path: PathBuf,
    writer: Mutex<EventLogWriter>,
    metrics: Option<Arc<PersistenceMetrics>>,
}

impl PersistenceBridge {
    /// Build a new bridge for the provided event log path.
    pub fn new(
        event_log_path: PathBuf,
        metrics: Option<Arc<PersistenceMetrics>>,
    ) -> PersistenceResult<Self> {
        let writer = EventLogWriter::open(&event_log_path)?;
        Ok(Self {
            event_log_path,
            writer: Mutex::new(writer),
            metrics,
        })
    }

    /// Construct a bridge for a grid based on the grid snapshot root path.
    pub fn for_grid(
        grid_root: &Path,
        metrics: Option<Arc<PersistenceMetrics>>,
    ) -> PersistenceResult<Self> {
        let mut log_path = PathBuf::from(grid_root);
        log_path.push("events.log");
        Self::new(log_path, metrics)
    }

    /// Append a controller tick event to the event log.
    pub fn record_controller_tick(
        &self,
        grid_id: &str,
        controller_id: &str,
        tick: u64,
        mode: &str,
        active: bool,
        telemetry: &Value,
    ) -> PersistenceResult<u64> {
        self.record_event(
            grid_id,
            controller_id,
            "controller_tick",
            json!({
                "tick": tick,
                "mode": mode,
                "active": active,
                "telemetry": telemetry,
            }),
        )
    }

    /// Record that a snapshot was persisted at the provided path.
    pub fn record_snapshot_saved(
        &self,
        grid_id: &str,
        controller_id: &str,
        path: &Path,
    ) -> PersistenceResult<u64> {
        if let Some(metrics) = &self.metrics {
            metrics.record_snapshot_saved(grid_id, controller_id);
        }
        self.record_event(
            grid_id,
            controller_id,
            "snapshot_saved",
            json!({
                "grid_id": grid_id,
                "controller_id": controller_id,
                "path": path.display().to_string(),
            }),
        )
    }

    /// Record that a snapshot was restored for a controller.
    pub fn record_snapshot_restored(
        &self,
        grid_id: &str,
        controller_id: &str,
        path: &Path,
    ) -> PersistenceResult<u64> {
        self.record_event(
            grid_id,
            controller_id,
            "snapshot_restored",
            json!({
                "grid_id": grid_id,
                "controller_id": controller_id,
                "path": path.display().to_string(),
            }),
        )
    }

    /// Record metrics for a snapshot failure without writing to the event log.
    pub fn record_snapshot_failure(&self, grid_id: &str, controller_id: &str) {
        if let Some(metrics) = &self.metrics {
            metrics.record_snapshot_failed(grid_id, controller_id);
        }
    }

    /// Record a failover promotion.
    pub fn record_failover(
        &self,
        grid_id: &str,
        controller_id: &str,
        reason: &str,
    ) -> PersistenceResult<u64> {
        self.record_event(
            grid_id,
            controller_id,
            "failover",
            json!({
                "grid_id": grid_id,
                "controller_id": controller_id,
                "reason": reason,
            }),
        )
    }

    /// Flush pending writes to disk.
    pub fn flush(&self) -> PersistenceResult<()> {
        let mut guard = self.writer.lock();
        guard.flush()
    }

    /// Replay the full event log, invoking the supplied handler for each entry.
    pub fn replay<F>(&self, handler: F) -> PersistenceResult<usize>
    where
        F: FnMut(EventLogEntry) -> PersistenceResult<()>,
    {
        self.flush()?;
        replay_event_log(&self.event_log_path, handler)
    }

    /// Replay events for a specific controller.
    pub fn replay_for_controller<F>(
        &self,
        grid_id: &str,
        controller_id: &str,
        mut handler: F,
    ) -> PersistenceResult<usize>
    where
        F: FnMut(EventLogEntry) -> PersistenceResult<()>,
    {
        let started = Instant::now();
        let result = self.replay(|entry| {
            let matches_grid = entry
                .payload
                .get("grid_id")
                .and_then(Value::as_str)
                .map(|value| value == grid_id)
                .unwrap_or(false);
            let matches_controller = entry
                .payload
                .get("controller_id")
                .and_then(Value::as_str)
                .map(|value| value == controller_id)
                .unwrap_or(false);
            if matches_grid && matches_controller {
                handler(entry)?;
            }
            Ok(())
        });
        let elapsed = started.elapsed().as_secs_f64();
        if let Some(metrics) = &self.metrics {
            metrics.observe_replay_duration(grid_id, controller_id, elapsed);
        }
        result
    }

    /// Append a raw event payload to the log.
    pub fn record_event(
        &self,
        grid_id: &str,
        controller_id: &str,
        kind: &str,
        payload: Value,
    ) -> PersistenceResult<u64> {
        let mut entry_payload = ensure_object(payload);
        entry_payload.insert("kind".into(), Value::String(kind.to_owned()));
        let mut writer = self.writer.lock();
        let event = EventLogEntry::new(Value::Object(entry_payload));
        let (sequence, bytes) = writer.append(event)?;
        if let Some(metrics) = &self.metrics {
            metrics.record_event_bytes(grid_id, controller_id, bytes);
        }
        Ok(sequence)
    }

    /// Expose the underlying event log path (primarily for diagnostics/tests).
    pub fn event_log_path(&self) -> &Path {
        &self.event_log_path
    }
}

fn ensure_object(payload: Value) -> Map<String, Value> {
    match payload {
        Value::Object(map) => map,
        other => {
            let mut map = Map::new();
            map.insert("data".into(), other);
            map
        }
    }
}

/// Simple helper invoked during replay to emit debug traces.
pub fn log_replayed_entry(entry: &EventLogEntry) {
    debug!(sequence = entry.sequence, payload = %entry.payload, "replayed persistence event");
}

/// Error alias re-exported for convenience.
pub type Error = PersistenceError;
