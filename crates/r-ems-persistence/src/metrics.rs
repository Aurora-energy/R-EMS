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

use prometheus::{self, CounterVec, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry};

use crate::Result;

/// Metrics published by the persistence subsystem.
#[derive(Clone)]
pub struct PersistenceMetrics {
    snapshots_saved: IntCounterVec,
    snapshots_failed: IntCounterVec,
    event_log_bytes: CounterVec,
    replay_duration: HistogramVec,
    #[allow(dead_code)]
    registry: Arc<Registry>,
}

impl PersistenceMetrics {
    /// Register all persistence metrics with the provided registry.
    pub fn new(registry: Arc<Registry>) -> Result<Self> {
        let snapshots_saved = IntCounterVec::new(
            Opts::new(
                "r_ems_snapshots_saved_total",
                "Total number of controller snapshots successfully persisted",
            ),
            &["grid_id", "controller_id"],
        )?;
        registry.register(Box::new(snapshots_saved.clone()))?;

        let snapshots_failed = IntCounterVec::new(
            Opts::new(
                "r_ems_snapshots_failed_total",
                "Total number of controller snapshot persist operations that failed",
            ),
            &["grid_id", "controller_id"],
        )?;
        registry.register(Box::new(snapshots_failed.clone()))?;

        let event_log_bytes = CounterVec::new(
            Opts::new(
                "r_ems_event_log_bytes_total",
                "Total bytes appended to persistence event logs",
            ),
            &["grid_id", "controller_id"],
        )?;
        registry.register(Box::new(event_log_bytes.clone()))?;

        let histogram_opts = HistogramOpts::new(
            "r_ems_replay_duration_seconds",
            "Duration spent replaying persistence logs for a controller",
        )
        .buckets(prometheus::exponential_buckets(0.001, 2.0, 12)?);
        let replay_duration = HistogramVec::new(histogram_opts, &["grid_id", "controller_id"])?;
        registry.register(Box::new(replay_duration.clone()))?;

        Ok(Self {
            snapshots_saved,
            snapshots_failed,
            event_log_bytes,
            replay_duration,
            registry,
        })
    }

    /// Record a successful snapshot for the provided controller.
    pub fn record_snapshot_saved(&self, grid_id: &str, controller_id: &str) {
        self.snapshots_saved
            .with_label_values(&[grid_id, controller_id])
            .inc();
    }

    /// Record a failed snapshot persist attempt.
    pub fn record_snapshot_failed(&self, grid_id: &str, controller_id: &str) {
        self.snapshots_failed
            .with_label_values(&[grid_id, controller_id])
            .inc();
    }

    /// Add to the total number of bytes written to the event log.
    pub fn record_event_bytes(&self, grid_id: &str, controller_id: &str, bytes: usize) {
        self.event_log_bytes
            .with_label_values(&[grid_id, controller_id])
            .inc_by(bytes as f64);
    }

    /// Observe the duration spent replaying controller events.
    pub fn observe_replay_duration(&self, grid_id: &str, controller_id: &str, seconds: f64) {
        self.replay_duration
            .with_label_values(&[grid_id, controller_id])
            .observe(seconds);
    }
}

impl std::fmt::Debug for PersistenceMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistenceMetrics").finish_non_exhaustive()
    }
}
