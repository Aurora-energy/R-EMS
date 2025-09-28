//! ---
//! ems_section: "07-resilience-fault-tolerance"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Resilience strategies and chaos tooling."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
#![warn(missing_docs)]

pub mod chaos;
pub mod degradation;
pub mod failover;
pub mod metrics;
pub mod self_healing;

pub use chaos::{ChaosAction, ChaosEngine, ChaosEventRecord, ChaosScenario};
pub use degradation::{DegradationLevel, DegradationPolicy, DegradationState, DegradationTracker};
pub use failover::{FailoverStressConfig, FailoverStressReport, FailoverStressRunner};
pub use metrics::ResilienceMetrics;
pub use self_healing::{RestartPolicy, SelfHealingManager, SelfHealingOutcome};

/// Crate prelude collecting the most commonly used builders.
pub mod prelude {
    pub use super::chaos::{ChaosEngine, ChaosScenario};
    pub use super::degradation::{
        DegradationLevel, DegradationPolicy, DegradationState, DegradationTracker,
    };
    pub use super::failover::{FailoverStressConfig, FailoverStressReport, FailoverStressRunner};
    pub use super::metrics::ResilienceMetrics;
    pub use super::self_healing::{RestartPolicy, SelfHealingManager, SelfHealingOutcome};
}
