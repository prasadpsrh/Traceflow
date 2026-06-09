//! Cross-platform OCR provider using the `ocrs` crate.
//!
//! Used on macOS and Linux. On Windows, the native provider is preferred
//! because it's faster and doesn't require bundled models.
//!
//! The ONNX models are loaded once at construction and reused for every
//! frame — construction is expensive (~500ms), recognition is ~200–400ms
//! per frame depending on text density.

#![cfg(not(target_os = "windows"))]

use super::provider::{OcrProvider, OcrWord};
use anyhow::{Context, Result};
use image::RgbaImage;
use ocrs::{OcrEngine, OcrEngineParams};
use std::sync::Arc;

pub struct OcrsProvider {
    engine: Arc<OcrEngine>,
}

impl OcrsProvider {
    /// Create a new provider, loading the bundled ONNX models.
    /// This is expensive (~500ms) — call once at app startup, not per frame.
    pub fn new() -> Result<Self> {
        let engine = OcrEngine::new(OcrEngineParams::default())
            .context("loading ocrs ONNX models")?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }
}

impl OcrProvider for OcrsProvider {
    fn recognize(&self, image: &RgbaImage) -> Result<Vec<OcrWord>> {
        let (w, h) = image.dimensions();

        // ocrs expects an ImageSource. Convert from RgbaImage.
        let gray = image::imageops::grayscale(image);
        let img_source = ocrs::ImageSource::from_bytes(
            gray.as_raw(),
            ocrs::ImageDimensions {
                width: w,
                height: h,
                channels: 1,
            },
        )
        .context("creating ocrs image source")?;

        let ocr_input = self.engine.prepare_input(img_source)
            .context("preparing ocrs input")?;

        let word_rects = self.engine.detect_words(&ocr_input)
            .context("detecting words")?;

        let line_rects = self.engine.find_text_lines(&ocr_input, &word_rects);

        let line_texts = self.engine.recognize_text(&ocr_input, &line_rects)
            .context("recognizing text")?;

        let mut words = Vec::new();
        for line in line_texts {
            for word in line.words() {
                if let Some(rect) = word.rect() {
                    let text = word.text().to_string();
                    if text.trim().is_empty() {
                        continue;
                    }
                    words.push(OcrWord {
                        text,
                        x: rect.left().max(0) as u32,
                        y: rect.top().max(0) as u32,
                        w: (rect.right() - rect.left()).max(0) as u32,
                        h: (rect.bottom() - rect.top()).max(0) as u32,
                        confidence: 0.9, // ocrs doesn't expose per-word confidence yet
                    });
                }
            }
        }

        Ok(words)
    }

    fn name(&self) -> &'static str {
        "ocrs"
    }
}