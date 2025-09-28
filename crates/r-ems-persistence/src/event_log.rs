//! ---
//! ems_section: "03-persistence-logging"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Persistence abstractions and storage bindings."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{snapshot::SNAPSHOT_VERSION, PersistenceError, Result};
use sha2::Digest;

/// Event log file header stored as the first line in the log.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventLogHeader {
    version: u16,
    created_at: DateTime<Utc>,
    hash: String,
}

impl EventLogHeader {
    fn new() -> Self {
        let created_at = Utc::now();
        let hash = format!(
            "{:x}",
            sha2::Sha256::digest(created_at.to_rfc3339().as_bytes())
        );
        Self {
            version: SNAPSHOT_VERSION,
            created_at,
            hash,
        }
    }
}

/// Event payload captured in the log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventLogEntry {
    /// Sequential identifier assigned when appending.
    pub sequence: u64,
    /// Timestamp when the event was recorded.
    pub timestamp: DateTime<Utc>,
    /// Arbitrary JSON payload (command, telemetry record, etc.).
    pub payload: serde_json::Value,
}

impl EventLogEntry {
    /// Construct an entry with the provided payload.
    pub fn new(payload: serde_json::Value) -> Self {
        Self {
            sequence: 0,
            timestamp: Utc::now(),
            payload,
        }
    }
}

/// Append-only writer for the event log.
pub struct EventLogWriter {
    path: std::path::PathBuf,
    writer: BufWriter<File>,
    next_sequence: u64,
}

impl EventLogWriter {
    /// Open an event log for appending, writing a header if the file is new.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let exists = path.exists();
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let mut writer = BufWriter::new(file);

        if !exists || is_empty(path)? {
            let header = EventLogHeader::new();
            let line = serde_json::to_string(&header)?;
            writer.write_all(line.as_bytes())?;
            writer.write_all(b"\n")?;
            writer.flush()?;
            return Ok(Self {
                path: path.to_path_buf(),
                writer,
                next_sequence: 0,
            });
        }

        let next_sequence = determine_next_sequence(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            writer,
            next_sequence,
        })
    }

    /// Append a new entry to the log and return the assigned sequence number and byte count.
    pub fn append(&mut self, mut entry: EventLogEntry) -> Result<(u64, usize)> {
        self.next_sequence += 1;
        entry.sequence = self.next_sequence;
        let line = serde_json::to_string(&entry)?;
        let bytes = line.len() + 1; // newline delimiter
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok((entry.sequence, bytes))
    }

    /// Flush buffered writes to the underlying file handle.
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    /// Access the current path on disk (useful for tests).
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn is_empty(path: &Path) -> Result<bool> {
    Ok(fs::metadata(path)?.len() == 0)
}

fn determine_next_sequence(path: &Path) -> Result<u64> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut last_seq = 0u64;
    for line in reader.lines().skip(1) {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<EventLogEntry>(&line) {
            last_seq = entry.sequence;
        }
    }
    Ok(last_seq)
}

/// Replay the log in order, invoking the callback for each entry.
pub fn replay<F>(path: &Path, mut handler: F) -> Result<usize>
where
    F: FnMut(EventLogEntry) -> Result<()>,
{
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut count = 0usize;
    for line in reader.lines().skip(1) {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: EventLogEntry = serde_json::from_str(&line)?;
        handler(entry)?;
        count += 1;
    }
    Ok(count)
}

/// Expose a streaming iterator over the log entries.
pub struct EventLogReader {
    lines: std::io::Lines<BufReader<File>>,
}

impl EventLogReader {
    /// Open the log for sequential reading.
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut first_line = String::new();
        reader.read_line(&mut first_line)?; // discard header
        Ok(Self {
            lines: reader.lines(),
        })
    }
}

impl Iterator for EventLogReader {
    type Item = Result<EventLogEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.lines.next()? {
            Ok(line) if line.trim().is_empty() => self.next(),
            Ok(line) => Some(serde_json::from_str(&line).map_err(PersistenceError::from)),
            Err(err) => Some(Err(err.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn append_and_replay_events() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("event.log");
        let mut writer = EventLogWriter::open(&path).unwrap();

        let _ = writer
            .append(EventLogEntry::new(json!({"cmd": "start"})))
            .unwrap();
        let _ = writer
            .append(EventLogEntry::new(json!({"cmd": "stop"})))
            .unwrap();

        let mut events = Vec::new();
        replay(&path, |entry| {
            events.push(entry.payload.clone());
            Ok(())
        })
        .unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["cmd"], json!("start"));
        assert_eq!(events[1]["cmd"], json!("stop"));
    }

    #[test]
    fn reader_iterates_in_order() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.log");
        let mut writer = EventLogWriter::open(&path).unwrap();
        let _ = writer
            .append(EventLogEntry::new(json!({"tick": 1})))
            .unwrap();
        let _ = writer
            .append(EventLogEntry::new(json!({"tick": 2})))
            .unwrap();

        let reader = EventLogReader::open(&path).unwrap();
        let sequences: Vec<_> = reader.map(|entry| entry.unwrap().sequence).collect();
        assert_eq!(sequences, vec![1, 2]);
    }
}
