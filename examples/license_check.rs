//! ---
//! ems_section: "14-versioning-licensing-system"
//! ems_subsection: "example"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Example performing a licensing entitlement check."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};
use r_ems_licensing::core::LicenseManager;

fn main() -> Result<()> {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("configs/license.dev.lic"));
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read license file {}", path.display()))?;
    let manager = LicenseManager::new();
    let details = manager
        .parse(raw.trim())
        .with_context(|| format!("failed to parse license {}", path.display()))?;

    println!("License {} owned by {}", details.key_id, details.owner);
    println!("Expires at {}", details.expires_at);
    println!("Features:");
    for (feature, enabled) in details.features.to_map() {
        println!("  {:<24} {}", feature, enabled);
    }
    Ok(())
}
