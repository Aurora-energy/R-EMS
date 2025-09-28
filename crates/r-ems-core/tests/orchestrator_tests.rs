//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Primary orchestration and lifecycle management."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::time::Duration;

use uuid::Uuid;

use indexmap::IndexMap;
use r_ems_common::config::{
    AppConfig, ControllerConfig, ControllerRole, GridConfig, LicenseConfig, Mode,
};
use r_ems_common::license::LicenseValidation;
use r_ems_core::orchestrator::RemsOrchestrator;
use r_ems_core::update::UpdateClient;
use tempfile::tempdir;

#[allow(clippy::field_reassign_with_default)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn orchestrator_start_stop_simulation() {
    let mut config = AppConfig::default();
    config.mode = Mode::Simulation;
    let mut grid = GridConfig::default();
    let mut controllers: IndexMap<String, ControllerConfig> = IndexMap::new();
    let mut primary = ControllerConfig::default();
    primary.role = ControllerRole::Primary;
    primary.heartbeat_interval = Duration::from_millis(20);
    primary.watchdog_timeout = Duration::from_millis(40);
    controllers.insert("primary".into(), primary);
    let mut secondary = ControllerConfig::default();
    secondary.role = ControllerRole::Secondary;
    secondary.failover_order = 1;
    controllers.insert("secondary".into(), secondary);
    grid.controllers = controllers;
    config.grids.insert("grid-a".into(), grid);
    config.license = LicenseConfig {
        allow_bypass: true,
        ..LicenseConfig::default()
    };

    let license = LicenseValidation::Bypassed {
        reason: "integration test".into(),
    };
    let update_client = UpdateClient::from_config(
        &config.update,
        r_ems_common::version::VersionInfo::current(),
    )
    .unwrap();
    let orchestrator = RemsOrchestrator::new(config, license, update_client, None);
    let handle = orchestrator.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.shutdown().await.unwrap();
}

#[allow(clippy::field_reassign_with_default)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failover_promotes_secondary_and_gates_snapshots() {
    let temp = tempdir().expect("tempdir");

    let grid_id = format!("grid-{}", Uuid::new_v4().simple());

    let mut config = AppConfig::default();
    config.mode = Mode::Simulation;

    let mut grid = GridConfig::default();
    grid.snapshot.directory = temp.path().to_path_buf();
    grid.snapshot.retain_last = 10;

    let mut primary = ControllerConfig::default();
    primary.role = ControllerRole::Primary;
    primary.heartbeat_interval = Duration::from_millis(10);
    primary.watchdog_timeout = Duration::from_millis(40);
    primary
        .metadata
        .insert("exit_after_ticks".into(), "10".into());

    let mut secondary = ControllerConfig::default();
    secondary.role = ControllerRole::Secondary;
    secondary.failover_order = 1;
    secondary.heartbeat_interval = Duration::from_millis(10);
    secondary.watchdog_timeout = Duration::from_millis(40);

    let mut controllers: IndexMap<String, ControllerConfig> = IndexMap::new();
    controllers.insert("primary".into(), primary);
    controllers.insert("secondary".into(), secondary);
    grid.controllers = controllers;

    config.grids.insert(grid_id.clone(), grid);
    config.license = LicenseConfig {
        allow_bypass: true,
        ..LicenseConfig::default()
    };

    let license = LicenseValidation::Bypassed {
        reason: "integration test".into(),
    };
    let update_client = UpdateClient::from_config(
        &config.update,
        r_ems_common::version::VersionInfo::current(),
    )
    .unwrap();
    let orchestrator = RemsOrchestrator::new(config, license, update_client, None);
    let handle = orchestrator.start().await.unwrap();

    let primary_dir = temp.path().join(&grid_id).join("primary");
    let secondary_dir = temp.path().join(&grid_id).join("secondary");

    let mut primary_ready = false;
    for _ in 0..40 {
        if dir_count(&primary_dir) > 0 {
            primary_ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    println!(
        "primary_count={} secondary_count={} (pre-failover)",
        dir_count(&primary_dir),
        dir_count(&secondary_dir)
    );
    assert!(primary_ready, "expected primary snapshots to be present");
    assert_eq!(dir_count(&secondary_dir), 0);

    let mut secondary_ready = false;
    for _ in 0..40 {
        if dir_count(&secondary_dir) > 0 {
            secondary_ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    println!(
        "primary_only={} secondary_only={}",
        dir_count(&primary_dir),
        dir_count(&secondary_dir)
    );
    assert!(secondary_ready, "expected standby snapshots after failover");

    handle.shutdown().await.unwrap();

    let log_dir = std::path::Path::new("target/test-logs");
    let primary_log = log_dir.join(format!("{}-primary-jitter.json", grid_id));
    let secondary_log = log_dir.join(format!("{}-secondary-jitter.json", grid_id));

    assert!(
        primary_log.exists(),
        "expected jitter log for primary controller at {:?}",
        primary_log
    );
    assert!(
        secondary_log.exists(),
        "expected jitter log for secondary controller at {:?}",
        secondary_log
    );

    let primary_len = std::fs::metadata(&primary_log)
        .expect("primary jitter log metadata")
        .len();
    let secondary_len = std::fs::metadata(&secondary_log)
        .expect("secondary jitter log metadata")
        .len();

    assert!(primary_len > 0, "primary jitter log should not be empty");
    assert!(
        secondary_len > 0,
        "secondary jitter log should not be empty"
    );

    // Best-effort cleanup to avoid accumulating large logs between test runs.
    let _ = std::fs::remove_file(primary_log);
    let _ = std::fs::remove_file(secondary_log);
}

fn dir_count(path: &std::path::Path) -> usize {
    std::fs::read_dir(path)
        .map(|iter| iter.count())
        .unwrap_or(0)
}
