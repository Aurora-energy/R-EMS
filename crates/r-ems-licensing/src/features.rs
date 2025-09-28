//! ---
//! ems_section: "14-versioning-licensing-system"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Licensing enforcement and entitlement checks."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::core::LicenseTier;

/// Enumeration of license-controlled features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    /// Core simulation tooling available to all users.
    Simulation,
    /// High-availability marine redundancy capabilities.
    MarineRedundancy,
    /// Security hardening and certificate management.
    SecurityHardening,
    /// Certificate provisioning hooks.
    Certificates,
}

impl Feature {
    /// Stable identifier string for API serialisation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Feature::Simulation => "simulation",
            Feature::MarineRedundancy => "marine_redundancy",
            Feature::SecurityHardening => "security_hardening",
            Feature::Certificates => "certificates",
        }
    }
}

/// Matrix mapping features to enabled/disabled states.
#[derive(Debug, Clone)]
pub struct FeatureMatrix {
    inner: BTreeMap<Feature, bool>,
}

impl FeatureMatrix {
    /// Construct a matrix from raw payload strings and the license tier.
    #[must_use]
    pub fn from_payload(raw: &[String], tier: LicenseTier) -> Self {
        let mut inner = BTreeMap::new();
        // Simulation is always available (free feature).
        inner.insert(Feature::Simulation, true);

        let mut marine = raw
            .iter()
            .any(|entry| entry == Feature::MarineRedundancy.as_str());
        if matches!(tier, LicenseTier::Development) {
            marine = false;
        }
        inner.insert(Feature::MarineRedundancy, marine);

        if raw
            .iter()
            .any(|entry| entry == Feature::SecurityHardening.as_str())
        {
            warn!("security_hardening feature requested but excluded by policy");
        }
        inner.insert(Feature::SecurityHardening, false);

        if raw
            .iter()
            .any(|entry| entry == Feature::Certificates.as_str())
        {
            warn!("certificates feature requested but excluded by policy");
        }
        inner.insert(Feature::Certificates, false);

        Self { inner }
    }

    /// Returns true if the feature is enabled.
    #[must_use]
    pub fn is_enabled(&self, feature: Feature) -> bool {
        self.inner.get(&feature).copied().unwrap_or(false)
    }

    /// Returns a serialisable map representation.
    #[must_use]
    pub fn to_map(&self) -> BTreeMap<String, bool> {
        self.inner
            .iter()
            .map(|(feature, enabled)| (feature.as_str().to_owned(), *enabled))
            .collect()
    }
}
