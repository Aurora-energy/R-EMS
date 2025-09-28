//! ---
//! ems_section: "06-security-access-control"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Security policies, identity, and cryptographic utilities."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use prometheus::{IntCounter, Registry};
use std::sync::Arc;

/// Security metrics exported via Prometheus.
#[derive(Clone)]
pub struct SecurityMetrics {
    registry: Arc<Registry>,
    auth_attempts_total: IntCounter,
    auth_failures_total: IntCounter,
    rbac_denials_total: IntCounter,
}

impl SecurityMetrics {
    /// Register metrics with the provided registry.
    pub fn new(registry: Arc<Registry>) -> anyhow::Result<Self> {
        let auth_attempts_total =
            IntCounter::new("auth_attempts_total", "Total authentication attempts")?;
        let auth_failures_total =
            IntCounter::new("auth_failures_total", "Failed authentication attempts")?;
        let rbac_denials_total =
            IntCounter::new("rbac_denials_total", "Access denials due to RBAC policies")?;

        registry.register(Box::new(auth_attempts_total.clone()))?;
        registry.register(Box::new(auth_failures_total.clone()))?;
        registry.register(Box::new(rbac_denials_total.clone()))?;

        Ok(Self {
            registry,
            auth_attempts_total,
            auth_failures_total,
            rbac_denials_total,
        })
    }

    /// Access the underlying registry.
    pub fn registry(&self) -> Arc<Registry> {
        self.registry.clone()
    }

    /// Increment authentication attempts.
    pub fn inc_auth_attempt(&self) {
        self.auth_attempts_total.inc();
    }

    /// Increment authentication failures.
    pub fn inc_auth_failure(&self) {
        self.auth_failures_total.inc();
    }

    /// Increment RBAC denials.
    pub fn inc_rbac_denial(&self) {
        self.rbac_denials_total.inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_increment() {
        let registry = Arc::new(Registry::new());
        let metrics = SecurityMetrics::new(registry.clone()).unwrap();
        metrics.inc_auth_attempt();
        metrics.inc_auth_failure();
        metrics.inc_rbac_denial();
        let families = registry.gather();
        assert_eq!(families.len(), 3);
    }
}
