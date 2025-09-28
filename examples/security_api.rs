//! ---
//! ems_section: "06-security-access-control"
//! ems_subsection: "example"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Example interacting with security APIs."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
/*! ---
ems_section: "06-security"
ems_subsection: "09-examples"
ems_type: "code"
ems_scope: "core"
ems_description: "Example demonstrating API key issuance and RBAC enforcement."
ems_version: "v0.1.0"
ems_owner: "tbd"
reference: docs/VERSIONING.md
--- */

use chrono::Duration;
use r_ems_security::audit::AuditLog;
use r_ems_security::identity::{IdentityProvider, UserAccount};
use r_ems_security::rbac::{Permission, RbacEngine, Role};

fn main() -> anyhow::Result<()> {
    // Provision identity provider with an admin.
    let provider = IdentityProvider::new();
    provider.upsert_user(UserAccount::new("admin", "alice", vec![Role::admin()]));

    // Issue API key for the admin user.
    let api_key = provider.issue_api_key("admin", &["commands".into()], Some(Duration::hours(1)))?;

    // Authenticate and check RBAC permissions.
    let claims = provider.authenticate_api_key(&api_key.secret)?;
    let engine = RbacEngine::new();
    let allowed = engine.is_authorized(&claims.roles, Permission::ExecuteCommand)?;
    println!("RBAC allowed: {allowed}");

    // Append audit log entry for the command.
    let mut log = AuditLog::new("security_example_audit.log")?;
    log.append(&claims.subject, "command.execute", serde_json::json!({ "command": "restart" }))?;

    println!("API key: {}", api_key.secret);
    Ok(())
}
