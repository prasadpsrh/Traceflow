// Privacy/redaction primitives.
//
// `apply_redactions` is the high-level entry point: given OCR word regions
// and a compiled rule engine, it evaluates every word's text, applies
// pixel-level redactions for Blur/BlackBox hits, and returns a summary of
// what was redacted (for the RedactionApplied event log).

use crate::events::event::OcrRegion;
use crate::rules::engine::{RuleAction, RuleEngine};
use image::{ImageBuffer, Rgba, RgbaImage};

/// Evaluate every OCR word against the rule engine.
/// For Blur/BlackBox hits the pixel region is modified in-place.
/// Returns `(rule_name, text_hash)` for every hit — used to build
/// `RedactionApplied` events.
pub fn apply_redactions(
    frame: &mut RgbaImage,
    text_regions: &[(String, OcrRegion)],
    engine: &RuleEngine,
) -> Vec<(String, String)> {
    let mut applied = Vec::new();
    for (text, region) in text_regions {
        for hit in engine.evaluate(text) {
            match &hit.action {
                RuleAction::Blur => blur_region(frame, region.x, region.y, region.w, region.h),
                RuleAction::BlackBox => black_box(frame, region.x, region.y, region.w, region.h),
                // Text-level or flag-only actions: no pixel change, still recorded.
                RuleAction::Mask { .. } | RuleAction::Drop | RuleAction::Flag => {}
            }
            applied.push((hit.rule_name.clone(), region.text_hash.clone()));
        }
    }
    applied
}

/// Blur a rectangular region of a frame (used when a rule's action is `Blur`).
pub fn blur_region(frame: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32) {
    if w == 0 || h == 0 {
        return;
    }
    let (fw, fh) = frame.dimensions();
    let x2 = (x + w).min(fw);
    let y2 = (y + h).min(fh);
    let xx = x.min(fw);
    let yy = y.min(fh);
    if x2 <= xx || y2 <= yy {
        return;
    }
    let region_w = x2 - xx;
    let region_h = y2 - yy;
    let sub = imageproc::filter::gaussian_blur_f32(
        &ImageBuffer::from_fn(region_w, region_h, |dx, dy| {
            *frame.get_pixel(xx + dx, yy + dy)
        }),
        12.0,
    );
    for (dx, dy, p) in sub.enumerate_pixels() {
        frame.put_pixel(xx + dx, yy + dy, *p);
    }
}

/// Solid-fill black box (highest-sensitivity redaction).
pub fn black_box(frame: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32) {
    let (fw, fh) = frame.dimensions();
    for yy in y..(y + h).min(fh) {
        for xx in x..(x + w).min(fw) {
            frame.put_pixel(xx, yy, Rgba([0, 0, 0, 255]));
        }
    }
}
