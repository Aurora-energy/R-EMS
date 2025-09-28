//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Shared primitives and utilities for the core runtime."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::env;

use base64::{engine::general_purpose, Engine as _};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use r_ems_common::config::LicenseConfig;
use r_ems_common::license::LicenseValidator;
use sha2::Sha256;

fn encode_license(owner: &str, key_id: &str, expires_in_days: i64) -> String {
    let expires_at = (Utc::now() + Duration::days(expires_in_days)).to_rfc3339();
    let issued_at = Utc::now().to_rfc3339();
    let mut mac = Hmac::<Sha256>::new_from_slice(b"R-EMS-MOCK-SALT").unwrap();
    mac.update(owner.as_bytes());
    mac.update(key_id.as_bytes());
    mac.update(expires_at.as_bytes());
    mac.update(issued_at.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    let payload = serde_json::json!({
        "owner": owner,
        "key_id": key_id,
        "expires_at": expires_at,
        "issued_at": issued_at,
        "signature": signature,
    });
    general_purpose::STANDARD.encode(serde_json::to_vec(&payload).unwrap())
}

#[test]
fn license_validation_success() {
    let license_str = encode_license("Test Owner", "KEY-123", 10);
    env::set_var("R_EMS_LICENSE", license_str);
    let config = LicenseConfig::default();
    let validator = LicenseValidator::new(&config);
    let result = validator.validate(false).expect("license should validate");
    assert!(result.is_valid(), "expected valid license");
}

#[test]
fn license_validation_fails_without_material() {
    env::remove_var("R_EMS_LICENSE");
    let config = LicenseConfig {
        allow_bypass: false,
        ..Default::default()
    };
    let validator = LicenseValidator::new(&config);
    let err = validator.validate(false).expect_err("expected failure");
    assert!(err.to_string().contains("license material missing"));
}
