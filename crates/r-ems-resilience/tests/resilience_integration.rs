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

use anyhow::anyhow;
use indexmap::IndexMap;
use r_ems_common::config::ControllerConfig;
use r_ems_metrics::new_registry;
use r_ems_resilience::{
    ChaosEngine, ChaosScenario, DegradationPolicy, DegradationTracker, FailoverStressConfig,
    FailoverStressRunner, ResilienceMetrics, RestartPolicy, SelfHealingManager,
};

#[tokio::test]
async fn combined_resilience_flow() {
    let registry = new_registry();
    let metrics = ResilienceMetrics::new(registry.clone()).unwrap();

    // Chaos scenario with a single kill action to keep the test quick.
    let scenario = r#"
        jitter_ms = 0
        [[actions]]
        type = "kill_controller"
        grid = "grid-a"
        controller = "primary"
        delay_sec = 0
        "#
    .parse::<ChaosScenario>()
    .unwrap();
    let mut chaos = ChaosEngine::new(scenario, Some(metrics.clone()));
    let records = chaos.execute().await.expect("chaos should run");
    assert_eq!(records.len(), 1);

    // Failover between primary and secondary controllers.
    let mut controllers = IndexMap::new();
    let primary = ControllerConfig {
        watchdog_timeout: Duration::from_millis(5),
        ..Default::default()
    };
    controllers.insert("primary".into(), primary);
    let secondary = ControllerConfig {
        watchdog_timeout: Duration::from_millis(5),
        ..Default::default()
    };
    controllers.insert("secondary".into(), secondary);

    let config =
        FailoverStressConfig::new("grid-a", controllers).with_grace(Duration::from_millis(1));
    let mut runner = FailoverStressRunner::new(config, Some(metrics.clone())).unwrap();
    let report = runner.run_iteration().await.unwrap();
    assert_eq!(report.promoted_controller, "secondary");

    // Degradation transitions as controllers fail and recover.
    let mut tracker =
        DegradationTracker::new(DegradationPolicy::default(), 2, Some(metrics.clone()));
    let degraded = tracker.evaluate(1);
    assert_eq!(degraded.level.as_str(), "degraded");
    let healthy = tracker.evaluate(2);
    assert_eq!(healthy.level.as_str(), "healthy");

    // Self-healing succeeds on the second attempt.
    let policy = RestartPolicy::new(3, Duration::from_millis(1), Duration::from_millis(1));
    let mut manager = SelfHealingManager::new(policy, Some(metrics.clone()));
    let attempt_tracker = std::sync::Arc::new(std::sync::Mutex::new(0usize));
    let tracker_clone = attempt_tracker.clone();
    let outcome = manager
        .attempt_recovery(
            "primary",
            move |attempt| {
                let tracker_inner = tracker_clone.clone();
                async move {
                    *tracker_inner.lock().unwrap() = attempt;
                    if attempt < 2 {
                        Err(anyhow!("simulated restart failure"))
                    } else {
                        Ok(())
                    }
                }
            },
            &["secondary".into()],
        )
        .await
        .unwrap();
    assert!(outcome.success);
    assert_eq!(*attempt_tracker.lock().unwrap(), 2);

    // Ensure metrics were registered.
    let metric_names: Vec<_> = registry
        .gather()
        .iter()
        .map(|fam| fam.get_name().to_string())
        .collect();
    assert!(metric_names.contains(&"r_ems_resilience_chaos_events_total".to_string()));
    assert!(metric_names.contains(&"r_ems_resilience_failovers_total".to_string()));
    assert!(metric_names.contains(&"r_ems_resilience_self_heal_restarts_total".to_string()));
}
