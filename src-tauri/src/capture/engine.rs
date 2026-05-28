// Capture engine, v2.
//
// Same change-then-stabilize state machine as before, but now every promotion
// emits events to the session's append-only log instead of mutating an
// in-memory Session struct. The log is the single source of truth.
//
// Frame files are content-addressed: each PNG is saved as `{sha256}.png`
// inside `frames/`. This gives free deduplication (identical frames produce
// identical files) and makes screenshots tamper-evident — any pixel change
// produces a different hash, which produces a different event, which breaks
// the chain.

use crate::ai::describer;
use crate::capture::{diff, monitor};
use crate::commands::SharedState;
use crate::events::EventKind;
use anyhow::{Context, Result};
use image::{GrayImage, RgbaImage};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub async fn run_capture_loop(state: SharedState, app: AppHandle) -> Result<()> {
    log::info!("capture loop started");

    let (poll_fps, change_threshold, stability_frames, ai_on, lang) = {
        let guard = state.lock().await;
        let s = &guard.config.capture;
        (
            s.poll_fps,
            s.change_threshold,
            s.stability_frames,
            s.ai_describe,
            s.language.clone(),
        )
    };

    let tick = Duration::from_millis((1000 / poll_fps.max(1)) as u64);

    let mut reference: Option<GrayImage> = None;
    let mut candidate: Option<(GrayImage, RgbaImage)> = None;
    let mut stable_streak: u32 = 0;
    let mut next_index: usize = 0;

    loop {
        // Stop check
        {
            let guard = state.lock().await;
            if guard.capture_stop_flag || guard.active.is_none() {
                log::info!("capture loop stopping");
                return Ok(());
            }
        }

        let frame = match grab_frame(0).await {
            Ok(f) => f,
            Err(e) => {
                log::warn!("frame grab failed: {e}");
                tokio::time::sleep(tick).await;
                continue;
            }
        };
        let fp = diff::fingerprint(&frame);

        match (&reference, &candidate) {
            (None, _) => {
                reference = Some(fp);
            }
            (Some(ref_fp), None) => {
                let score = diff::diff_score(ref_fp, &fp);
                if score >= change_threshold {
                    candidate = Some((fp, frame));
                    stable_streak = 1;
                }
            }
            (Some(_), Some((cand_fp, _))) => {
                let score = diff::diff_score(cand_fp, &fp);
                if score < change_threshold * 0.5 {
                    stable_streak += 1;
                    if stable_streak >= stability_frames {
                        let (_, cand_frame) = candidate.take().unwrap();
                        promote_step(&state, &app, cand_frame.clone(), next_index, ai_on, &lang)
                            .await?;
                        next_index += 1;
                        reference = Some(diff::fingerprint(&cand_frame));
                        stable_streak = 0;
                    }
                } else {
                    candidate = Some((fp, frame));
                    stable_streak = 1;
                }
            }
        }

        tokio::time::sleep(tick).await;
    }
}

async fn grab_frame(monitor_index: usize) -> Result<RgbaImage> {
    let m = monitor::get_monitor(monitor_index)?;
    let img = m.capture_image().context("capture_image failed")?;
    Ok(img)
}

async fn promote_step(
    state: &SharedState,
    app: &AppHandle,
    frame: RgbaImage,
    index: usize,
    ai_on: bool,
    lang: &str,
) -> Result<()> {
    // 1. Encode to PNG in memory
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        frame
            .write_to(&mut cursor, image::ImageFormat::Png)
            .context("encoding PNG")?;
    }

    // 2. Content-address: sha256 of bytes → filename
    let frame_hash = hex::encode(Sha256::digest(&png_bytes));

    // 3. Save to frames/{hash}.png, dedup-safe
    let (log, frames_dir, window_title) = {
        let guard = state.lock().await;
        let active = guard
            .active
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("session vanished"))?;
        (
            active.log.clone(),
            active.frames_dir.clone(),
            active_window_title(),
        )
    };
    let frame_path = frames_dir.join(format!("{frame_hash}.png"));
    if !frame_path.exists() {
        std::fs::write(&frame_path, &png_bytes)
            .with_context(|| format!("writing {}", frame_path.display()))?;
    }

    // 4. Emit StepPromoted event
    let rec = log.append(EventKind::StepPromoted {
        step_index: index,
        frame_hash: frame_hash.clone(),
        width: frame.width(),
        height: frame.height(),
        window_title: window_title.clone(),
        window_class: None,
        app_name: None,
    })?;

    // 5. Optionally run AI description and emit a follow-up event
    if ai_on {
        if let Ok(desc) = describer::describe(&frame) {
            let _ = log.append(EventKind::AiDescription {
                step_index: index,
                text: desc.clone(),
                model: "heuristic".into(),
                language: lang.to_string(),
            });
        }
    }

    // 6. Bump UI step counter and emit a "step-captured" event with a view
    //    payload the React side can render directly.
    {
        let mut guard = state.lock().await;
        guard.step_count += 1;
    }
    let payload = serde_json::json!({
        "event_record": rec,
        "frame_path": frame_path,
        "step_index": index,
    });
    let _ = app.emit("step-captured", payload);
    log::info!("captured step #{index} ({frame_hash})");
    Ok(())
}

fn active_window_title() -> Option<String> {
    std::panic::catch_unwind(|| {
        let wins = xcap::Window::all().ok()?;
        wins.into_iter()
            .find(|w| w.is_focused().unwrap_or(false))
            .and_then(|w| w.title().ok().map(|t| t.to_string()))
    })
    .ok()
    .flatten()
}
