//! ---
//! ems_section: "07-resilience-fault-tolerance"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Resilience strategies and chaos tooling."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use crate::metrics::ResilienceMetrics;
use std::fmt;

/// Enumerates the possible degradation levels exposed to operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradationLevel {
    /// All expected controllers are online and the system operates normally.
    Healthy,
    /// Some redundancy has been lost; the system is running in a constrained mode.
    Degraded,
    /// Critical loss of redundancy – only a minimal subset of controllers are available.
    Critical,
}

impl DegradationLevel {
    /// Represent the level as a static label for metrics and status payloads.
    pub fn as_str(&self) -> &'static str {
        match self {
            DegradationLevel::Healthy => "healthy",
            DegradationLevel::Degraded => "degraded",
            DegradationLevel::Critical => "critical",
        }
    }
}

impl fmt::Display for DegradationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Threshold configuration controlling when degradation levels change.
#[derive(Debug, Clone, Copy)]
pub struct DegradationPolicy {
    /// Minimum number of active controllers required to remain fully healthy.
    pub degraded_threshold: usize,
    /// Minimum number of active controllers required to avoid entering the critical state.
    pub critical_threshold: usize,
}

impl DegradationPolicy {
    /// Construct a new policy with the provided thresholds.
    pub fn new(degraded_threshold: usize, critical_threshold: usize) -> Self {
        Self {
            degraded_threshold,
            critical_threshold,
        }
    }

    /// Determine the degradation level for the provided counts.
    pub fn determine(&self, active_controllers: usize) -> DegradationLevel {
        if active_controllers >= self.degraded_threshold {
            DegradationLevel::Healthy
        } else if active_controllers >= self.critical_threshold {
            DegradationLevel::Degraded
        } else {
            DegradationLevel::Critical
        }
    }
}

impl Default for DegradationPolicy {
    fn default() -> Self {
        Self {
            degraded_threshold: 2,
            critical_threshold: 1,
        }
    }
}

/// Snapshot summarising the current degradation level and controller fleet size.
#[derive(Debug, Clone)]
pub struct DegradationState {
    /// Reported degradation level.
    pub level: DegradationLevel,
    /// Number of controllers that are currently active.
    pub active_controllers: usize,
    /// Total number of controllers expected for the grid or installation.
    pub total_controllers: usize,
}

impl DegradationState {
    /// Express the state in a lightweight JSON-style payload for APIs.
    pub fn as_status_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "level": self.level.as_str(),
            "active_controllers": self.active_controllers,
            "total_controllers": self.total_controllers,
        })
    }
}

/// Tracks degradation level transitions and reports them via metrics.
#[derive(Debug)]
pub struct DegradationTracker {
    policy: DegradationPolicy,
    metrics: Option<ResilienceMetrics>,
    last_level: Option<DegradationLevel>,
    total_controllers: usize,
}

impl DegradationTracker {
    /// Create a new tracker for the given policy and controller fleet size.
    pub fn new(
        policy: DegradationPolicy,
        total_controllers: usize,
        metrics: Option<ResilienceMetrics>,
    ) -> Self {
        Self {
            policy,
            metrics,
            last_level: None,
            total_controllers,
        }
    }

    /// Update the tracker with the latest active controller count.
    pub fn evaluate(&mut self, active_controllers: usize) -> DegradationState {
        let level = self.policy.determine(active_controllers);
        if self.last_level.map(|prev| prev != level).unwrap_or(true) {
            if let Some(metrics) = &self.metrics {
                metrics.record_degradation(level);
            }
            tracing::info!(
                target: "r_ems::resilience::degradation",
                level = %level,
                active_controllers,
                total_controllers = self.total_controllers,
                "degradation level transition",
            );
            self.last_level = Some(level);
        }
        DegradationState {
            level,
            active_controllers,
            total_controllers: self.total_controllers,
        }
    }

    /// Return the most recently computed level, if any.
    pub fn current_level(&self) -> Option<DegradationLevel> {
        self.last_level
    }

    /// Change the expected total controller count (e.g. dynamic scaling scenarios).
    pub fn set_total_controllers(&mut self, total: usize) {
        self.total_controllers = total;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_escalates_and_recovers() {
        let mut tracker = DegradationTracker::new(DegradationPolicy::default(), 4, None);
        let healthy = tracker.evaluate(4);
        assert_eq!(healthy.level, DegradationLevel::Healthy);
        let degraded = tracker.evaluate(1);
        assert_eq!(degraded.level, DegradationLevel::Degraded);
        let critical = tracker.evaluate(0);
        assert_eq!(critical.level, DegradationLevel::Critical);
        let recovered = tracker.evaluate(4);
        assert_eq!(recovered.level, DegradationLevel::Healthy);
    }
}
