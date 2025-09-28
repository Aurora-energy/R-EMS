//! ---
//! ems_section: "07-resilience-fault-tolerance"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Redundancy planning and failover coordinators."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::time::{Duration, Instant};

use r_ems_common::config::{ControllerConfig, ControllerRole};

/// Unique identifier and static configuration for a controller instance.
#[derive(Debug, Clone)]
pub struct ControllerContext {
    pub grid_id: String,
    pub controller_id: String,
    pub role: ControllerRole,
    pub failover_order: u32,
    pub heartbeat_interval: Duration,
    pub watchdog_timeout: Duration,
}

impl ControllerContext {
    pub fn from_config(grid_id: &str, controller_id: &str, config: &ControllerConfig) -> Self {
        Self {
            grid_id: grid_id.to_owned(),
            controller_id: controller_id.to_owned(),
            role: config.role.clone(),
            failover_order: config.failover_order,
            heartbeat_interval: config.heartbeat_interval,
            watchdog_timeout: config.watchdog_timeout,
        }
    }
}

/// Controller runtime state maintained by the redundancy supervisor.
#[derive(Debug, Clone)]
pub struct ControllerRuntimeState {
    pub context: ControllerContext,
    last_heartbeat: Option<Instant>,
    pub is_active: bool,
    failure_count: u32,
}

impl ControllerRuntimeState {
    pub fn new(context: ControllerContext) -> Self {
        Self {
            context,
            last_heartbeat: None,
            is_active: false,
            failure_count: 0,
        }
    }

    pub fn record_heartbeat(&mut self, now: Instant) -> HeartbeatStatus {
        let status = match self.last_heartbeat {
            Some(previous) => {
                let delta = now.duration_since(previous);
                if delta <= self.context.heartbeat_interval + Duration::from_millis(50) {
                    HeartbeatStatus::OnTime
                } else {
                    HeartbeatStatus::Late(delta - self.context.heartbeat_interval)
                }
            }
            None => HeartbeatStatus::OnTime,
        };
        self.last_heartbeat = Some(now);
        status
    }

    pub fn evaluate(&mut self, now: Instant) -> HeartbeatStatus {
        match self.last_heartbeat {
            Some(previous) => {
                let delta = now.duration_since(previous);
                if delta > self.context.watchdog_timeout {
                    self.failure_count += 1;
                    HeartbeatStatus::Missing(delta - self.context.watchdog_timeout)
                } else {
                    HeartbeatStatus::OnTime
                }
            }
            None => {
                self.failure_count += 1;
                HeartbeatStatus::Missing(self.context.watchdog_timeout)
            }
        }
    }

    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }
}

/// Result of a heartbeat evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeartbeatStatus {
    OnTime,
    Late(Duration),
    Missing(Duration),
}
