// Perceptual pixel diff.
//
// We downscale every captured frame to a small grayscale thumbnail
// (320x180 by default) and compute the mean absolute difference vs
// the previous reference frame. A normalized value in [0.0, 1.0] is
// returned, where 0 = identical and 1 = maximally different.
//
// This is dramatically cheaper than diffing full-resolution frames
// and is robust to tiny anti-aliasing changes, sub-pixel cursor jitter,
// and clock updates.

use image::{imageops::FilterType, GrayImage, RgbaImage};

/// Target thumbnail size for diffing. 320x180 = 57.6k pixels.
pub const THUMB_W: u32 = 320;
pub const THUMB_H: u32 = 180;

/// Reduce an RGBA frame to a small grayscale thumbnail suitable for diffing.
pub fn fingerprint(frame: &RgbaImage) -> GrayImage {
    let resized = image::imageops::resize(frame, THUMB_W, THUMB_H, FilterType::Triangle);
    let mut gray = GrayImage::new(THUMB_W, THUMB_H);
    for (x, y, p) in resized.enumerate_pixels() {
        // Standard luma coefficients
        let [r, g, b, _a] = p.0;
        let luma = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u8;
        gray.put_pixel(x, y, image::Luma([luma]));
    }
    gray
}

/// Mean absolute difference, normalized to [0.0, 1.0].
/// Returns 0.0 if both fingerprints are identical, 1.0 if maximally different.
pub fn diff_score(a: &GrayImage, b: &GrayImage) -> f32 {
    debug_assert_eq!(a.dimensions(), b.dimensions());
    let n = (a.width() * a.height()) as u64;
    let mut sum: u64 = 0;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        let d = (pa[0] as i32 - pb[0] as i32).unsigned_abs() as u64;
        sum += d;
    }
    (sum as f32) / (n as f32 * 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn solid_frame(r: u8, g: u8, b: u8) -> RgbaImage {
        ImageBuffer::from_pixel(640, 360, Rgba([r, g, b, 255]))
    }

    #[test]
    fn identical_frames_score_zero() {
        let f1 = solid_frame(128, 128, 128);
        let f2 = solid_frame(128, 128, 128);
        assert!((diff_score(&fingerprint(&f1), &fingerprint(&f2))).abs() < 0.001);
    }

    #[test]
    fn different_frames_score_high() {
        let f1 = solid_frame(0, 0, 0);
        let f2 = solid_frame(255, 255, 255);
        let s = diff_score(&fingerprint(&f1), &fingerprint(&f2));
        assert!(s > 0.9, "expected near 1.0, got {s}");
    }
}
