//! ---
//! ems_section: "03-persistence-logging"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Persistence abstractions and storage bindings."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::Deserialize;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::{PersistenceError, Result};

/// Default snapshot directory used when none is provided.
const DEFAULT_SNAPSHOT_PATH: &str = "/var/lib/r-ems/snapshots";

/// Persistence configuration parsed from TOML.
#[derive(Debug, Clone, Deserialize)]
pub struct PersistenceConfig {
    /// Snapshot configuration block.
    #[serde(default)]
    pub snapshot: SnapshotConfig,
    /// Event log configuration block.
    #[serde(default)]
    pub event_log: EventLogConfig,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            snapshot: SnapshotConfig::default(),
            event_log: EventLogConfig::default(),
        }
    }
}

/// Snapshot-specific configuration settings.
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotConfig {
    /// Snapshot target directory.
    #[serde(default = "SnapshotConfig::default_path")]
    pub path: PathBuf,
    /// Interval in seconds between scheduled snapshots.
    #[serde(default = "SnapshotConfig::default_interval")]
    pub interval_sec: u64,
    /// Number of days to retain snapshots.
    #[serde(default = "SnapshotConfig::default_retention")]
    pub retention_days: u64,
}

impl SnapshotConfig {
    fn default_path() -> PathBuf {
        PathBuf::from(DEFAULT_SNAPSHOT_PATH)
    }

    const fn default_interval() -> u64 {
        60
    }

    const fn default_retention() -> u64 {
        7
    }
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            path: Self::default_path(),
            interval_sec: Self::default_interval(),
            retention_days: Self::default_retention(),
        }
    }
}

/// Event log configuration settings.
#[derive(Debug, Clone, Deserialize)]
pub struct EventLogConfig {
    /// Directory containing event log files.
    #[serde(default = "EventLogConfig::default_path")]
    pub path: PathBuf,
    /// Maximum size (in MiB) before rotation is triggered.
    #[serde(default = "EventLogConfig::default_rotate")]
    pub rotate_mb: u64,
}

impl EventLogConfig {
    fn default_path() -> PathBuf {
        PathBuf::from("/var/lib/r-ems/logs")
    }

    const fn default_rotate() -> u64 {
        100
    }
}

impl Default for EventLogConfig {
    fn default() -> Self {
        Self {
            path: Self::default_path(),
            rotate_mb: Self::default_rotate(),
        }
    }
}

/// Supervisor responsible for retention and scheduled cleanup.
#[derive(Debug, Clone)]
pub struct PersistenceSupervisor {
    config: Arc<PersistenceConfig>,
}

impl PersistenceSupervisor {
    /// Parse configuration from a TOML file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let config: PersistenceConfig = toml::from_str(&raw)?;
        Ok(Self::new(config))
    }

    /// Build a supervisor from an existing configuration.
    pub fn new(config: PersistenceConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Spawn a background cleanup thread that runs at the provided interval.
    pub fn spawn_cleanup(self: &Arc<Self>, interval: Duration) -> thread::JoinHandle<()> {
        let supervisor = Arc::clone(self);
        thread::spawn(move || loop {
            if let Err(err) = supervisor.cleanup() {
                warn!(error = %err, "persistence cleanup failed");
            }
            thread::sleep(interval);
        })
    }

    /// Execute snapshot/log retention immediately.
    pub fn cleanup(&self) -> Result<()> {
        self.prune_directory(&self.config.snapshot.path, self.config.snapshot.retention_days)?;
        self.prune_directory(&self.config.event_log.path, self.config.snapshot.retention_days)?;
        Ok(())
    }

    fn prune_directory(&self, path: &Path, retention_days: u64) -> Result<()> {
        if retention_days == 0 {
            return Ok(());
        }
        let cutoff = Utc::now() - ChronoDuration::days(retention_days as i64);
        for entry in WalkDir::new(path).min_depth(1).into_iter().filter_map(|e| e.ok()) {
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(err) => {
                    warn!(path = %entry.path().display(), error = %err, "unable to read metadata");
                    continue;
                }
            };
            if metadata.is_dir() {
                continue;
            }
            if let Ok(modified) = metadata.modified() {
                let modified: DateTime<Utc> = modified.into();
                if modified < cutoff {
                    debug!(path = %entry.path().display(), "removing expired persistence artifact");
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

    /// Access the supervisor configuration.
    pub fn config(&self) -> &PersistenceConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cleanup_prunes_old_files() {
        let dir = tempdir().unwrap();
        let snapshots = dir.path().join("snapshots");
        let logs = dir.path().join("logs");
        std::fs::create_dir_all(&snapshots).unwrap();
        std::fs::create_dir_all(&logs).unwrap();

        let snapshot_file = snapshots.join("old.json");
        std::fs::write(&snapshot_file, b"{}").unwrap();
        let log_file = logs.join("old.log");
        std::fs::write(&log_file, b"entry").unwrap();

        // Backdate files by two days
        let two_days_ago = (Utc::now() - ChronoDuration::days(2)).into();
        filetime::set_file_mtime(&snapshot_file, filetime::FileTime::from_system_time(two_days_ago)).unwrap();
        filetime::set_file_mtime(&log_file, filetime::FileTime::from_system_time(two_days_ago)).unwrap();

        let config = PersistenceConfig {
            snapshot: SnapshotConfig {
                path: snapshots.clone(),
                interval_sec: 60,
                retention_days: 1,
            },
            event_log: EventLogConfig {
                path: logs.clone(),
                rotate_mb: 50,
            },
        };
        let supervisor = PersistenceSupervisor::new(config);
        supervisor.cleanup().unwrap();

        assert!(!snapshot_file.exists());
        assert!(!log_file.exists());
    }
}
