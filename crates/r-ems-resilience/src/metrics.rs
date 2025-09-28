//! ---
//! ems_section: "07-resilience-fault-tolerance"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Resilience strategies and chaos tooling."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::time::Duration;

use anyhow::Result;
use prometheus::{self, HistogramOpts, HistogramVec, IntCounterVec, Opts};
use r_ems_metrics::SharedRegistry;

use crate::degradation::DegradationLevel;
use crate::self_healing::SelfHealingOutcome;

/// Metrics published by the resilience subsystem.
#[derive(Clone)]
pub struct ResilienceMetrics {
    registry: SharedRegistry,
    failovers_total: IntCounterVec,
    failover_latency_seconds: HistogramVec,
    chaos_events_total: IntCounterVec,
    degradations_total: IntCounterVec,
    self_heal_restarts_total: IntCounterVec,
}

impl ResilienceMetrics {
    /// Register the resilience metric family against the provided registry.
    pub fn new(registry: SharedRegistry) -> Result<Self> {
        let failovers_total = IntCounterVec::new(
            Opts::new(
                "r_ems_resilience_failovers_total",
                "Total number of failover promotions triggered during resilience testing",
            ),
            &["grid_id", "failed", "promoted"],
        )?;
        registry.register(Box::new(failovers_total.clone()))?;

        let histogram_opts = HistogramOpts::new(
            "r_ems_resilience_failover_latency_seconds",
            "Observed duration between controller failure and promotion of the successor",
        )
        .buckets(prometheus::exponential_buckets(0.001, 2.0, 16)?);
        let failover_latency_seconds =
            HistogramVec::new(histogram_opts, &["grid_id", "failed", "promoted"])?;
        registry.register(Box::new(failover_latency_seconds.clone()))?;

        let chaos_events_total = IntCounterVec::new(
            Opts::new(
                "r_ems_resilience_chaos_events_total",
                "Number of chaos injection actions executed",
            ),
            &["action"],
        )?;
        registry.register(Box::new(chaos_events_total.clone()))?;

        let degradations_total = IntCounterVec::new(
            Opts::new(
                "r_ems_resilience_degradations_total",
                "Count of degradation level transitions",
            ),
            &["level"],
        )?;
        registry.register(Box::new(degradations_total.clone()))?;

        let self_heal_restarts_total = IntCounterVec::new(
            Opts::new(
                "r_ems_resilience_self_heal_restarts_total",
                "Attempts made by the self-healing supervisor",
            ),
            &["controller", "outcome"],
        )?;
        registry.register(Box::new(self_heal_restarts_total.clone()))?;

        Ok(Self {
            registry,
            failovers_total,
            failover_latency_seconds,
            chaos_events_total,
            degradations_total,
            self_heal_restarts_total,
        })
    }

    /// Expose the underlying shared registry for convenience.
    pub fn registry(&self) -> SharedRegistry {
        self.registry.clone()
    }

    /// Record a failover duration and increment counters for the involved controllers.
    pub fn observe_failover(
        &self,
        grid_id: &str,
        failed: &str,
        promoted: &str,
        duration: Duration,
    ) {
        let labels = [grid_id, failed, promoted];
        self.failovers_total.with_label_values(&labels).inc();
        self.failover_latency_seconds
            .with_label_values(&labels)
            .observe(duration.as_secs_f64());
    }

    /// Bump the chaos counter for the provided action identifier.
    pub fn inc_chaos_event(&self, action: &str) {
        self.chaos_events_total.with_label_values(&[action]).inc();
    }

    /// Track a transition into a new degradation level.
    pub fn record_degradation(&self, level: DegradationLevel) {
        self.degradations_total
            .with_label_values(&[level.as_str()])
            .inc();
    }

    /// Register a self-healing attempt outcome for observability.
    pub fn record_self_heal(&self, controller: &str, outcome: &SelfHealingOutcome) {
        let label_outcome = if outcome.success {
            "success"
        } else {
            "failure"
        };
        self.self_heal_restarts_total
            .with_label_values(&[controller, label_outcome])
            .inc_by(outcome.attempts as u64);
    }
}

impl std::fmt::Debug for ResilienceMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResilienceMetrics").finish_non_exhaustive()
    }
}
