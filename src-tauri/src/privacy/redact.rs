// Privacy/redaction primitives.
//
// In v2, the rules engine (`crate::rules`) is the brain — it decides what
// counts as PII based on whichever rule packs are loaded. This module just
// provides the *image-level* primitives the rules engine asks for once it
// has bounding boxes (from OCR, in phase 2).

use image::{ImageBuffer, Rgba, RgbaImage};

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
