//! ---
//! ems_section: "07-resilience-fault-tolerance"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Resilience strategies and chaos tooling."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use r_ems_common::config::ControllerConfig;
use r_ems_redundancy::{ControllerContext, FailoverEvent, FailoverReason, RedundancySupervisor};
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::metrics::ResilienceMetrics;

/// Configuration describing the controllers that participate in the stress harness.
#[derive(Debug, Clone)]
pub struct FailoverStressConfig {
    /// Logical grid identifier under test.
    pub grid_id: String,
    /// Ordered controller set taking part in the scenario.
    pub controllers: IndexMap<String, ControllerConfig>,
    /// Grace period added on top of a controller's watchdog timeout before evaluation.
    pub evaluation_grace: Duration,
}

impl FailoverStressConfig {
    /// Construct a configuration with the supplied grid id and controller map.
    pub fn new(
        grid_id: impl Into<String>,
        controllers: IndexMap<String, ControllerConfig>,
    ) -> Self {
        Self {
            grid_id: grid_id.into(),
            controllers,
            evaluation_grace: Duration::from_millis(10),
        }
    }

    /// Adjust the evaluation grace period.
    pub fn with_grace(mut self, grace: Duration) -> Self {
        self.evaluation_grace = grace;
        self
    }
}

impl Default for FailoverStressConfig {
    fn default() -> Self {
        Self {
            grid_id: "resilience-grid".into(),
            controllers: IndexMap::new(),
            evaluation_grace: Duration::from_millis(10),
        }
    }
}

/// Result from a single failover simulation.
#[derive(Debug, Clone)]
pub struct FailoverStressReport {
    /// Controller that was intentionally failed.
    pub failed_controller: String,
    /// Controller promoted to become the new primary.
    pub promoted_controller: String,
    /// Reason emitted by the redundancy supervisor.
    pub reason: FailoverReason,
    /// Wall-clock duration between the final heartbeat and promotion.
    pub recovery_duration: Duration,
    /// Full failover event from the redundancy supervisor.
    pub event: FailoverEvent,
}

/// Stateful harness that repeatedly triggers failover sequences and records metrics.
#[derive(Debug)]
pub struct FailoverStressRunner {
    grid_id: String,
    supervisor: RedundancySupervisor,
    controllers: IndexMap<String, ControllerConfig>,
    active_controller: String,
    evaluation_grace: Duration,
    metrics: Option<ResilienceMetrics>,
}

impl FailoverStressRunner {
    /// Instantiate a stress harness from the provided configuration.
    pub fn new(config: FailoverStressConfig, metrics: Option<ResilienceMetrics>) -> Result<Self> {
        if config.controllers.len() < 2 {
            return Err(anyhow!(
                "failover stress testing requires at least two controllers"
            ));
        }
        let supervisor = RedundancySupervisor::new(&config.grid_id);
        let mut active = None;
        for (controller_id, controller_cfg) in &config.controllers {
            let context =
                ControllerContext::from_config(&config.grid_id, controller_id, controller_cfg);
            supervisor.register(context);
            let now = Instant::now();
            supervisor.heartbeat(controller_id, now);
            if active.is_none() {
                active = Some(controller_id.clone());
            }
        }
        let active = active.ok_or_else(|| anyhow!("no controllers were registered"))?;
        Ok(Self {
            grid_id: config.grid_id,
            supervisor,
            controllers: config.controllers,
            active_controller: active,
            evaluation_grace: config.evaluation_grace,
            metrics,
        })
    }

    /// Return the id of the controller currently holding the primary role.
    pub fn active_controller(&self) -> &str {
        &self.active_controller
    }

    /// Trigger a single failover cycle by waiting for the watchdog timeout and running evaluation.
    pub async fn run_iteration(&mut self) -> Result<FailoverStressReport> {
        let failing = self.active_controller.clone();
        let Some(config) = self.controllers.get(&failing) else {
            return Err(anyhow!("unknown controller {}", failing));
        };

        let heartbeat_at = Instant::now();
        let status = self.supervisor.heartbeat(&failing, heartbeat_at);
        debug!(
            grid = %self.grid_id,
            controller = %failing,
            ?status,
            "recorded heartbeat prior to failover trigger"
        );

        let watchdog = config.watchdog_timeout + self.evaluation_grace;
        sleep(watchdog).await;

        let evaluate_at = Instant::now();
        let result = self
            .supervisor
            .evaluate(evaluate_at)
            .ok_or_else(|| anyhow!("no failover event emitted"))?;
        let promoted = result.activated_controller.clone();
        let duration = evaluate_at
            .checked_duration_since(heartbeat_at)
            .unwrap_or_default();

        if promoted == failing {
            warn!(
                grid = %self.grid_id,
                controller = %promoted,
                "failover did not promote a new controller"
            );
        } else {
            info!(
                grid = %self.grid_id,
                failed = %failing,
                promoted = %promoted,
                seconds = duration.as_secs_f64(),
                "failover stress iteration completed"
            );
        }

        if let Some(metrics) = &self.metrics {
            metrics.observe_failover(&self.grid_id, &failing, &promoted, duration);
        }
        self.active_controller = promoted.clone();

        Ok(FailoverStressReport {
            failed_controller: failing,
            promoted_controller: promoted,
            reason: result.reason,
            recovery_duration: duration,
            event: result,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use r_ems_common::config::ControllerRole;
    use r_ems_metrics::new_registry;
    use r_ems_redundancy::FailoverReason;
    use crate::metrics::ResilienceMetrics;

    #[tokio::test]
    async fn runner_promotes_secondary() {
        let mut controllers = IndexMap::new();
        let mut primary = ControllerConfig::default();
        primary.role = ControllerRole::Primary;
        primary.watchdog_timeout = Duration::from_millis(10);
        controllers.insert("primary".into(), primary);
        let mut secondary = ControllerConfig::default();
        secondary.role = ControllerRole::Secondary;
        secondary.failover_order = 1;
        secondary.watchdog_timeout = Duration::from_millis(10);
        controllers.insert("secondary".into(), secondary);

        let config = FailoverStressConfig::new("grid-a", controllers);
        let mut runner = FailoverStressRunner::new(config, None).unwrap();
        assert_eq!(runner.active_controller(), "primary");
        let report = runner.run_iteration().await.unwrap();
        assert_eq!(report.failed_controller, "primary");
        assert_eq!(report.promoted_controller, "secondary");
        assert!(report.recovery_duration >= Duration::from_millis(10));
    }

    #[tokio::test]
    async fn runner_cycles_controllers_and_records_metrics() {
        let mut controllers = IndexMap::new();
        let mut primary = ControllerConfig::default();
        primary.role = ControllerRole::Primary;
        primary.watchdog_timeout = Duration::from_millis(5);
        controllers.insert("primary".into(), primary);

        let mut secondary = ControllerConfig::default();
        secondary.role = ControllerRole::Secondary;
        secondary.failover_order = 1;
        secondary.watchdog_timeout = Duration::from_millis(5);
        controllers.insert("secondary".into(), secondary);

        let registry = new_registry();
        let metrics = ResilienceMetrics::new(registry.clone()).unwrap();

        let config = FailoverStressConfig::new("grid-metrics", controllers)
            .with_grace(Duration::from_millis(1));
        let mut runner = FailoverStressRunner::new(config, Some(metrics.clone())).unwrap();

        let first = runner.run_iteration().await.unwrap();
        assert_eq!(first.failed_controller, "primary");
        assert_eq!(first.promoted_controller, "secondary");
        assert!(matches!(first.reason, FailoverReason::HeartbeatTimeout));

        let second = runner.run_iteration().await.unwrap();
        assert_eq!(second.failed_controller, "secondary");
        assert_eq!(second.promoted_controller, "primary");
        assert!(matches!(second.reason, FailoverReason::HeartbeatTimeout));

        let families = registry.gather();
        let failovers = families
            .iter()
            .find(|fam| fam.get_name() == "r_ems_resilience_failovers_total")
            .expect("failover counter registered");

        let mut observed = std::collections::HashMap::new();
        for metric in failovers.get_metric() {
            let mut failed = None;
            let mut promoted = None;
            for label in metric.get_label() {
                match label.get_name() {
                    "failed" => failed = Some(label.get_value().to_string()),
                    "promoted" => promoted = Some(label.get_value().to_string()),
                    _ => {}
                }
            }
            let key = (failed.expect("failed label"), promoted.expect("promoted label"));
            observed.insert(key, metric.get_counter().get_value());
        }

        assert_eq!(observed.get(&("primary".into(), "secondary".into())), Some(&1.0));
        assert_eq!(observed.get(&("secondary".into(), "primary".into())), Some(&1.0));
    }
}
