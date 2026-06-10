//! No-op OCR provider — always returns an empty word list.
//!
//! Used as a fallback when no real OCR provider is available (e.g.
//! Windows Server Core without language packs, or a build with OCR
//! disabled). Also useful for benchmarking the capture pipeline
//! without OCR overhead.

use super::provider::{OcrProvider, OcrWord};
use anyhow::Result;
use image::RgbaImage;

pub struct NoopOcrProvider;

impl OcrProvider for NoopOcrProvider {
    fn recognize(&self, _image: &RgbaImage) -> Result<Vec<OcrWord>> {
        Ok(Vec::new())
    }

    fn name(&self) -> &'static str {
        "noop"
    }
}
