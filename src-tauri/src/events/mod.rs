// Event-sourced backbone.
//
// Every observable thing during a capture session — frames promoted to steps,
// window focus changes, mouse clicks, redactions, AI descriptions — is appended
// to a tamper-evident log. The log is the source of truth. Everything else
// (Word docs, PDFs, audit reports, knowledge graphs) is a projection.
//
// Three files:
//   - event.rs  — the Event enum and metadata
//   - log.rs    — append-only NDJSON writer + reader
//   - chain.rs  — hash chain: each event references the SHA-256 of the previous

pub mod chain;
pub mod event;
pub mod log;

pub use chain::ChainHasher;
pub use event::{Event, EventKind, EventRecord};
pub use log::EventLog;
