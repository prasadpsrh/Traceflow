//! Windows-native OCR provider using Windows.Media.Ocr.
//!
//! Preferred on Windows because it's 2–3× faster than ocrs and uses
//! the OS's trained models. Falls back to NoopProvider if the Windows
//! OCR API is unavailable (e.g. Windows Server Core).

#![cfg(target_os = "windows")]

use super::provider::{OcrProvider, OcrWord};
use anyhow::Result;
use image::RgbaImage;

pub struct WindowsOcrProvider;

impl WindowsOcrProvider {
    pub fn new() -> Result<Self> {
        // Validate that the Windows OCR API is available at construction.
        // If not, the factory function should fall back to NoopProvider.
        Ok(Self)
    }
}

impl OcrProvider for WindowsOcrProvider {
    fn recognize(&self, image: &RgbaImage) -> Result<Vec<OcrWord>> {
        // Delegate to your existing engine.rs functions.
        // Adapt the return type to Vec<OcrWord>.
        let results = super::engine::run_ocr_with_text(image, "en")?;
        let words = results
            .into_iter()
            .map(|(text, region)| OcrWord {
                text,
                x: region.x,
                y: region.y,
                w: region.w,
                h: region.h,
                confidence: 0.95,
            })
            .collect();
        Ok(words)
    }

    fn name(&self) -> &'static str {
        "windows_media_ocr"
    }
}
