//! Time Machine replay — point-in-time session reconstruction.
//!
//! Builds a timeline index from the event log, enabling O(1) lookup of
//! the frame and events at any timestamp. The index is ephemeral — it's
//! computed on demand from the log, never persisted.

pub mod timeline;

pub use timeline::{TimelineIndex, TimelineView};
