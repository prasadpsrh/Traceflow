// On-device OCR.
//
// Windows: delegates to Windows.Media.Ocr — zero additional model downloads,
// built into every Windows 10/11 installation, returns word-level bounding
// boxes and text in the display language.
//
// Other platforms: stubbed (returns empty regions) pending Phase 3 work.

pub mod engine;
pub use engine::run_ocr_with_text;
