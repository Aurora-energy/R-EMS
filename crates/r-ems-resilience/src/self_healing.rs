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
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::metrics::ResilienceMetrics;

/// Policy parameters controlling restart attempts and scheduling.
#[derive(Debug, Clone, Copy)]
pub struct RestartPolicy {
    /// Maximum number of restart attempts before declaring failure.
    pub max_attempts: usize,
    /// Base delay applied before the second attempt (exponential backoff).
    pub base_delay: Duration,
    /// Maximum jitter added to each delay to avoid thundering herds.
    pub jitter: Duration,
}

impl RestartPolicy {
    /// Construct a conservative policy suitable for production defaults.
    pub fn new(max_attempts: usize, base_delay: Duration, jitter: Duration) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            base_delay,
            jitter,
        }
    }

    /// Calculate the delay for the provided attempt (1-indexed) with exponential growth.
    fn backoff_delay(&self, attempt: usize, rng: &mut StdRng) -> Duration {
        let exponent = (attempt.saturating_sub(1) as u32).min(8);
        let base = self.base_delay.mul_f64(2u32.pow(exponent) as f64);
        if self.jitter.is_zero() {
            base
        } else {
            let jitter_ms = rng.gen_range(0..=self.jitter.as_millis().max(1)) as u64;
            base + Duration::from_millis(jitter_ms)
        }
    }
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self::new(5, Duration::from_millis(250), Duration::from_millis(50))
    }
}

/// Outcome produced after running the self-healing routine.
#[derive(Debug, Clone)]
pub struct SelfHealingOutcome {
    /// Target controller that was being recovered.
    pub controller: String,
    /// Number of attempts executed.
    pub attempts: usize,
    /// Whether a restart ultimately succeeded.
    pub success: bool,
    /// Optional fallback controller chosen for workload reallocation.
    pub reallocated_to: Option<String>,
}

impl SelfHealingOutcome {
    /// Construct a success outcome helper.
    pub fn success(controller: impl Into<String>, attempts: usize) -> Self {
        Self {
            controller: controller.into(),
            attempts,
            success: true,
            reallocated_to: None,
        }
    }

    /// Construct a failure outcome helper with an optional reallocation target.
    pub fn failure(
        controller: impl Into<String>,
        attempts: usize,
        reallocated_to: Option<String>,
    ) -> Self {
        Self {
            controller: controller.into(),
            attempts,
            success: false,
            reallocated_to,
        }
    }
}

/// Supervisor responsible for driving restart attempts and reporting metrics.
#[derive(Debug)]
pub struct SelfHealingManager {
    policy: RestartPolicy,
    metrics: Option<ResilienceMetrics>,
    rng: StdRng,
}

impl SelfHealingManager {
    /// Create a new manager with the provided policy and optional metrics handle.
    pub fn new(policy: RestartPolicy, metrics: Option<ResilienceMetrics>) -> Self {
        let seed = 0xDEADBEEF_u64;
        let rng = StdRng::seed_from_u64(seed);
        Self {
            policy,
            metrics,
            rng,
        }
    }

    /// Seed the internal RNG for deterministic testing.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.rng = StdRng::seed_from_u64(seed);
        self
    }

    /// Attempt to restart a controller. The provided closure should perform one restart attempt.
    pub async fn attempt_recovery<F, Fut>(
        &mut self,
        controller: &str,
        mut operation: F,
        reallocation_candidates: &[String],
    ) -> Result<SelfHealingOutcome>
    where
        F: FnMut(usize) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        for attempt in 1..=self.policy.max_attempts {
            info!(
                target: "r_ems::resilience::self_healing",
                controller,
                attempt,
                "attempting controller restart",
            );
            match operation(attempt).await {
                Ok(()) => {
                    let outcome = SelfHealingOutcome::success(controller.to_owned(), attempt);
                    if let Some(metrics) = &self.metrics {
                        metrics.record_self_heal(controller, &outcome);
                    }
                    info!(controller, attempt, "controller restart succeeded");
                    return Ok(outcome);
                }
                Err(err) => {
                    warn!(
                        controller,
                        attempt,
                        error = %err,
                        "controller restart attempt failed",
                    );
                    if attempt == self.policy.max_attempts {
                        break;
                    }
                    let delay = self.policy.backoff_delay(attempt, &mut self.rng);
                    sleep(delay).await;
                }
            }
        }

        let reallocated = self.select_reallocation(reallocation_candidates);
        if let Some(target) = &reallocated {
            error!(
                controller,
                new_primary = %target,
                "exhausted restart attempts; reallocating workload",
            );
        } else {
            error!(
                controller,
                "exhausted restart attempts with no reallocation target"
            );
        }

        let outcome = SelfHealingOutcome::failure(
            controller.to_owned(),
            self.policy.max_attempts,
            reallocated,
        );
        if let Some(metrics) = &self.metrics {
            metrics.record_self_heal(controller, &outcome);
        }
        Ok(outcome)
    }

    fn select_reallocation(&mut self, candidates: &[String]) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }
        let index = self.rng.gen_range(0..candidates.len());
        candidates.get(index).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn self_healing_eventually_succeeds() {
        let policy = RestartPolicy::new(3, Duration::from_millis(1), Duration::from_millis(1));
        let mut manager = SelfHealingManager::new(policy, None).with_seed(1234);
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let attempts_clone = attempts.clone();
        let outcome = manager
            .attempt_recovery(
                "ctrl-a",
                move |attempt| {
                    let attempts_inner = attempts_clone.clone();
                    async move {
                        *attempts_inner.lock().unwrap() = attempt;
                        if attempt < 2 {
                            Err(anyhow::anyhow!("boom"))
                        } else {
                            Ok(())
                        }
                    }
                },
                &[],
            )
            .await
            .unwrap();
        assert!(outcome.success);
        assert_eq!(outcome.attempts, 2);
        assert_eq!(*attempts.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn self_healing_escalates_to_reallocation() {
        let policy = RestartPolicy::new(2, Duration::from_millis(1), Duration::from_millis(1));
        let mut manager = SelfHealingManager::new(policy, None).with_seed(42);
        let outcome = manager
            .attempt_recovery(
                "ctrl-b",
                |_| async move { Err(anyhow::anyhow!("failure")) },
                &["ctrl-c".into(), "ctrl-d".into()],
            )
            .await
            .unwrap();
        assert!(!outcome.success);
        assert_eq!(outcome.attempts, 2);
        assert!(outcome.reallocated_to.is_some());
    }
}
