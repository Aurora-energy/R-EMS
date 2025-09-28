//! ---
//! ems_section: "06-security-access-control"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Security policies, identity, and cryptographic utilities."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Entry recorded in the audit log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditEntry {
    /// Timestamp when the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Actor responsible for the event.
    pub actor: String,
    /// Event type (e.g. `config.reload`, `command.execute`).
    pub action: String,
    /// Additional context serialized as JSON.
    pub metadata: serde_json::Value,
    /// SHA-256 hash of the entry contents and previous hash.
    pub hash: String,
    /// Hash of the previous entry (or zero string for the first entry).
    pub previous_hash: String,
}

impl AuditEntry {
    fn compute_hash(
        timestamp: DateTime<Utc>,
        actor: &str,
        action: &str,
        metadata: &serde_json::Value,
        previous_hash: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(
            timestamp
                .timestamp_nanos_opt()
                .unwrap_or_default()
                .to_be_bytes(),
        );
        hasher.update(actor.as_bytes());
        hasher.update(action.as_bytes());
        hasher.update(metadata.to_string().as_bytes());
        hasher.update(previous_hash.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Audit log backed by a newline-delimited JSON file.
#[derive(Debug, Clone)]
pub struct AuditLog {
    path: PathBuf,
    last_hash: String,
}

impl AuditLog {
    /// Create an audit log at the given path. Existing entries are loaded to determine the head hash.
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut log = Self {
            path: path.clone(),
            last_hash: "0".repeat(64),
        };
        if path.exists() {
            for line in BufReader::new(fs::File::open(&path)?).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let entry: AuditEntry = serde_json::from_str(&line)?;
                log.last_hash = entry.hash.clone();
            }
        }
        Ok(log)
    }

    /// Append a new audit entry to the log.
    pub fn append(
        &mut self,
        actor: &str,
        action: &str,
        metadata: serde_json::Value,
    ) -> Result<AuditEntry> {
        let timestamp = Utc::now();
        let hash = AuditEntry::compute_hash(timestamp, actor, action, &metadata, &self.last_hash);
        let entry = AuditEntry {
            timestamp,
            actor: actor.to_string(),
            action: action.to_string(),
            metadata,
            hash: hash.clone(),
            previous_hash: self.last_hash.clone(),
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("unable to open audit log {}", self.path.display()))?;
        file.write_all(serde_json::to_string(&entry)?.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        self.last_hash = hash;
        Ok(entry)
    }

    /// Verify integrity of the log (detect tampering).
    pub fn verify(&self) -> Result<bool> {
        let mut previous = "0".repeat(64);
        if !self.path.exists() {
            return Ok(true);
        }
        for line in BufReader::new(fs::File::open(&self.path)?).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: AuditEntry = serde_json::from_str(&line)?;
            let expected = AuditEntry::compute_hash(
                entry.timestamp,
                &entry.actor,
                &entry.action,
                &entry.metadata,
                &previous,
            );
            if expected != entry.hash {
                return Ok(false);
            }
            previous = entry.hash;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};
    use tempfile::tempdir;

    #[test]
    fn audit_log_detects_tampering() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("audit.log");
        let mut log = AuditLog::new(&path).unwrap();
        log.append(
            "alice",
            "config.reload",
            serde_json::json!({"status": "ok"}),
        )
        .unwrap();
        log.append(
            "bob",
            "command.execute",
            serde_json::json!({"command": "restart"}),
        )
        .unwrap();
        assert!(log.verify().unwrap());

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let mut entries: Vec<serde_json::Value> = contents
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        entries[1]["metadata"]["command"] = serde_json::json!("shutdown");
        file.set_len(0).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        for value in entries {
            file.write_all(value.to_string().as_bytes()).unwrap();
            file.write_all(b"\n").unwrap();
        }
        assert!(!AuditLog::new(&path).unwrap().verify().unwrap());
    }
}
