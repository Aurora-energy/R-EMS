//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Primary orchestration and lifecycle management."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use anyhow::Result;
use r_ems_versioning::semver::VersionInfo;
use r_ems_versioning::update::{self as versioning_update, UpdateSettings};

use crate::config::UpdateConfig;

pub use versioning_update::{
    detect_source, UpdateCommand, UpdateEntry, UpdateResult, UpdateSource,
};

/// Client responsible for determining update availability using workspace configuration.
#[derive(Debug, Clone)]
pub struct UpdateClient {
    inner: versioning_update::UpdateClient,
}

impl UpdateClient {
    /// Construct an update client from configuration and version metadata.
    pub fn from_config(config: &UpdateConfig, version: VersionInfo) -> Result<Self> {
        let settings = UpdateSettings {
            feed_path: config.feed_path.clone(),
            github_owner: config.github_owner.clone(),
            github_repo: config.github_repo.clone(),
            allow_apply_in_dev: config.allow_apply_in_dev,
        };
        Ok(Self {
            inner: versioning_update::UpdateClient::new(settings, version),
        })
    }

    /// Execute an update check.
    pub async fn check(&self) -> Result<UpdateResult> {
        self.inner.check().await
    }

    /// Apply an update when permitted.
    pub async fn apply(&self, result: &UpdateResult) -> Result<()> {
        self.inner.apply(result).await
    }

    /// Expose the underlying update settings.
    #[must_use]
    pub fn settings(&self) -> &UpdateSettings {
        self.inner.settings()
    }
}
