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
use r_ems_redundancy::{ControllerContext, RedundancySupervisor};

fn controller_config(role: ControllerRole, failover_order: u32) -> ControllerConfig {
    ControllerConfig {
        role,
        heartbeat_interval: Duration::from_millis(10),
        watchdog_timeout: Duration::from_millis(20),
        failover_order,
        ..ControllerConfig::default()
    }
}

#[test]
fn supervisor_promotes_secondary_on_missed_heartbeat() {
    let supervisor = RedundancySupervisor::new("grid-a");
    let primary = ControllerContext::from_config(
        "grid-a",
        "primary",
        &controller_config(ControllerRole::Primary, 0),
    );
    let secondary = ControllerContext::from_config(
        "grid-a",
        "secondary",
        &controller_config(ControllerRole::Secondary, 1),
    );

    supervisor.register(primary.clone());
    supervisor.register(secondary.clone());

    let now = Instant::now();
    supervisor.heartbeat("primary", now);
    let future = now + Duration::from_millis(50);
    let event = supervisor
        .evaluate(future)
        .expect("expect promotion event when heartbeat missing");
    assert_eq!(event.activated_controller, "secondary");
    assert!(supervisor.is_active("secondary"));
}
