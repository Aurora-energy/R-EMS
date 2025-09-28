//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Shared primitives and utilities for the core runtime."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;

use anyhow::{Context, Result};
use tracing::debug;

use crate::config::LicenseConfig;
use r_ems_licensing::core::{LicenseValidation, LicenseValidator as InnerValidator};

pub use r_ems_licensing::core::{LicenseDetails, LicenseTier};
pub use r_ems_licensing::features::{Feature, FeatureMatrix};
pub use r_ems_licensing::logging::{record_invalid_license, record_license_load};

/// Trait abstraction for license validation strategies.
pub trait LicenseAuthority {
    /// Verify the license returning validation state.
    fn verify(&self, bypass_flag: bool) -> Result<LicenseValidation>;
}

/// Loads and validates license material for the daemon.
#[derive(Debug, Clone)]
pub struct LicenseValidator {
    config: LicenseConfig,
    inner: InnerValidator,
}

impl LicenseValidator {
    /// Create a new validator using the provided configuration.
    #[must_use]
    pub fn new(config: &LicenseConfig) -> Self {
        Self {
            config: config.clone(),
            inner: InnerValidator::new(config.allow_bypass),
        }
    }

    /// Validate the license, optionally allowing a developer bypass.
    pub fn validate(&self, bypass_flag: bool) -> Result<LicenseValidation> {
        let material = self.load_material()?;
        self.inner.validate(material, bypass_flag)
    }

    fn load_material(&self) -> Result<Option<String>> {
        if let Some(path) = &self.config.path {
            if path.exists() {
                debug!(license_path = %path.display(), "loading license file");
                let raw = fs::read_to_string(path)
                    .with_context(|| format!("unable to read license file {}", path.display()))?;
                return Ok(Some(raw.trim().to_owned()));
            }
        }

        match std::env::var(&self.config.env_var) {
            Ok(value) if !value.trim().is_empty() => {
                debug!(env = %self.config.env_var, "loaded license material from environment");
                Ok(Some(value.trim().to_owned()))
            }
            _ => Ok(None),
        }
    }
}

impl LicenseAuthority for LicenseValidator {
    fn verify(&self, bypass_flag: bool) -> Result<LicenseValidation> {
        self.validate(bypass_flag)
    }
}

/// Mock authority returning a predetermined response, useful for tests and examples.
#[derive(Debug, Clone)]
pub struct MockLicenseAuthority {
    response: LicenseValidation,
}

impl MockLicenseAuthority {
    /// Construct a new mock authority.
    #[must_use]
    pub fn new(response: LicenseValidation) -> Self {
        Self { response }
    }
}

impl LicenseAuthority for MockLicenseAuthority {
    fn verify(&self, _bypass_flag: bool) -> Result<LicenseValidation> {
        Ok(self.response.clone())
    }
}
