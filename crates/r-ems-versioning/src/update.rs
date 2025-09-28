//! ---
//! ems_section: "14-versioning-licensing-system"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Version metadata and release governance helpers."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use octocrab::Octocrab;
use once_cell::sync::Lazy;
use prometheus::{register_int_counter, IntCounter};
use semver::Version;
use serde::Deserialize;
use std::convert::TryInto;

use tokio::fs as async_fs;
use tracing::{debug, info, warn};

use crate::semver::VersionInfo;

static UPDATES_PERFORMED_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "updates_performed_total",
        "Total number of updates applied through the updater"
    )
    .expect("metric registration to succeed")
});

const RELEASE_PUBLIC_KEY: [u8; 32] = [
    168, 0, 169, 32, 60, 42, 128, 57, 90, 246, 86, 71, 142, 136, 197, 255, 102, 76, 29, 121, 51,
    29, 142, 59, 79, 67, 201, 133, 11, 56, 13, 229,
];

/// Source definition for fetching updates.
#[derive(Debug, Clone)]
pub struct UpdateSettings {
    /// Local feed path containing simulated releases.
    pub feed_path: PathBuf,
    /// Optional GitHub owner.
    pub github_owner: Option<String>,
    /// Optional GitHub repository name.
    pub github_repo: Option<String>,
    /// Whether apply operations are allowed in development environments.
    pub allow_apply_in_dev: bool,
}

impl UpdateSettings {
    /// Helper to determine if GitHub lookup is enabled.
    #[must_use]
    pub fn github(&self) -> Option<(&str, &str)> {
        self.github_owner
            .as_deref()
            .zip(self.github_repo.as_deref())
    }
}

/// Supported update command types consumed by the CLI.
#[derive(Debug, Clone, Copy)]
pub enum UpdateCommand {
    /// Perform a read-only check.
    Check,
    /// Apply an available update.
    Apply,
}

/// Client responsible for determining update availability.
#[derive(Debug, Clone)]
pub struct UpdateClient {
    settings: UpdateSettings,
    current: VersionInfo,
}

impl UpdateClient {
    /// Construct a client from settings and the current version metadata.
    #[must_use]
    pub fn new(settings: UpdateSettings, version: VersionInfo) -> Self {
        Self {
            settings,
            current: version,
        }
    }

    /// Access the resolved settings for the client.
    #[must_use]
    pub fn settings(&self) -> &UpdateSettings {
        &self.settings
    }

    /// Check local and remote sources for available updates.
    pub async fn check(&self) -> Result<UpdateResult> {
        let mut latest = self.check_local_feed().await?;
        if latest.is_none() {
            if let Some((owner, repo)) = self.settings.github() {
                latest = self.check_github(owner, repo).await?;
            }
        }
        Ok(UpdateResult {
            current: self.current.clone(),
            latest,
        })
    }

    /// Apply an update when available, subject to configuration constraints.
    pub async fn apply(&self, result: &UpdateResult) -> Result<()> {
        if !self.settings.allow_apply_in_dev {
            return Err(anyhow!(
                "update-apply is restricted to development environments"
            ));
        }
        if !result.update_available() {
            return Err(anyhow!("no updates available to apply"));
        }
        let Some(latest) = &result.latest else {
            return Err(anyhow!("expected update metadata"));
        };
        let mut dir = PathBuf::from("target/update-simulated");
        async_fs::create_dir_all(&dir).await?;
        dir.push(format!(
            "{}-{}.txt",
            latest.version,
            latest.commit.as_deref().unwrap_or("unknown")
        ));
        let contents = format!(
            "Simulated update apply for version {} at {}\n",
            latest.version,
            Utc::now()
        );
        async_fs::write(&dir, contents).await?;
        verify_release_signature(latest)?;
        UPDATES_PERFORMED_TOTAL.inc();
        info!(
            version = %latest.version,
            target = %dir.display(),
            "simulated update apply complete"
        );
        Ok(())
    }

    async fn check_local_feed(&self) -> Result<Option<UpdateEntry>> {
        if !self.settings.feed_path.exists() {
            debug!(path = %self.settings.feed_path.display(), "update feed missing");
            return Ok(None);
        }
        let raw = fs::read_to_string(&self.settings.feed_path).with_context(|| {
            format!(
                "failed reading update feed {}",
                self.settings.feed_path.display()
            )
        })?;
        let feed: UpdateFeed = serde_json::from_str(&raw).with_context(|| {
            format!("invalid update feed {}", self.settings.feed_path.display())
        })?;
        let mut releases = feed.releases;
        releases.sort_by(|a, b| match (a.semver(), b.semver()) {
            (Ok(a), Ok(b)) => b.cmp(&a),
            _ => b.version.cmp(&a.version),
        });
        Ok(releases.into_iter().next())
    }

    async fn check_github(&self, owner: &str, repo: &str) -> Result<Option<UpdateEntry>> {
        let octo = match Octocrab::builder().build() {
            Ok(client) => client,
            Err(err) => {
                warn!(owner, repo, error = %err, "unable to construct GitHub client");
                return Ok(None);
            }
        };
        let response = match octo
            .repos(owner.to_owned(), repo.to_owned())
            .releases()
            .list()
            .per_page(5)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(err) => {
                warn!(owner, repo, error = %err, "failed querying GitHub releases");
                return Ok(None);
            }
        };
        let release = match response.items.into_iter().next() {
            Some(rel) => rel,
            None => return Ok(None),
        };
        let tag = release.tag_name.clone();
        let version = tag.trim_start_matches('v').to_owned();
        let entry = UpdateEntry {
            version,
            commit: Some(release.target_commitish),
            download_url: Some(release.html_url.to_string()),
            notes: release.body,
            published_at: release
                .published_at
                .map(|dt| dt.with_timezone(&Utc).to_rfc3339()),
        };
        Ok(Some(entry))
    }
}

/// Result of an update check.
#[derive(Debug, Clone)]
pub struct UpdateResult {
    /// Current running version.
    pub current: VersionInfo,
    /// Latest available release metadata.
    pub latest: Option<UpdateEntry>,
}

impl UpdateResult {
    /// Determine if an update is available.
    #[must_use]
    pub fn update_available(&self) -> bool {
        match (&self.latest, Version::parse(&self.current.semver)) {
            (Some(latest), Ok(current_semver)) => match latest.semver() {
                Ok(latest_semver) => latest_semver > current_semver,
                Err(_) => false,
            },
            _ => false,
        }
    }
}

/// Representation of a release entry.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEntry {
    /// SemVer string for the release.
    pub version: String,
    #[serde(default)]
    /// Optional commit hash reference.
    pub commit: Option<String>,
    #[serde(default)]
    /// Optional download URL.
    pub download_url: Option<String>,
    #[serde(default)]
    /// Optional release notes.
    pub notes: Option<String>,
    #[serde(default)]
    /// Optional published timestamp.
    pub published_at: Option<String>,
    #[serde(default)]
    /// Optional signature verifying the release payload.
    pub signature: Option<String>,
}

impl UpdateEntry {
    /// Parse the version string into a [`semver::Version`].
    pub fn semver(&self) -> Result<Version> {
        Version::parse(&self.version)
            .map_err(|err| anyhow!("invalid semver {}: {}", self.version, err))
    }
}

#[derive(Debug, Deserialize)]
struct UpdateFeed {
    #[serde(default)]
    releases: Vec<UpdateEntry>,
}

/// Source selection helper.
#[derive(Debug, Clone, Copy)]
pub enum UpdateSource {
    /// Local JSON feed.
    Local,
    /// GitHub releases endpoint.
    GitHub,
}

/// Convenience helper that checks the current source preference.
#[must_use]
pub fn detect_source(settings: &UpdateSettings) -> UpdateSource {
    if settings.github_owner.is_some() && settings.github_repo.is_some() {
        UpdateSource::GitHub
    } else {
        UpdateSource::Local
    }
}

fn verify_release_signature(entry: &UpdateEntry) -> Result<()> {
    let signature = entry
        .signature
        .as_deref()
        .ok_or_else(|| anyhow!("release {} is unsigned", entry.version))?;
    let payload = release_message(entry);
    let bytes = general_purpose::STANDARD
        .decode(signature)
        .with_context(|| "release signature must be base64 encoded")?;
    let array: [u8; 64] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("invalid release signature length"))?;
    let signature = Signature::from_bytes(&array);
    let key = VerifyingKey::from_bytes(&RELEASE_PUBLIC_KEY)
        .map_err(|err| anyhow!("invalid release public key: {err}"))?;
    key.verify_strict(payload.as_bytes(), &signature)
        .map_err(|err| anyhow!("release signature verification failed: {err}"))?;
    Ok(())
}

fn release_message(entry: &UpdateEntry) -> String {
    format!(
        "{}|{}",
        entry.version,
        entry.commit.as_deref().unwrap_or("unknown")
    )
}
