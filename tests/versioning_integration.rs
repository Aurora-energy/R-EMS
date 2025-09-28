//! ---
//! ems_section: "15-testing-qa-runbook"
//! ems_subsection: "integration-tests"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Integration and validation tests for the R-EMS stack."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use r_ems_licensing::core::LicenseManager;
use r_ems_versioning::semver::VersionInfo;
use r_ems_versioning::update::{UpdateClient, UpdateEntry, UpdateResult, UpdateSettings};

#[test]
fn tampered_license_is_rejected() {
    let raw = std::fs::read_to_string("configs/license.dev.lic").expect("dev license present");
    let mut tampered_bytes = raw.clone().into_bytes();
    if let Some(last) = tampered_bytes.last_mut() {
        *last = match *last { b'=' => b'A', b'A' => b'B', other => other.wrapping_add(1) };
    }
    let tampered = String::from_utf8(tampered_bytes).expect("valid utf8 tampering");
    let manager = LicenseManager::new();
    assert!(manager.parse(&raw).is_ok());
    assert!(manager.parse(&tampered).is_err());
}

#[tokio::test]
async fn unsigned_release_rejected_on_apply() {
    let mut version = VersionInfo::current();
    version.semver = "0.1.0".to_string();
    let settings = UpdateSettings {
        feed_path: std::path::PathBuf::from("configs/update_feed.json"),
        github_owner: None,
        github_repo: None,
        allow_apply_in_dev: true,
    };
    let client = UpdateClient::new(settings, version.clone());
    let entry = UpdateEntry {
        version: "9.9.9".to_string(),
        commit: Some("abcdef".to_string()),
        download_url: None,
        notes: None,
        published_at: None,
        signature: None,
    };
    let result = UpdateResult {
        current: version,
        latest: Some(entry),
    };
    let err = client.apply(&result).await.expect_err("unsigned release should fail");
    assert!(err.to_string().contains("unsigned"));
}
