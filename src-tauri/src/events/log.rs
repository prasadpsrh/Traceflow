// Append-only NDJSON event log.
//
// One JSON object per line, durable, simple to grep, easy to stream.
// Writes are synchronous to disk (no buffering) so a crash never loses
// an event that was already acknowledged.

use crate::events::chain::ChainHasher;
use crate::events::event::{EventKind, EventRecord};
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

/// Thread-safe handle to a session's NDJSON log.
pub struct EventLog {
    path: PathBuf,
    file: Mutex<File>,
    chain: Mutex<ChainHasher>,
}

impl EventLog {
    /// Open a brand-new log file for a fresh session.
    pub fn create(path: &Path, session: Uuid) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .with_context(|| format!("creating {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            chain: Mutex::new(ChainHasher::new(session)),
        })
    }

    /// Re-open an existing log for appending, resuming the chain at its tail.
    pub fn open_for_append(path: &Path) -> Result<Self> {
        let records = read_all(path)?;
        let last = records
            .last()
            .ok_or_else(|| anyhow::anyhow!("log {} is empty", path.display()))?;
        let chain = ChainHasher::resume(last.session, last.hash.clone(), last.seq + 1);
        let file = OpenOptions::new()
            .append(true)
            .open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            chain: Mutex::new(chain),
        })
    }

    /// Append one event. Returns the persisted record.
    pub fn append(&self, body: EventKind) -> Result<EventRecord> {
        let mut chain = self.chain.lock().unwrap();
        let rec = chain.build(body)?;
        let json = serde_json::to_string(&rec).context("serializing event")?;
        let mut file = self.file.lock().unwrap();
        // Write the newline-terminated JSON, flush and sync to durable storage.
        writeln!(file, "{json}").context("writing event")?;
        file.flush().context("flushing event")?;
        file.sync_all()
            .with_context(|| format!("syncing {}", self.path.display()))?;
        Ok(rec)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Read every record in a log into memory. Suitable for projection (see
/// the document module) and for verification.
pub fn read_all(path: &Path) -> Result<Vec<EventRecord>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("reading line {i}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let rec: EventRecord =
            serde_json::from_str(&line).with_context(|| format!("parsing line {i}: {line}"))?;
        out.push(rec);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::chain::verify_chain;

    #[test]
    fn write_read_roundtrip_preserves_chain() {
        let path =
            std::env::temp_dir().join(format!("traceflow-test-{}.ndjson", uuid::Uuid::new_v4()));
        let session = uuid::Uuid::new_v4();
        let log = EventLog::create(&path, session).unwrap();
        log.append(EventKind::SessionStart {
            title: "test".into(),
            capture_settings: serde_json::json!({}),
            app_version: "0.2".into(),
            host_os: "test".into(),
        })
        .unwrap();
        log.append(EventKind::SessionEnd {
            reason: "test".into(),
        })
        .unwrap();
        drop(log);

        let recs = read_all(&path).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(verify_chain(recs.iter()).unwrap(), 2);
        std::fs::remove_file(&path).ok();
    }
}
