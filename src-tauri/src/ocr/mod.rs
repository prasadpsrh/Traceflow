//! OCR subsystem — platform-aware text recognition.
//!
//! The public API is `create_provider()`, which returns a boxed
//! `OcrProvider` appropriate for the current platform.

pub mod engine;
pub mod noop_provider;
pub mod provider;

#[cfg(not(target_os = "windows"))]
pub mod ocrs_provider;

#[cfg(target_os = "windows")]
pub mod windows_provider;

pub use provider::OcrProvider;

use anyhow::Result;

/// Create the best available OCR provider for this platform.
///
/// Call once at session start and reuse — provider construction
/// may be expensive (model loading).
pub fn create_provider() -> Result<Box<dyn OcrProvider>> {
    #[cfg(target_os = "windows")]
    {
        match windows_provider::WindowsOcrProvider::new() {
            Ok(p) => {
                tracing::info!("OCR provider: windows_media_ocr");
                return Ok(Box::new(p));
            }
            Err(e) => {
                tracing::warn!("Windows OCR unavailable ({e}), falling back to noop");
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        match ocrs_provider::OcrsProvider::new() {
            Ok(p) => {
                tracing::info!("OCR provider: ocrs");
                return Ok(Box::new(p));
            }
            Err(e) => {
                tracing::warn!("ocrs initialization failed ({e}), falling back to noop");
            }
        }
    }

    tracing::info!("OCR provider: noop (no text recognition)");
    Ok(Box::new(noop_provider::NoopOcrProvider))
}
