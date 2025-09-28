//! ---
//! ems_section: "04-configuration-orchestration"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Configuration loading and orchestration helpers."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use r_ems_common::config::AppConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Default directory where active configuration state is stored.
pub const DEFAULT_CONFIG_ROOT: &str = "/etc/r-ems";
const INSTALLATIONS_DIR: &str = "installations";
const CURRENT_LINK: &str = "current.toml";

/// Metadata describing an installation manifest stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallationMetadata {
    /// Human-readable installation name supplied by the operator.
    pub name: String,
    /// Filesystem-safe slug generated from the installation name.
    pub slug: String,
    /// Timestamp (UTC) when the manifest was first created.
    pub created_at: DateTime<Utc>,
    /// Timestamp (UTC) when the manifest was last persisted.
    pub updated_at: DateTime<Utc>,
    /// SHA-256 hash of the effective [`AppConfig`] content.
    pub config_hash: String,
    /// Version of the tooling that produced the manifest.
    pub source_version: String,
}

/// Composite manifest that wraps [`AppConfig`] with installation metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationManifest {
    pub installation: InstallationMetadata,
    #[serde(flatten)]
    pub app: AppConfig,
}

/// Result of persisting an installation manifest to disk.
#[derive(Debug, Clone)]
pub struct PersistedInstallation {
    pub manifest: InstallationManifest,
    pub manifest_path: PathBuf,
    pub current_path: PathBuf,
}

/// Convenience container describing canonical configuration paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    pub root: PathBuf,
    pub installations_dir: PathBuf,
    pub current_symlink: PathBuf,
}

impl ConfigPaths {
    /// Construct a new [`ConfigPaths`] from an arbitrary root directory.
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        let root = root.as_ref().to_path_buf();
        let installations_dir = root.join(INSTALLATIONS_DIR);
        let current_symlink = root.join(CURRENT_LINK);
        Self {
            root,
            installations_dir,
            current_symlink,
        }
    }

    /// Ensure the installations directory exists.
    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.installations_dir).with_context(|| {
            format!(
                "unable to create installations directory {}",
                self.installations_dir.display()
            )
        })
    }
}

impl InstallationManifest {
    /// Construct a new manifest from a human-readable name and validated [`AppConfig`].
    pub fn new(name: impl Into<String>, app: AppConfig) -> Result<Self> {
        let name = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(anyhow!("installation name cannot be empty"));
        }
        let slug = slugify_name(&name);
        if slug.is_empty() {
            return Err(anyhow!(
                "installation name must contain at least one alphanumeric character"
            ));
        }
        let mut manifest = Self {
            installation: InstallationMetadata {
                name,
                slug,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                config_hash: String::new(),
                source_version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            app,
        };
        manifest.update_digest()?;
        Ok(manifest)
    }

    /// Return the filesystem-safe slug.
    pub fn slug(&self) -> &str {
        &self.installation.slug
    }

    /// Recompute the deterministic configuration hash and update timestamps.
    pub fn update_digest(&mut self) -> Result<()> {
        self.installation.config_hash = hash_app_config(&self.app)?;
        self.installation.updated_at = Utc::now();
        Ok(())
    }

    /// Persist the manifest under the provided root directory and refresh the `current.toml` symlink.
    pub fn persist(mut self, root: impl AsRef<Path>) -> Result<PersistedInstallation> {
        self.update_digest()?;
        let paths = ConfigPaths::new(root);
        paths.ensure_dirs()?;

        let filename = format!("{}.toml", self.installation.slug);
        let manifest_path = paths.installations_dir.join(filename);
        let serialized = toml::to_string_pretty(&self)
            .with_context(|| "failed to serialise installation manifest to TOML")?;
        fs::write(&manifest_path, serialized)
            .with_context(|| format!("unable to write manifest to {}", manifest_path.display()))?;

        create_symlink(&manifest_path, &paths.current_symlink)?;

        Ok(PersistedInstallation {
            manifest: self,
            manifest_path,
            current_path: paths.current_symlink,
        })
    }
}

impl PersistedInstallation {
    /// Convenience accessor for the manifest hash.
    pub fn config_hash(&self) -> &str {
        &self.manifest.installation.config_hash
    }
}

/// Persist an [`InstallationManifest`] and return the resulting paths.
pub fn persist_manifest(
    manifest: InstallationManifest,
    root: impl AsRef<Path>,
) -> Result<PersistedInstallation> {
    manifest.persist(root)
}

/// Load a manifest from a concrete path on disk.
pub fn load_manifest(path: impl AsRef<Path>) -> Result<InstallationManifest> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read manifest {}", path.display()))?;
    let manifest: InstallationManifest = toml::from_str(&raw)
        .with_context(|| format!("failed to parse manifest {}", path.display()))?;
    Ok(manifest)
}

/// Load the active manifest referenced by the `current.toml` symlink, if present.
pub fn load_active_manifest(root: impl AsRef<Path>) -> Result<Option<InstallationManifest>> {
    let paths = ConfigPaths::new(root);
    if !paths.current_symlink.exists() {
        return Ok(None);
    }
    let target = fs::read_link(&paths.current_symlink).unwrap_or(paths.current_symlink.clone());
    let manifest = load_manifest(&target).with_context(|| {
        format!(
            "unable to load manifest referenced by {}",
            paths.current_symlink.display()
        )
    })?;
    Ok(Some(manifest))
}

/// Compute the SHA-256 hash of a validated [`AppConfig`].
pub fn hash_app_config(config: &AppConfig) -> Result<String> {
    let serialised = toml::to_string(&config)
        .with_context(|| "failed to serialise configuration for hashing")?;
    let mut hasher = Sha256::new();
    hasher.update(serialised.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

/// Produce a filesystem-safe slug from a human-friendly installation name.
pub fn slugify_name(input: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if matches!(ch, ' ' | '-' | '_' | '.' | '/') && !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(link) {
        if meta.is_dir() {
            return Err(anyhow!(
                "expected symlink or file at {} but found directory",
                link.display()
            ));
        }
        fs::remove_file(link)
            .with_context(|| format!("unable to remove existing link {}", link.display()))?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).with_context(|| {
            format!(
                "unable to update symlink {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).with_context(|| {
            format!(
                "unable to update symlink {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}
