//! ---
//! ems_section: "07-resilience-fault-tolerance"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Redundancy planning and failover coordinators."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::Instant;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use tracing::{debug, info, warn};

use crate::controller::{ControllerContext, ControllerRuntimeState, HeartbeatStatus};
use r_ems_common::config::ControllerRole;

#[derive(Debug)]
struct SupervisorInner {
    active: Option<String>,
    controllers: HashMap<String, ControllerRuntimeState>,
}

/// Supervises controllers within a grid and mediates failover.
#[derive(Debug)]
pub struct RedundancySupervisor {
    grid_id: String,
    inner: Mutex<SupervisorInner>,
}

impl RedundancySupervisor {
    pub fn new(grid_id: impl Into<String>) -> Self {
        Self {
            grid_id: grid_id.into(),
            inner: Mutex::new(SupervisorInner {
                active: None,
                controllers: HashMap::new(),
            }),
        }
    }

    pub fn register(&self, context: ControllerContext) {
        let mut inner = self.inner.lock();
        let controller_id = context.controller_id.clone();
        let should_activate = match inner.active.as_ref() {
            None => true,
            Some(active_id) if active_id == &controller_id => true,
            Some(active_id) => inner
                .controllers
                .get(active_id)
                .map(|active_state| context_cmp(&context, &active_state.context) == Ordering::Less)
                .unwrap_or(true),
        };

        let context_clone = context.clone();
        inner
            .controllers
            .entry(controller_id.clone())
            .or_insert_with(|| ControllerRuntimeState::new(context_clone));

        if should_activate {
            if let Some(previous) = inner.active.replace(controller_id.clone()) {
                if let Some(prev_state) = inner.controllers.get_mut(&previous) {
                    prev_state.is_active = false;
                }
            }
        }
        if let Some(state) = inner.controllers.get_mut(&controller_id) {
            state.context = context;
            if should_activate {
                state.is_active = true;
            }
        }
        println!(
            "register controller={} active={:?} should_activate={}",
            controller_id, inner.active, should_activate
        );
        debug!(grid = %self.grid_id, controller = %controller_id, "registered controller");
    }

    pub fn heartbeat(&self, controller_id: &str, now: Instant) -> HeartbeatStatus {
        let mut inner = self.inner.lock();
        let Some(state) = inner.controllers.get_mut(controller_id) else {
            warn!(grid = %self.grid_id, controller_id, "received heartbeat for unknown controller");
            return HeartbeatStatus::Missing(Default::default());
        };
        let status = state.record_heartbeat(now);
        if inner.active.as_deref() != Some(controller_id) {
            debug!(grid = %self.grid_id, controller = controller_id, "heartbeat from standby");
        }
        status
    }

    pub fn is_active(&self, controller_id: &str) -> bool {
        let inner = self.inner.lock();
        inner.active.as_deref() == Some(controller_id)
    }

    /// Evaluate controller liveness and trigger promotion if required.
    pub fn evaluate(&self, now: Instant) -> Option<FailoverEvent> {
        let mut inner = self.inner.lock();
        let Some(active_id) = inner.active.clone() else {
            return self.promote_next_locked(&mut inner, FailoverReason::Startup, None);
        };

        let Some(active) = inner.controllers.get_mut(&active_id) else {
            return self.promote_next_locked(
                &mut inner,
                FailoverReason::Missing,
                Some(active_id.as_str()),
            );
        };

        match active.evaluate(now) {
            HeartbeatStatus::Missing(_) => {
                warn!(grid = %self.grid_id, controller = %active_id, "heartbeat missing; initiating failover");
                active.is_active = false;
                return self.promote_next_locked(
                    &mut inner,
                    FailoverReason::HeartbeatTimeout,
                    Some(active_id.as_str()),
                );
            }
            HeartbeatStatus::Late(delay) => {
                debug!(grid = %self.grid_id, controller = %active_id, delay_us = delay.as_micros(), "late heartbeat");
            }
            HeartbeatStatus::OnTime => {}
        }
        None
    }

    fn promote_next_locked(
        &self,
        inner: &mut SupervisorInner,
        reason: FailoverReason,
        exclude: Option<&str>,
    ) -> Option<FailoverEvent> {
        let Some((next_id, _)) = inner
            .controllers
            .iter()
            .filter(|(id, state)| !state.is_active && exclude.map_or(true, |ex| id.as_str() != ex))
            .min_by(|(_, a), (_, b)| priority_cmp(a, b))
        else {
            warn!(grid = %self.grid_id, "no standby controllers available for promotion");
            inner.active = None;
            return None;
        };
        let next_id = next_id.clone();
        if let Some(active_id) = inner.active.replace(next_id.clone()) {
            if let Some(active) = inner.controllers.get_mut(&active_id) {
                active.is_active = false;
            }
        }
        if let Some(next) = inner.controllers.get_mut(&next_id) {
            next.is_active = true;
        }
        let event = FailoverEvent {
            grid_id: self.grid_id.clone(),
            activated_controller: next_id.clone(),
            triggered_at: Utc::now(),
            reason,
        };
        info!(grid = %event.grid_id, controller = %event.activated_controller, ?reason, "controller promoted");
        Some(event)
    }
}

fn priority_cmp(a: &ControllerRuntimeState, b: &ControllerRuntimeState) -> Ordering {
    role_priority(&a.context.role)
        .cmp(&role_priority(&b.context.role))
        .then_with(|| a.context.failover_order.cmp(&b.context.failover_order))
}

fn context_cmp(a: &ControllerContext, b: &ControllerContext) -> Ordering {
    role_priority(&a.role)
        .cmp(&role_priority(&b.role))
        .then_with(|| a.failover_order.cmp(&b.failover_order))
}

fn role_priority(role: &ControllerRole) -> u8 {
    match role {
        ControllerRole::Primary => 0,
        ControllerRole::Secondary => 1,
        ControllerRole::Follower => 2,
        ControllerRole::Observer => 3,
    }
}

/// Event emitted by the redundancy supervisor on promotions.
#[derive(Debug, Clone)]
pub struct FailoverEvent {
    pub grid_id: String,
    pub activated_controller: String,
    pub triggered_at: DateTime<Utc>,
    pub reason: FailoverReason,
}

#[derive(Debug, Clone, Copy)]
pub enum FailoverReason {
    Startup,
    Manual,
    HeartbeatTimeout,
    Missing,
}

/// Outcome of a promotion cycle.
#[derive(Debug, Clone)]
pub struct Promotion {
    pub event: FailoverEvent,
}
