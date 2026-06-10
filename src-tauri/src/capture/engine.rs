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
use crate::ocr::OcrProvider;
use anyhow::{Context, Result};
use image::{GrayImage, RgbaImage};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tracing::Instrument;
use tracing::{debug, info_span};

pub async fn run_capture_loop(state: SharedState, app: AppHandle) -> Result<()> {
    tracing::info!("capture loop started");

    let (poll_fps, change_threshold, stability_frames, ai_on, ocr_on, keep_all, lang, monitor_idx) = {
        let guard = state.lock().await;
        let s = &guard.config.capture;
        (
            s.poll_fps,
            s.change_threshold,
            s.stability_frames,
            s.ai_describe,
            s.redact_pii,
            s.keep_all_frames,
            s.language.clone(),
            s.monitor_index as usize,
        )
    };

    // Create the OCR provider once — construction may be expensive (model loading).
    // Arc because spawn_blocking needs 'static ownership but we reuse across frames.
    let ocr_provider: Arc<dyn OcrProvider> =
        Arc::from(crate::ocr::create_provider().unwrap_or_else(|e| {
            tracing::warn!("OCR provider creation failed: {e}; redaction disabled");
            Box::new(crate::ocr::noop_provider::NoopOcrProvider)
        }));
    tracing::info!("OCR provider: {}", ocr_provider.name());

    let tick = Duration::from_millis((1000 / poll_fps.max(1)) as u64);

    let mut reference: Option<GrayImage> = None;
    let mut candidate: Option<(GrayImage, RgbaImage)> = None;
    let mut stable_streak: u32 = 0;
    let mut next_index: usize = 0;

    // Clone the atomic stop flag so we can check it without locking the mutex.
    let stop_flag = {
        let guard = state.lock().await;
        guard.capture_stop_flag.clone()
    };

    loop {
        let tick_start = std::time::Instant::now();

        // Stop check
        {
            if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::info!("capture loop stopping");
            return Ok(());
            }
        }

        // ── Grab the current frame ────────────────────────────────────────

        let frame = match grab_frame(monitor_idx)
            .instrument(info_span!("frame_grab", monitor = monitor_idx))
            .await
        {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("frame grab failed: {e}");
                let elapsed = tick_start.elapsed();
                let sleep_dur = adaptive_sleep(elapsed, tick);
                debug!(
                    elapsed_ms = elapsed.as_millis() as u64,
                    sleep_ms = sleep_dur.as_millis() as u64,
                    "tick timing"
                );
                tokio::time::sleep(sleep_dur).await;
                continue;
            }
        };
        // ── Fingerprint for change detection ──────────────────────────────
        let fp = {
            let _span = info_span!("fingerprint").entered();
            diff::fingerprint(&frame)
        };

        // ── Establish reference on first frame ────────────────────────────
        if reference.is_none() {
            reference = Some(fp);
            let elapsed = tick_start.elapsed();
            let sleep_dur = adaptive_sleep(elapsed, tick);
            debug!(
                elapsed_ms = elapsed.as_millis() as u64,
                sleep_ms = sleep_dur.as_millis() as u64,
                "tick timing"
            );
            tokio::time::sleep(sleep_dur).await;
            continue;
        }

        if candidate.is_some() {
            // ── Candidate exists: check stability ─────────────────────────
            // Scoped borrow so we can take ownership of `candidate` below.
            let score = {
                let _span = info_span!("diff").entered();
                let (cand_fp, _) = candidate.as_ref().unwrap();
                diff::diff_score(cand_fp, &fp)
            };

            if score < change_threshold * 0.5 {
                // Still on the same screen — accumulate confirmation frames.
                stable_streak += 1;
                if stable_streak >= stability_frames {
                    let (_, cand_frame) = candidate.take().unwrap();
                    promote_step(
                        &state,
                        &app,
                        cand_frame.clone(),
                        next_index,
                        ai_on,
                        ocr_on,
                        &lang,
                        &ocr_provider,
                    )
                    .await?;
                    next_index += 1;
                    reference = Some(diff::fingerprint(&cand_frame));
                    stable_streak = 0;
                }
            } else {
                // Screen changed AGAIN before the candidate stabilised.
                // Promote the in-flight candidate rather than silently dropping
                // it — this is what prevents fast navigation losing screens.
                let (old_fp, old_frame) = candidate.take().unwrap();
                promote_step(
                    &state,
                    &app,
                    old_frame.clone(),
                    next_index,
                    ai_on,
                    ocr_on,
                    &lang,
                    &ocr_provider,
                )
                .await?;
                next_index += 1;
                reference = Some(old_fp);

                // Is the brand-new frame already different from the just-set reference?
                let new_score = {
                    let _span = info_span!("diff").entered();
                    diff::diff_score(reference.as_ref().unwrap(), &fp)
                };
                if new_score >= change_threshold {
                    candidate = Some((fp, frame));
                    stable_streak = 0;
                }
            }
        } else {
            // ── No candidate: compare new frame against reference ─────────
            let score = {
                let _span = info_span!("diff").entered();
                diff::diff_score(reference.as_ref().unwrap(), &fp)
            };
            if score >= change_threshold {
                candidate = Some((fp, frame));
                stable_streak = 0;
            } else if keep_all && score > 0.001 {
                // Forensic mode: persist every sampled frame that differs even
                // slightly from the reference, even though it's below the
                // promotion threshold.  Costs disk space; enables full replay.
                let frame_clone = frame.clone();
                let score_copy = score;
                let state_clone = state.clone();
                tokio::spawn(async move {
                    let mut png = Vec::new();
                    if frame_clone
                        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                        .is_ok()
                    {
                        let hash = hex::encode(sha2::Sha256::digest(&png));
                        let guard = state_clone.lock().await;
                        if let Some(active) = &guard.active {
                            let path = active.frames_dir.join(format!("{hash}.png"));
                            if !path.exists() {
                                let _ = std::fs::write(&path, &png);
                            }
                            let _ = active.log.append(EventKind::FrameSampled {
                                frame_hash: Some(hash),
                                diff_score: score_copy,
                            });
                        }
                    }
                });
            }
        }

        // ── Adaptive throttle ─────────────────────────────────────────────
        let elapsed = tick_start.elapsed();
        let sleep_dur = adaptive_sleep(elapsed, tick);
        debug!(
            elapsed_ms = elapsed.as_millis() as u64,
            sleep_ms = sleep_dur.as_millis() as u64,
            "tick timing"
        );
        tokio::time::sleep(sleep_dur).await;
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
    ocr_on: bool,
    lang: &str,
    ocr_provider: &Arc<dyn OcrProvider>,
) -> Result<()> {
    // ── 1. OCR + PII redaction ────────────────────────────────────────────
    // Must happen BEFORE encoding/hashing so the saved PNG is the redacted
    // version. The plain word texts are evaluated in-process and never written
    // to disk; only text_hash values reach the event log.
    let (win_title, win_app) = active_window_info();

    let (frame, ocr_regions, redaction_hits) = if ocr_on {
        let rule_engine = {
            let guard = state.lock().await;
            guard.rule_engine.clone()
        };
        let provider = ocr_provider.clone();

        tokio::task::spawn_blocking(move || {
            let _span = info_span!("ocr_and_redact").entered();

            // Use the provider trait instead of calling engine.rs directly.
            let ocr_words = provider.recognize(&frame).unwrap_or_default();

            // Convert OcrWords to the (String, OcrRegion) format the
            // redaction pipeline expects.
            let text_regions: Vec<(String, crate::events::event::OcrRegion)> = ocr_words
                .into_iter()
                .map(|w| {
                    let text_hash = hex::encode(sha2::Sha256::digest(w.text.as_bytes()));
                    (
                        w.text,
                        crate::events::event::OcrRegion {
                            x: w.x,
                            y: w.y,
                            w: w.w,
                            h: w.h,
                            text_hash,
                        },
                    )
                })
                .collect();

            let mut redacted = frame;
            let hits = if let Some(engine) = &rule_engine {
                crate::privacy::redact::apply_redactions(&mut redacted, &text_regions, engine)
            } else {
                vec![]
            };

            let regions = text_regions.into_iter().map(|(_, r)| r).collect::<Vec<_>>();
            (redacted, regions, hits)
        })
        .await
        .map_err(|e| anyhow::anyhow!("OCR/redaction task panicked: {e:?}"))?
    } else {
        (frame, vec![], vec![])
    };

    // ── 2. Encode the (possibly redacted) frame ───────────────────────────
    let (w, h) = (frame.width(), frame.height());
    let (png_bytes, frame_hash) = {
        let _span = info_span!("encode_and_hash").entered();
        let mut png_bytes: Vec<u8> = Vec::new();
        {
            let mut cursor = std::io::Cursor::new(&mut png_bytes);
            frame
                .write_to(&mut cursor, image::ImageFormat::Png)
                .context("encoding PNG")?;
        }
        let hash = hex::encode(Sha256::digest(&png_bytes));
        (png_bytes, hash)
    };

    // ── 3. Persist to frames/{hash}.png (dedup-safe) ─────────────────────
    let (log, frames_dir) = {
        let guard = state.lock().await;
        let active = guard
            .active
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("session vanished"))?;
        (active.log.clone(), active.frames_dir.clone())
    };
    let frame_path = frames_dir.join(format!("{frame_hash}.png"));
    if !frame_path.exists() {
        let _span = info_span!("write_frame").entered();
        std::fs::write(&frame_path, &png_bytes)
            .with_context(|| format!("writing {}", frame_path.display()))?;
    }

    // ── 4. Emit StepPromoted ──────────────────────────────────────────────
    let rec = log.append(EventKind::StepPromoted {
        step_index: index,
        frame_hash: frame_hash.clone(),
        width: w,
        height: h,
        window_title: win_title.clone(),
        window_class: None,
        app_name: win_app.clone(),
    })?;

    // ── 5. Emit OcrResult (regions with text hashes only) ─────────────────
    if !ocr_regions.is_empty() {
        let _ = log.append(EventKind::OcrResult {
            frame_hash: frame_hash.clone(),
            text_length: ocr_regions.len(),
            regions: ocr_regions,
        });
    }

    // ── 6. Emit RedactionApplied events ──────────────────────────────────
    // Group hits by rule name to produce one event per rule per step.
    let mut by_rule: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (rule_name, _) in &redaction_hits {
        *by_rule.entry(rule_name.clone()).or_default() += 1;
    }
    for (rule_name, count) in by_rule {
        let _ = log.append(EventKind::RedactionApplied {
            frame_hash: Some(frame_hash.clone()),
            rule_name,
            match_count: count,
            action: "applied".into(),
        });
    }

    // ── 7. AI description ─────────────────────────────────────────────────
    if ai_on {
        if let Ok(desc) = describer::describe(&frame, win_title.as_deref(), win_app.as_deref()) {
            let _ = log.append(EventKind::AiDescription {
                step_index: index,
                text: desc,
                model: "heuristic-v2".into(),
                language: lang.to_string(),
            });
        }
    }

    // ── 8. Notify UI ──────────────────────────────────────────────────────
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
    tracing::info!("captured step #{index} ({frame_hash})");
    Ok(())
}

/// Adaptive throttle: if the capture tick took longer than the budget,
/// increase the sleep to avoid starving the CPU. If it's well within
/// budget, restore the configured rate.
///
/// Returns the duration to sleep before the next tick.
fn adaptive_sleep(elapsed: Duration, base_tick: Duration) -> Duration {
    let utilization = elapsed.as_secs_f64() / base_tick.as_secs_f64();

    if utilization > 0.6 {
        // CPU pressure — slow down. Scale linearly, cap at 4× base.
        let scale = (utilization * 1.5).min(4.0);
        Duration::from_secs_f64(base_tick.as_secs_f64() * scale)
    } else if utilization < 0.4 {
        // Headroom — restore base rate.
        base_tick
    } else {
        // Comfortable zone — hold current.
        base_tick
    }
}

/// Returns (window_title, app_name) for the currently focused window.
///
/// Windows: uses GetForegroundWindow + QueryFullProcessImageNameW — the only
/// accurate way to get the true foreground window, regardless of z-order.
///
/// Other platforms: falls back to the xcap heuristic (first non-minimised
/// window with a non-empty title) until OS-specific APIs are wired in P2.
#[cfg(target_os = "windows")]

fn active_window_info() -> (Option<String>, Option<String>) {
    use windows_sys::Win32::Foundation::FALSE;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return (None, None);
        }

        // Window title ─────────────────────────────────────────────────────
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 512);
        if title_len == 0 {
            return (None, None);
        }
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        // Executable name ───────────────────────────────────────────────────
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);

        let app_name = if pid != 0 {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
            if !handle.is_null() {
                let mut exe_buf = [0u16; 512];
                let mut size: u32 = 512;
                let ok = QueryFullProcessImageNameW(
                    handle,
                    PROCESS_NAME_WIN32,
                    exe_buf.as_mut_ptr(),
                    &mut size,
                );
                windows_sys::Win32::Foundation::CloseHandle(handle);
                if ok != 0 && size > 0 {
                    let path = String::from_utf16_lossy(&exe_buf[..size as usize]);
                    std::path::Path::new(&path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        (Some(title), app_name)
    }
}

#[cfg(not(target_os = "windows"))]
fn active_window_info() -> (Option<String>, Option<String>) {
    // xcap heuristic: first non-minimised, non-empty-title window.
    // Replace with platform-specific focus APIs in a follow-up P2 task.
    let result = std::panic::catch_unwind(|| {
        let wins = xcap::Window::all().ok()?;
        wins.into_iter()
            .filter(|w| !w.is_minimized() && !w.title().is_empty())
            .map(|w| (w.title().to_string(), w.app_name().to_string()))
            .next()
    });
    match result {
        Ok(Some((title, app))) => (Some(title), Some(app)),
        _ => (None, None),
    }
}

// ── Test-only helpers ─────────────────────────────────────────────────────────

/// Pure synchronous simulation of the capture state machine.
///
/// Takes pre-computed fingerprints (one per poll tick) and returns the indices
/// of the frames that would have been promoted as steps. Used by integration
/// tests to verify the diff + stability + promote-on-replace logic without
/// needing a real screen or the Tauri runtime.
///
/// An unconsumed candidate at the end of the sequence is flushed as promoted
/// (equivalent to the user stopping the recording while on that screen).
#[cfg(test)]
pub fn simulate_capture_decisions(
    fingerprints: &[image::GrayImage],
    change_threshold: f32,
    stability_frames: u32,
) -> Vec<usize> {
    use crate::capture::diff;

    let mut reference: Option<image::GrayImage> = None;
    let mut candidate: Option<(image::GrayImage, usize)> = None;
    let mut stable_streak: u32 = 0;
    let mut promoted: Vec<usize> = Vec::new();

    for (i, fp) in fingerprints.iter().enumerate() {
        if reference.is_none() {
            reference = Some(fp.clone());
            continue;
        }

        if candidate.is_some() {
            let score = {
                let (cand_fp, _) = candidate.as_ref().unwrap();
                diff::diff_score(cand_fp, fp)
            };
            if score < change_threshold * 0.5 {
                stable_streak += 1;
                if stable_streak >= stability_frames {
                    let (old_fp, old_idx) = candidate.take().unwrap();
                    promoted.push(old_idx);
                    reference = Some(old_fp);
                    stable_streak = 0;
                }
            } else {
                let (old_fp, old_idx) = candidate.take().unwrap();
                promoted.push(old_idx);
                reference = Some(old_fp);
                let new_score = diff::diff_score(reference.as_ref().unwrap(), fp);
                if new_score >= change_threshold {
                    candidate = Some((fp.clone(), i));
                    stable_streak = 0;
                }
            }
        } else {
            let score = diff::diff_score(reference.as_ref().unwrap(), fp);
            if score >= change_threshold {
                candidate = Some((fp.clone(), i));
                stable_streak = 0;
            }
        }
    }

    // Flush remaining candidate (session ended while dwelling on a screen).
    if let Some((_, idx)) = candidate.take() {
        promoted.push(idx);
    }

    promoted
}
