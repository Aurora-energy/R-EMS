//! ---
//! ems_section: "01-core-functionality"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Primary orchestration and lifecycle management."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::debug;
use walkdir::WalkDir;

use crate::integration_persistence::{persist_snapshot, restore_snapshot};
use r_ems_common::config::SnapshotConfig;

/// Persisted controller state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerSnapshot {
    pub grid_id: String,
    pub controller_id: String,
    pub captured_at: DateTime<Utc>,
    pub payload: Value,
}

impl ControllerSnapshot {
    pub fn file_name(&self) -> String {
        format!(
            "{}-{}.json",
            self.captured_at.timestamp(),
            self.controller_id
        )
    }
}

/// Snapshot storage helper.
#[derive(Debug, Clone)]
pub struct SnapshotStore {
    root: PathBuf,
    retain_last: usize,
}

impl SnapshotStore {
    pub fn from_config(config: &SnapshotConfig, grid_id: &str) -> Result<Self> {
        let mut root = config.directory.clone();
        root.push(grid_id);
        fs::create_dir_all(&root)
            .with_context(|| format!("unable to create snapshot directory {}", root.display()))?;
        Ok(Self {
            root,
            retain_last: config.retain_last.max(1),
        })
    }

    pub fn write(&self, snapshot: &ControllerSnapshot) -> Result<PathBuf> {
        let mut dir = self.root.clone();
        dir.push(&snapshot.controller_id);
        fs::create_dir_all(&dir).with_context(|| {
            format!("unable to create controller snapshot dir {}", dir.display())
        })?;
        let mut path = dir.clone();
        path.push(snapshot.file_name());
        persist_snapshot(snapshot, &path)
            .with_context(|| format!("failed to persist snapshot {}", path.display()))?;
        self.prune_old(&dir)?;
        debug!(
            grid = %snapshot.grid_id,
            controller = %snapshot.controller_id,
            path = %path.display(),
            "snapshot persisted"
        );
        Ok(path)
    }

    pub fn load_latest(
        &self,
        grid_id: &str,
        controller_id: &str,
    ) -> Result<Option<(ControllerSnapshot, PathBuf)>> {
        let mut dir = self.root.clone();
        dir.push(controller_id);
        if !dir.exists() {
            return Ok(None);
        }
        let mut entries: Vec<PathBuf> = WalkDir::new(&dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.into_path())
            .collect();
        entries.sort();
        let Some(latest) = entries.into_iter().last() else {
            return Ok(None);
        };
        let snapshot = restore_snapshot(&latest)
            .with_context(|| format!("failed to restore snapshot {}", latest.display()))?;
        if snapshot.grid_id != grid_id {
            return Ok(None);
        }
        Ok(Some((snapshot, latest)))
    }

    /// Return the root directory where snapshots for this grid are stored.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn prune_old(&self, dir: &Path) -> Result<()> {
        let mut entries: Vec<PathBuf> = WalkDir::new(dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.into_path())
            .collect();
        entries.sort();
        if entries.len() <= self.retain_last {
            return Ok(());
        }
        let excess = entries.len() - self.retain_last;
        for path in entries.into_iter().take(excess) {
            if let Err(err) = fs::remove_file(&path) {
                tracing::warn!(path = %path.display(), error = %err, "failed pruning snapshot");
            }
        }
        Ok(())
    }
}
