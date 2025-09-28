//! ---
//! ems_section: "15-testing-qa-runbook"
//! ems_subsection: "integration-tests"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Integration and validation tests for the R-EMS stack."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use chrono::Duration;
use r_ems_security::audit::AuditLog;
use r_ems_security::compliance::{generate_report, ComplianceMode};
use r_ems_security::crypto::{load_tls_assets, TlsConfig};
use r_ems_security::identity::{IdentityProvider, UserAccount};
use r_ems_security::metrics::SecurityMetrics;
use r_ems_security::rbac::{Permission, RbacEngine, Role};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn end_to_end_access_control_flow() {
    // Identity + API key issuance
    let provider = IdentityProvider::new();
    provider.upsert_user(UserAccount::new("admin", "alice", vec![Role::admin()]));
    let api_key = provider
        .issue_api_key("admin", &["commands".into()], Some(Duration::minutes(5)))
        .unwrap();
    let claims = provider.authenticate_api_key(&api_key.secret).unwrap();

    // RBAC enforcement
    let engine = RbacEngine::new();
    assert!(engine
        .is_authorized(&claims.roles, Permission::ExecuteCommand)
        .unwrap());

    // Audit log entry
    let dir = tempdir().unwrap();
    let path = dir.path().join("audit.log");
    let mut log = AuditLog::new(&path).unwrap();
    log.append(&claims.subject, "command.execute", serde_json::json!({"command": "restart"}))
        .unwrap();
    assert!(log.verify().unwrap());

    // Compliance report
    let report = generate_report(ComplianceMode::Strict);
    assert_eq!(report.mode, ComplianceMode::Strict);

    // TLS assets (self-signed dev mode)
    let assets = load_tls_assets(&TlsConfig::default()).unwrap();
    assert!(assets.certificate_pem.contains("BEGIN CERTIFICATE"));

    // Metrics counters
    let registry = Arc::new(prometheus::Registry::new());
    let metrics = SecurityMetrics::new(registry.clone()).unwrap();
    metrics.inc_auth_attempt();
    metrics.inc_auth_failure();
    metrics.inc_rbac_denial();
    assert_eq!(registry.gather().len(), 3);
}
