//! ---
//! ems_section: "07-resilience-fault-tolerance"
//! ems_subsection: "example"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Example showcasing chaos engineering utilities."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
/*! ---
ems_section: "07-resilience"
ems_subsection: "07-tests"
ems_type: "code"
ems_scope: "core"
ems_description: "Example combining chaos injection with failover stress metrics."
ems_version: "v0.1.0"
ems_owner: "tbd"
reference: docs/VERSIONING.md
--- */

use std::time::Duration;

use indexmap::IndexMap;
use r_ems_common::config::{ControllerConfig, ControllerRole};
use r_ems_metrics::new_registry;
use r_ems_resilience::{
    ChaosEngine, ChaosScenario, FailoverStressConfig, FailoverStressRunner, ResilienceMetrics,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise metrics shared across the demo.
    let registry = new_registry();
    let metrics = ResilienceMetrics::new(registry.clone()).ok();

    // Run the chaos engine using the development scenario.
    let scenario = ChaosScenario::from_file("configs/chaos.dev.toml")?;
    let mut chaos = ChaosEngine::new(scenario, metrics.clone());
    let records = chaos.execute().await?;
    println!("Chaos actions executed: {}", records.len());

    // Build a simple failover scenario with two controllers.
    let mut controllers = IndexMap::new();
    let mut primary = ControllerConfig::default();
    primary.role = ControllerRole::Primary;
    primary.watchdog_timeout = Duration::from_millis(750);
    controllers.insert("primary".into(), primary);

    let mut secondary = ControllerConfig::default();
    secondary.role = ControllerRole::Secondary;
    secondary.failover_order = 1;
    secondary.watchdog_timeout = Duration::from_millis(750);
    controllers.insert("secondary".into(), secondary);

    let config = FailoverStressConfig::new("demo-grid", controllers).with_grace(Duration::from_millis(50));
    let mut runner = FailoverStressRunner::new(config, metrics.clone())?;

    let first = runner.run_iteration().await?;
    println!(
        "Failover promoted {} after {:?} because {:?}",
        first.promoted_controller, first.recovery_duration, first.reason
    );

    let second = runner.run_iteration().await?;
    println!(
        "Second failover returned control to {} after {:?}",
        second.promoted_controller, second.recovery_duration
    );

    if let Some(metrics) = metrics {
        let families = metrics.registry().gather();
        if let Some(family) = families
            .iter()
            .find(|fam| fam.get_name() == "r_ems_resilience_failovers_total")
        {
            for metric in family.get_metric() {
                let labels = metric
                    .get_label()
                    .iter()
                    .map(|label| format!("{}={}", label.get_name(), label.get_value()))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "Recorded failover metric [{}] = {}",
                    labels,
                    metric.get_counter().get_value()
                );
            }
        }
    }

    println!("Resilience demo completed successfully");
    Ok(())
}
