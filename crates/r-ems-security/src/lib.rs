//! ---
//! ems_section: "06-security-access-control"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Security policies, identity, and cryptographic utilities."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
#![warn(missing_docs)]

pub mod audit;
pub mod certificates;
pub mod compliance;
pub mod crypto;
pub mod identity;
pub mod metrics;
pub mod rbac;

pub use audit::{AuditEntry, AuditLog};
pub use certificates::{CertificateAuthority, CertificateConfig, CertificateStatus};
pub use compliance::{ComplianceMode, ComplianceReport};
pub use crypto::{KeyMaterial, TlsAssets};
pub use identity::{ApiKey, IdentityProvider, TokenClaims, UserAccount};
pub use metrics::SecurityMetrics;
pub use rbac::{Permission, RbacEngine, Role, RoleAssignment};
