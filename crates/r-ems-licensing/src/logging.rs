//! ---
//! ems_section: "14-versioning-licensing-system"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Licensing enforcement and entitlement checks."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use once_cell::sync::Lazy;
use prometheus::{register_int_counter, IntCounter};
use tracing::info;

use crate::core::LicenseDetails;

static LICENSE_LOADS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "license_loads_total",
        "Total number of license loads that succeeded"
    )
    .expect("metric registration to succeed")
});

static LICENSE_INVALID_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "license_invalid_total",
        "Total number of invalid license loads"
    )
    .expect("metric registration to succeed")
});

/// Record a successful license validation event.
pub fn record_license_load(details: &LicenseDetails) {
    LICENSE_LOADS_TOTAL.inc();
    info!(
        key_id = %details.key_id,
        owner = %details.owner,
        tier = %details.tier,
        "license accepted"
    );
}

/// Record an invalid license scenario.
pub fn record_invalid_license(reason: &str) {
    LICENSE_INVALID_TOTAL.inc();
    info!(reason = reason, "license rejected");
}
