// AI step description generator.
//
// Phase 1 (this scaffold): a heuristic placeholder that produces sensible
// fallback text. The interface is stable so a real model can drop in.
//
// Phase 2: bundle a small on-device vision-language model. Recommended options:
//   - `candle` (Hugging Face's Rust ML framework) running a quantized
//     image-captioning model like BLIP-2 or a SmolVLM variant.
//   - `llama.cpp` Rust bindings (`llama-cpp-2` crate) running a multimodal
//     gguf-quantized model.
//   - `ort` (ONNX Runtime) running an exported captioning model.
//
// All three keep inference 100% local — critical for our privacy promise.

use anyhow::Result;
use image::RgbaImage;

/// Generate a one-line description for a captured frame.
/// Returns an Err only on hard failure; soft failures should return a
/// best-effort string so the user always sees *something*.
pub fn describe(frame: &RgbaImage) -> Result<String> {
    // Phase 1 heuristic: describe by image characteristics.
    // Mean brightness + dominant color hint at the type of UI.
    let (w, h) = frame.dimensions();
    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;
    let n = (w * h) as u64;
    for p in frame.pixels() {
        sum_r += p[0] as u64;
        sum_g += p[1] as u64;
        sum_b += p[2] as u64;
    }
    let avg_r = (sum_r / n) as u8;
    let avg_g = (sum_g / n) as u8;
    let avg_b = (sum_b / n) as u8;
    let brightness = (avg_r as u32 + avg_g as u32 + avg_b as u32) / 3;

    let hint = if brightness > 200 {
        "Light-themed dialog or wizard step shown."
    } else if brightness < 60 {
        "Dark-themed view shown."
    } else {
        "Application screen changed."
    };

    Ok(hint.to_string())
}

/// Future entry point: model-backed describer.
#[allow(dead_code)]
pub fn describe_with_model(_frame: &RgbaImage, _lang: &str) -> Result<String> {
    // TODO: load model once at startup, cache the session, run inference.
    unimplemented!("model-backed describer not yet wired")
}
