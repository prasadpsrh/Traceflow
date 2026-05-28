// Hash chain.
//
// Every event in the log carries `prev` (hash of the previous record) and
// `hash` (hash of THIS record's body, including `prev`). Any tampering breaks
// the chain and is detectable by a single linear scan.
//
// We canonicalize the body with serde_json's `to_writer` against a sorted map
// so the same event always hashes to the same value regardless of field order.

use crate::events::event::{EventKind, EventRecord};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Zero hash used as `prev` for the very first record.
pub const ZERO_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Builds chained records given a running cursor (prev_hash + next_seq).
pub struct ChainHasher {
    pub prev_hash: String,
    pub next_seq: u64,
    pub session: Uuid,
}

impl ChainHasher {
    pub fn new(session: Uuid) -> Self {
        Self {
            prev_hash: ZERO_HASH.to_string(),
            next_seq: 0,
            session,
        }
    }

    /// Resume a chain from an existing record (e.g. after reopening a session).
    pub fn resume(session: Uuid, prev_hash: String, next_seq: u64) -> Self {
        Self {
            prev_hash,
            next_seq,
            session,
        }
    }

    /// Build the next record from an event body. Advances the cursor.
    pub fn build(&mut self, body: EventKind) -> Result<EventRecord> {
        let at: DateTime<Utc> = Utc::now();
        // First fill `hash` with a placeholder, then canonicalize, then compute hash.
        let placeholder = EventRecord {
            seq: self.next_seq,
            at,
            session: self.session,
            prev: self.prev_hash.clone(),
            hash: String::new(),
            body,
        };
        let canonical = canonical_bytes_excluding_hash(&placeholder)
            .context("canonicalizing record")?;
        let mut h = Sha256::new();
        h.update(&canonical);
        let digest = hex::encode(h.finalize());

        let record = EventRecord {
            hash: digest.clone(),
            ..placeholder
        };

        self.prev_hash = digest;
        self.next_seq += 1;
        Ok(record)
    }
}

/// Verify a chain: returns Ok(count) if the chain is intact,
/// Err with the first bad seq number otherwise.
pub fn verify_chain<'a, I: IntoIterator<Item = &'a EventRecord>>(records: I) -> Result<u64> {
    let mut expected_prev = ZERO_HASH.to_string();
    let mut expected_seq: u64 = 0;
    let mut count = 0u64;
    for r in records {
        if r.seq != expected_seq {
            anyhow::bail!("sequence gap at seq={} (expected {})", r.seq, expected_seq);
        }
        if r.prev != expected_prev {
            anyhow::bail!("broken prev link at seq={}", r.seq);
        }
        let canonical = canonical_bytes_excluding_hash(r)?;
        let mut h = Sha256::new();
        h.update(&canonical);
        let digest = hex::encode(h.finalize());
        if digest != r.hash {
            anyhow::bail!("hash mismatch at seq={}", r.seq);
        }
        expected_prev = r.hash.clone();
        expected_seq += 1;
        count += 1;
    }
    Ok(count)
}

/// Serialize the record to canonical JSON with `hash` set to the empty string.
/// The empty `hash` field is included so the canonical form for verification
/// matches the canonical form used at construction.
fn canonical_bytes_excluding_hash(r: &EventRecord) -> Result<Vec<u8>> {
    // Strategy: clone with empty hash, then serde_json into sorted-key BTreeMap value.
    let cleaned = EventRecord {
        hash: String::new(),
        ..r.clone()
    };
    let v = serde_json::to_value(&cleaned)?;
    let canonical = canonicalize_value(v);
    Ok(canonical.to_string().into_bytes())
}

/// Recursively sort object keys so the resulting JSON is byte-stable.
fn canonicalize_value(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Object(map) => {
            let mut sorted = std::collections::BTreeMap::new();
            for (k, val) in map {
                sorted.insert(k, canonicalize_value(val));
            }
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(canonicalize_value).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::event::EventKind;

    #[test]
    fn empty_chain_verifies() {
        let recs: Vec<EventRecord> = vec![];
        assert_eq!(verify_chain(recs.iter()).unwrap(), 0);
    }

    #[test]
    fn three_event_chain_verifies() {
        let session = Uuid::new_v4();
        let mut h = ChainHasher::new(session);
        let r1 = h
            .build(EventKind::SessionStart {
                title: "t".into(),
                capture_settings: serde_json::json!({}),
                app_version: "0.2".into(),
                host_os: "linux".into(),
            })
            .unwrap();
        let r2 = h
            .build(EventKind::StepPromoted {
                step_index: 0,
                frame_hash: "abc".into(),
                width: 100,
                height: 100,
                window_title: None,
                window_class: None,
                app_name: None,
            })
            .unwrap();
        let r3 = h
            .build(EventKind::SessionEnd {
                reason: "user".into(),
            })
            .unwrap();
        let recs = vec![r1, r2, r3];
        assert_eq!(verify_chain(recs.iter()).unwrap(), 3);
    }

    #[test]
    fn tampered_chain_fails() {
        let session = Uuid::new_v4();
        let mut h = ChainHasher::new(session);
        let r1 = h
            .build(EventKind::SessionStart {
                title: "t".into(),
                capture_settings: serde_json::json!({}),
                app_version: "0.2".into(),
                host_os: "linux".into(),
            })
            .unwrap();
        let mut r2 = h
            .build(EventKind::SessionEnd {
                reason: "user".into(),
            })
            .unwrap();
        // Tamper with the body without recomputing hash.
        r2.body = EventKind::SessionEnd {
            reason: "tampered".into(),
        };
        assert!(verify_chain([r1, r2].iter()).is_err());
    }
}
