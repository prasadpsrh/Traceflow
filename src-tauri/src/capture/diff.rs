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

/// Minimum per-pixel luma delta to count a pixel as "changed".
/// Filters sub-pixel rendering noise, JPEG artifacts, and clock-digit flips
/// while catching real content changes.
const CHANGED_PIXEL_MIN: u64 = 10;

/// Weight applied to the changed-pixel fraction before combining with MAD.
/// At 0.7 a frame where ≥6 % of pixels shift by ≥10 luma units scores ≥ 0.042,
/// which clears the default 0.04 threshold even when the average delta is small
/// (e.g. dark-theme UIs where most pixels stay near zero).
const CPC_WEIGHT: f32 = 0.7;

/// Hybrid diff score, normalised to [0.0, 1.0].
///
/// Combines:
///   • MAD  – mean absolute luma difference (good for high-contrast or large changes)
///   • CPC  – changed-pixel count fraction × CPC_WEIGHT (good for sparse changes on
///            dark-themed UIs where only a small region updates)
///
/// The `max` of the two is returned so either signal alone can trigger capture.
pub fn diff_score(a: &GrayImage, b: &GrayImage) -> f32 {
    debug_assert_eq!(a.dimensions(), b.dimensions());
    let n = (a.width() * a.height()) as u64;
    let mut sum: u64 = 0;
    let mut changed: u64 = 0;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        let d = (pa[0] as i32 - pb[0] as i32).unsigned_abs() as u64;
        sum += d;
        if d >= CHANGED_PIXEL_MIN {
            changed += 1;
        }
    }
    let mad = (sum as f32) / (n as f32 * 255.0);
    let cpc = (changed as f32) / n as f32;
    mad.max(cpc * CPC_WEIGHT)
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

    #[test]
    fn dark_theme_sparse_change_detected() {
        // Simulate two dark-theme screens where ~8 % of pixels change from
        // near-black to near-white (text / panel update).  MAD alone would give
        // ~0.031 (below the default 0.04 threshold); CPC should push the score
        // above it.
        let mut f1: RgbaImage = ImageBuffer::from_pixel(640, 360, Rgba([20, 20, 20, 255]));
        let f2_base: RgbaImage = f1.clone();
        let mut f2 = f2_base;
        let total = 640u32 * 360;
        let to_change = total / 12; // ~8.3 %
        for i in 0..to_change {
            let x = i % 640;
            let y = i / 640;
            f2.put_pixel(x, y, Rgba([200, 200, 200, 255]));
        }
        // Also paint the same pixels in f1 identically dark so there's no
        // pre-existing difference, making this a pure content-swap test.
        for i in 0..to_change {
            let x = i % 640;
            let y = i / 640;
            f1.put_pixel(x, y, Rgba([20, 20, 20, 255]));
        }
        let score = diff_score(&fingerprint(&f1), &fingerprint(&f2));
        assert!(
            score >= 0.04,
            "dark-theme sparse change not detected: score={score:.4}"
        );
    }
}
