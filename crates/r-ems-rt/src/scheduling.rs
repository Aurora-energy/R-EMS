//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Runtime helpers supporting the orchestrator."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::time::Duration;

use anyhow::Result;
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior};

/// Simple async rate limiter that ensures deterministic loop intervals.
#[derive(Debug)]
pub struct RateLimiter {
    interval: tokio::time::Interval,
}

impl RateLimiter {
    pub fn new(period: Duration) -> Self {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        Self { interval }
    }

    pub async fn tick(&mut self) -> Instant {
        self.interval.tick().await
    }
}

/// Deterministic executor that tracks spawned controller tasks for audits.
#[derive(Debug, Default)]
pub struct DeterministicExecutor {
    tasks: Vec<JoinHandle<Result<()>>>,
}

impl DeterministicExecutor {
    pub fn spawn<F>(&mut self, fut: F)
    where
        F: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let handle = tokio::spawn(fut);
        self.tasks.push(handle);
    }

    pub async fn join(self) -> Result<()> {
        for task in self.tasks {
            task.await
                .map_err(|err| anyhow::anyhow!("task join failure: {}", err))??;
        }
        Ok(())
    }
}
