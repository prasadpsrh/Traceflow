//! OCR provider trait — abstracts platform-specific OCR implementations.
//!
//! The capture engine calls `OcrProvider::recognize()` on each promoted frame.
//! The result is a list of recognized words with bounding boxes. The words
//! are evaluated by the rule engine for PII detection, then DROPPED — they
//! must never be persisted to disk (§2.5 of ENGINEERING_STANDARDS.md).

use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// A single recognized word with its bounding box in the original image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrWord {
    pub text: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Confidence score, 0.0–1.0. Not all providers supply this.
    pub confidence: f32,
}

/// The platform-agnostic OCR interface.
///
/// Implementations must be `Send + Sync` because the capture engine
/// runs OCR inside `spawn_blocking` from an async context.
pub trait OcrProvider: Send + Sync {
    /// Recognize text in the given RGBA image.
    /// Returns an empty vec if no text is found (never errors on "no text").
    /// Errors only on infrastructure failures (model load, memory, etc.).
    fn recognize(&self, image: &RgbaImage) -> anyhow::Result<Vec<OcrWord>>;

    /// Human-readable name for logging ("windows_media_ocr", "ocrs", "noop").
    fn name(&self) -> &'static str;
}
