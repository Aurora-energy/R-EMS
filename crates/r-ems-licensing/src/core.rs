//! ---
//! ems_section: "14-versioning-licensing-system"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Licensing enforcement and entitlement checks."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fmt;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::certificates::{verify_certificate, LicenseCertificate};
use crate::features::{Feature, FeatureMatrix};
use crate::logging::{record_invalid_license, record_license_load};

/// Tier assigned to a license payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LicenseTier {
    /// Developer or testing use in isolated environments.
    Development,
    /// Non-commercial operation for evaluation or education.
    NonCommercial,
}

impl fmt::Display for LicenseTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LicenseTier::Development => write!(f, "development"),
            LicenseTier::NonCommercial => write!(f, "non-commercial"),
        }
    }
}

/// Metadata extracted from a validated license.
#[derive(Debug, Clone)]
pub struct LicenseDetails {
    /// Key identifier embedded in the license certificate.
    pub key_id: String,
    /// Owner string provided by the issuing authority.
    pub owner: String,
    /// Tier of the license (development/non-commercial).
    pub tier: LicenseTier,
    /// Expiry timestamp for the license.
    pub expires_at: DateTime<Utc>,
    /// Optional issuance timestamp.
    pub issued_at: Option<DateTime<Utc>>,
    /// Whether commercial deployments are disallowed.
    pub non_commercial_only: bool,
    /// Feature matrix computed from the license payload.
    pub features: FeatureMatrix,
    /// Raw license string as provided.
    pub raw: String,
}

impl LicenseDetails {
    /// Returns true if the license is past its expiry.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at < Utc::now()
    }

    /// Query whether a named feature is enabled.
    #[must_use]
    pub fn allows(&self, feature: Feature) -> bool {
        self.features.is_enabled(feature)
    }
}

/// Outcome of the license validation pipeline.
#[derive(Debug, Clone)]
pub enum LicenseValidation {
    /// License validated successfully.
    Valid(LicenseDetails),
    /// License bypass granted for development flows.
    Bypassed { reason: String },
}

impl LicenseValidation {
    /// Convenience helper to access metadata when valid.
    #[must_use]
    pub fn metadata(&self) -> Option<&LicenseDetails> {
        match self {
            LicenseValidation::Valid(details) => Some(details),
            LicenseValidation::Bypassed { .. } => None,
        }
    }

    /// Returns true when the license is valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, LicenseValidation::Valid(_))
    }
}

/// Coordinator responsible for turning raw license material into [`LicenseDetails`].
#[derive(Debug, Clone)]
pub struct LicenseManager {
    public_key: [u8; 32],
}

impl Default for LicenseManager {
    fn default() -> Self {
        Self {
            public_key: crate::certificates::DEV_PUBLIC_KEY,
        }
    }
}

impl LicenseManager {
    /// Construct a manager with the default embedded public key.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the public key used for verification (useful in tests).
    #[must_use]
    pub fn with_public_key(public_key: [u8; 32]) -> Self {
        Self { public_key }
    }

    /// Parse, verify, and hydrate a license from a raw string.
    pub fn parse(&self, raw: &str) -> Result<LicenseDetails> {
        let certificate = match LicenseCertificate::decode(raw) {
            Ok(cert) => cert,
            Err(err) => {
                record_invalid_license("malformed");
                return Err(err);
            }
        };
        if let Err(err) = verify_certificate(&certificate, &self.public_key) {
            record_invalid_license("invalid_signature");
            return Err(err);
        }
        let payload = certificate.payload;

        if payload.non_commercial_only && matches!(payload.tier, LicenseTier::Development) {
            // Development licenses are inherently non-commercial.
        }

        let expires_at = payload
            .expires_at
            .parse::<DateTime<Utc>>()
            .map_err(|err| anyhow!("invalid expires_at timestamp: {err}"))?;
        let issued_at = payload
            .issued_at
            .as_deref()
            .map(|value| value.parse::<DateTime<Utc>>())
            .transpose()
            .map_err(|err| anyhow!("invalid issued_at timestamp: {err}"))?;

        let features = FeatureMatrix::from_payload(&payload.features, payload.tier);

        let details = LicenseDetails {
            key_id: payload.key_id,
            owner: payload.owner,
            tier: payload.tier,
            expires_at,
            issued_at,
            non_commercial_only: payload.non_commercial_only,
            features,
            raw: raw.to_owned(),
        };

        if details.is_expired() {
            record_invalid_license("expired");
            return Err(anyhow!(
                "license '{}' expired at {}",
                details.key_id,
                details.expires_at
            ));
        }

        record_license_load(&details);
        Ok(details)
    }
}

/// High level validator that supports optional bypass logic.
#[derive(Debug, Clone)]
pub struct LicenseValidator {
    manager: LicenseManager,
    pub allow_bypass: bool,
}

impl LicenseValidator {
    /// Create a new validator with the default certificate authority.
    #[must_use]
    pub fn new(allow_bypass: bool) -> Self {
        Self {
            manager: LicenseManager::new(),
            allow_bypass,
        }
    }

    /// Validate raw material, optionally allowing bypass when absent.
    pub fn validate(&self, raw: Option<String>, bypass_flag: bool) -> Result<LicenseValidation> {
        match raw {
            Some(material) => match self.manager.parse(&material) {
                Ok(details) => {
                    info!(
                        key_id = %details.key_id,
                        owner = %details.owner,
                        tier = %details.tier,
                        expires_at = %details.expires_at,
                        "license validated"
                    );
                    Ok(LicenseValidation::Valid(details))
                }
                Err(err) => {
                    if bypass_flag || self.allow_bypass {
                        warn!(error = %err, "license validation failed; bypassing enforcement");
                        Ok(LicenseValidation::Bypassed {
                            reason: format!("validation failed: {err}"),
                        })
                    } else {
                        Err(err)
                    }
                }
            },
            None => {
                if bypass_flag || self.allow_bypass {
                    warn!("license material missing; bypassing enforcement");
                    Ok(LicenseValidation::Bypassed {
                        reason: "license material missing".to_owned(),
                    })
                } else {
                    record_invalid_license("missing");
                    Err(anyhow!("license material missing and bypass not permitted"))
                }
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct LicensePayload {
    key_id: String,
    owner: String,
    tier: LicenseTier,
    expires_at: String,
    #[serde(default)]
    issued_at: Option<String>,
    #[serde(default = "default_non_commercial")]
    non_commercial_only: bool,
    #[serde(default)]
    features: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LicenseEnvelope {
    version: u32,
    #[serde(rename = "payload")]
    payload: LicensePayload,
    signature: String,
}

fn default_non_commercial() -> bool {
    true
}

impl LicenseEnvelope {
    fn decode(raw: &str) -> Result<Self> {
        let bytes = general_purpose::STANDARD
            .decode(raw.trim())
            .with_context(|| "license payload must be base64 encoded")?;
        let envelope: LicenseEnvelope = serde_json::from_slice(&bytes)
            .with_context(|| "license must decode into a JSON envelope")?;
        Ok(envelope)
    }
}

impl LicenseCertificate {
    pub(crate) fn decode(raw: &str) -> Result<Self> {
        let envelope = LicenseEnvelope::decode(raw)?;
        Ok(Self {
            payload: envelope.payload,
            signature: envelope.signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tampered_license() -> (String, String) {
        let raw = std::fs::read_to_string("configs/license.dev.lic")
            .expect("development license fixture should exist");
        let mut tampered_bytes = raw.clone().into_bytes();
        if let Some(last) = tampered_bytes.last_mut() {
            *last = match *last {
                b'=' => b'A',
                b'A' => b'B',
                other => other.wrapping_add(1),
            };
        }
        let tampered = String::from_utf8(tampered_bytes).expect("valid utf8 after tampering");
        (raw, tampered)
    }

    #[test]
    fn invalid_license_blocks_without_bypass() {
        let (valid, tampered) = tampered_license();
        let validator = LicenseValidator::new(false);
        assert!(validator.validate(Some(valid), false).is_ok());
        assert!(validator.validate(Some(tampered), false).is_err());
    }

    #[test]
    fn invalid_license_can_bypass_in_development() {
        let (_, tampered) = tampered_license();
        let validator = LicenseValidator::new(false);
        let result = validator
            .validate(Some(tampered), true)
            .expect("bypass permitted");
        assert!(matches!(result, LicenseValidation::Bypassed { .. }));
    }
}
