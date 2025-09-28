//! ---
//! ems_section: "07-resilience-fault-tolerance"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Resilience strategies and chaos tooling."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Deserialize;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::metrics::ResilienceMetrics;

/// Declarative chaos scenario loaded from TOML configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ChaosScenario {
    /// Optional seed to guarantee deterministic replay of jitter.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Global jitter applied to the delay of each action (milliseconds).
    #[serde(default)]
    pub jitter_ms: Option<u64>,
    /// Ordered chaos actions executed by the engine.
    #[serde(default)]
    pub actions: Vec<ChaosAction>,
}

impl ChaosScenario {
    /// Load a scenario from disk.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let contents = fs::read_to_string(path.as_ref())
            .with_context(|| format!("unable to read chaos config {}", path.as_ref().display()))?;
        contents.parse::<Self>()
    }

    fn jitter_duration(&self, rng: &mut StdRng) -> Duration {
        let Some(jitter_ms) = self.jitter_ms else {
            return Duration::ZERO;
        };
        if jitter_ms == 0 {
            return Duration::ZERO;
        }
        let jitter = rng.gen_range(0..=jitter_ms);
        Duration::from_millis(jitter)
    }
}

impl std::str::FromStr for ChaosScenario {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        toml::from_str::<Self>(input).map_err(anyhow::Error::new)
    }
}

/// Supported chaos actions.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ChaosAction {
    /// Simulate the abrupt termination of a controller.
    #[serde(rename = "kill_controller")]
    KillController {
        /// Grid identifier targeted by the chaos action.
        grid: String,
        /// Controller identifier which will be terminated.
        #[serde(deserialize_with = "deserialize_string")]
        controller: String,
        /// Seconds to wait before executing the action.
        #[serde(default)]
        delay_sec: u64,
    },
    /// Introduce a network partition for a grid.
    #[serde(rename = "network_partition")]
    NetworkPartition {
        /// Grid affected by the partition.
        grid: String,
        /// Duration the partition remains active (seconds).
        #[serde(default)]
        duration_sec: u64,
        /// Delay before injecting the partition (seconds).
        #[serde(default)]
        delay_sec: u64,
    },
    /// Drop a percentage of inter-controller messages.
    #[serde(rename = "drop_messages")]
    DropMessages {
        /// Grid experiencing message drops.
        grid: String,
        /// Percentage of traffic to drop during the window (0-100).
        percentage: f64,
        /// Duration of the drop window in seconds.
        #[serde(default)]
        duration_sec: u64,
        /// Delay before the drop begins in seconds.
        #[serde(default)]
        delay_sec: u64,
    },
    /// Corrupt the most recent snapshot for a controller.
    #[serde(rename = "corrupt_snapshot")]
    CorruptSnapshot {
        /// Grid owning the snapshot to corrupt.
        grid: String,
        /// Controller identifier associated with the snapshot.
        #[serde(deserialize_with = "deserialize_string")]
        controller: String,
        /// Delay before corruption occurs in seconds.
        #[serde(default)]
        delay_sec: u64,
    },
}

impl ChaosAction {
    fn label(&self) -> &'static str {
        match self {
            ChaosAction::KillController { .. } => "kill_controller",
            ChaosAction::NetworkPartition { .. } => "network_partition",
            ChaosAction::DropMessages { .. } => "drop_messages",
            ChaosAction::CorruptSnapshot { .. } => "corrupt_snapshot",
        }
    }

    fn delay(&self) -> Duration {
        match self {
            ChaosAction::KillController { delay_sec, .. }
            | ChaosAction::NetworkPartition { delay_sec, .. }
            | ChaosAction::DropMessages { delay_sec, .. }
            | ChaosAction::CorruptSnapshot { delay_sec, .. } => Duration::from_secs(*delay_sec),
        }
    }

    fn grid(&self) -> &str {
        match self {
            ChaosAction::KillController { grid, .. }
            | ChaosAction::NetworkPartition { grid, .. }
            | ChaosAction::DropMessages { grid, .. }
            | ChaosAction::CorruptSnapshot { grid, .. } => grid,
        }
    }

    fn controller(&self) -> Option<&str> {
        match self {
            ChaosAction::KillController { controller, .. }
            | ChaosAction::CorruptSnapshot { controller, .. } => Some(controller.as_str()),
            ChaosAction::NetworkPartition { .. } | ChaosAction::DropMessages { .. } => None,
        }
    }

    fn parameters(&self) -> serde_json::Value {
        match self {
            ChaosAction::KillController { controller, .. } => serde_json::json!({
                "controller": controller,
            }),
            ChaosAction::NetworkPartition { duration_sec, .. } => serde_json::json!({
                "duration_sec": duration_sec,
            }),
            ChaosAction::DropMessages {
                percentage,
                duration_sec,
                ..
            } => serde_json::json!({
                "percentage": percentage,
                "duration_sec": duration_sec,
            }),
            ChaosAction::CorruptSnapshot { controller, .. } => serde_json::json!({
                "controller": controller,
            }),
        }
    }

    fn duration(&self) -> Option<Duration> {
        match self {
            ChaosAction::NetworkPartition { duration_sec, .. }
            | ChaosAction::DropMessages { duration_sec, .. } => {
                Some(Duration::from_secs(*duration_sec))
            }
            _ => None,
        }
    }
}

fn deserialize_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("string or integer")
        }

        fn visit_str<E>(self, value: &str) -> Result<String, E>
        where
            E: serde::de::Error,
        {
            Ok(value.to_owned())
        }

        fn visit_u64<E>(self, value: u64) -> Result<String, E>
        where
            E: serde::de::Error,
        {
            Ok(value.to_string())
        }
    }

    deserializer.deserialize_any(Visitor)
}

/// Execution record returned after running a chaos scenario.
#[derive(Debug, Clone)]
pub struct ChaosEventRecord {
    /// Action label executed.
    pub action: String,
    /// Grid targeted by the action.
    pub grid: String,
    /// Optional controller identifier.
    pub controller: Option<String>,
    /// Delay applied before the action ran.
    pub delay_applied: Duration,
    /// Optional duration associated with the action.
    pub duration: Option<Duration>,
    /// Timestamp when the action was triggered.
    pub executed_at: DateTime<Utc>,
    /// Additional contextual parameters recorded for traceability.
    pub parameters: serde_json::Value,
}

impl ChaosEventRecord {
    fn from_action(
        action: &ChaosAction,
        delay_applied: Duration,
        executed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            action: action.label().to_string(),
            grid: action.grid().to_string(),
            controller: action.controller().map(ToOwned::to_owned),
            delay_applied,
            duration: action.duration(),
            executed_at,
            parameters: action.parameters(),
        }
    }
}

/// Chaos engine responsible for executing scenarios.
#[derive(Debug)]
pub struct ChaosEngine {
    scenario: ChaosScenario,
    metrics: Option<ResilienceMetrics>,
    rng: StdRng,
}

impl ChaosEngine {
    /// Build a new chaos engine from a scenario.
    pub fn new(scenario: ChaosScenario, metrics: Option<ResilienceMetrics>) -> Self {
        let seed = scenario.seed.unwrap_or(0xC0FFEE_u64);
        let rng = StdRng::seed_from_u64(seed);
        Self {
            scenario,
            metrics,
            rng,
        }
    }

    /// Execute all chaos actions sequentially, returning execution records.
    pub async fn execute(&mut self) -> Result<Vec<ChaosEventRecord>> {
        let mut records = Vec::with_capacity(self.scenario.actions.len());
        for action in &self.scenario.actions {
            let jitter = self.scenario.jitter_duration(&mut self.rng);
            let delay = action.delay() + jitter;
            if delay > Duration::ZERO {
                sleep(delay).await;
            }
            let executed_at = Utc::now();
            if let Some(metrics) = &self.metrics {
                metrics.inc_chaos_event(action.label());
            }
            warn!(
                target: "r_ems::resilience::chaos",
                action = action.label(),
                grid = %action.grid(),
                controller = ?action.controller(),
                delay_ms = delay.as_millis(),
                duration_ms = action.duration().map(|d| d.as_millis()),
                params = %action.parameters(),
                "chaos action executed",
            );
            records.push(ChaosEventRecord::from_action(action, delay, executed_at));
        }
        info!(total_actions = records.len(), "completed chaos scenario");
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn engine_runs_actions_in_order() {
        let scenario = r#"
        jitter_ms = 0

        [[actions]]
        type = "kill_controller"
        grid = "grid-a"
        controller = "primary"
        delay_sec = 0

        [[actions]]
        type = "network_partition"
        grid = "grid-a"
        duration_sec = 1
        delay_sec = 0
        "#
        .parse::<ChaosScenario>()
        .unwrap();
        let mut engine = ChaosEngine::new(scenario, None);
        let records = engine.execute().await.unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].action, "kill_controller");
        assert_eq!(records[1].action, "network_partition");
    }
}
