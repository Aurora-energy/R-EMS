//! ---
//! ems_section: "03-persistence-logging"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Persistence abstractions and storage bindings."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{PersistenceError, Result};

/// Current snapshot envelope version.
pub const SNAPSHOT_VERSION: u16 = 1;

/// Controller state persisted in a snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControllerState {
    /// Identifier of the grid the controller belongs to.
    pub grid_id: String,
    /// Controller identifier.
    pub controller_id: String,
    /// Arbitrary JSON metadata capturing controller runtime state.
    #[serde(default)]
    pub state: serde_json::Value,
    /// Timestamp when the state was captured.
    pub captured_at: DateTime<Utc>,
}

impl ControllerState {
    /// Construct a controller state record from raw components.
    pub fn new(
        grid_id: impl Into<String>,
        controller_id: impl Into<String>,
        state: serde_json::Value,
    ) -> Self {
        Self {
            grid_id: grid_id.into(),
            controller_id: controller_id.into(),
            state,
            captured_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotEnvelope {
    version: u16,
    created_at: DateTime<Utc>,
    hash: String,
    state: ControllerState,
}

/// Persist a controller snapshot to the provided filesystem path.
///
/// The serializer is selected based on file extension: `.cbor` writes CBOR,
/// all other extensions default to JSON.
pub fn save_snapshot(state: &ControllerState, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let mut envelope = SnapshotEnvelope {
        version: SNAPSHOT_VERSION,
        created_at: Utc::now(),
        hash: String::new(),
        state: state.clone(),
    };

    envelope.hash = compute_hash(&envelope.state)?;

    let mut writer = BufWriter::new(File::create(path)?);
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("cbor") => {
            let bytes = serde_cbor::to_vec(&envelope).map_err(PersistenceError::from)?;
            writer.write_all(&bytes)?;
        }
        _ => {
            let json = serde_json::to_vec_pretty(&envelope)?;
            writer.write_all(&json)?;
        }
    }
    writer.flush()?;
    Ok(())
}

/// Load a snapshot from disk and return the contained controller state.
pub fn load_snapshot(path: &Path) -> Result<ControllerState> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    let envelope: SnapshotEnvelope = match path.extension().and_then(|ext| ext.to_str()) {
        Some("cbor") => serde_cbor::from_slice(&bytes).map_err(PersistenceError::from)?,
        _ => serde_json::from_slice(&bytes)?,
    };

    let expected = compute_hash(&envelope.state)?;
    if envelope.hash != expected {
        return Err(PersistenceError::HashMismatch);
    }

    Ok(envelope.state)
}

/// Verify the integrity of a snapshot without loading the payload.
pub fn verify_snapshot(path: &Path) -> bool {
    match load_envelope(path) {
        Ok(envelope) => compute_hash(&envelope.state)
            .map(|hash| hash == envelope.hash)
            .unwrap_or(false),
        Err(_) => false,
    }
}

fn load_envelope(path: &Path) -> Result<SnapshotEnvelope> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let envelope = match path.extension().and_then(|ext| ext.to_str()) {
        Some("cbor") => serde_cbor::from_slice(&bytes).map_err(PersistenceError::from)?,
        _ => serde_json::from_slice(&bytes)?,
    };
    Ok(envelope)
}

fn compute_hash(state: &ControllerState) -> Result<String> {
    let serialized = serde_json::to_vec(state)?;
    let mut hasher = Sha256::new();
    hasher.update(serialized);
    let digest = hasher.finalize();
    Ok(hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_and_load_json_snapshot() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snapshot.json");
        let state = ControllerState::new("grid-a", "ctrl-1", serde_json::json!({"voltage": 416.2}));

        save_snapshot(&state, &path).unwrap();
        assert!(verify_snapshot(&path));

        let loaded = load_snapshot(&path).unwrap();
        assert_eq!(loaded.grid_id, state.grid_id);
        assert_eq!(loaded.controller_id, state.controller_id);
        assert_eq!(loaded.state, state.state);
    }

    #[test]
    fn save_and_load_cbor_snapshot() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snapshot.cbor");
        let state = ControllerState::new("grid-b", "ctrl-2", serde_json::json!({"current": 12.4}));

        save_snapshot(&state, &path).unwrap();
        assert!(verify_snapshot(&path));

        let loaded = load_snapshot(&path).unwrap();
        assert_eq!(loaded.state, state.state);
    }

    #[test]
    fn verify_rejects_tampered_snapshot() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snapshot.json");
        let state = ControllerState::new("grid-x", "ctrl-x", serde_json::json!({"value": 1}));

        save_snapshot(&state, &path).unwrap();

        // Tamper with the file by editing the payload.
        let mut envelope: serde_json::Value =
            serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        envelope["state"]["state"]["value"] = serde_json::json!(999);
        fs::write(&path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();

        assert!(!verify_snapshot(&path));
        assert!(load_snapshot(&path).is_err());
    }
}
