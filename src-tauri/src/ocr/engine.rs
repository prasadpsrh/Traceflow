use crate::events::event::OcrRegion;
use anyhow::Result;
use image::RgbaImage;

/// Run OCR and return `(plain_text, region)` pairs.
/// The plain text is needed by the rules engine for PII detection; it is
/// never written to disk — only the SHA-256 hash in `OcrRegion` is persisted.
/// Called from `promote_step` via `tokio::task::spawn_blocking`.
pub fn run_ocr_with_text(frame: &RgbaImage, language: &str) -> Result<Vec<(String, OcrRegion)>> {
    #[cfg(target_os = "windows")]
    return platform::run(frame, language);

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (frame, language);
        Ok(vec![])
    }
}

// ── Windows implementation ────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use sha2::{Digest, Sha256};
    use windows::{
        core::HSTRING,
        Globalization::Language,
        Graphics::Imaging::{BitmapDecoder, BitmapPixelFormat, SoftwareBitmap},
        Media::Ocr::OcrEngine,
        Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
    };

    pub fn run(frame: &RgbaImage, language: &str) -> Result<Vec<(String, OcrRegion)>> {
        // Initialise MTA for this thread so WinRT factories work.
        unsafe {
            windows_sys::Win32::System::Com::CoInitializeEx(
                std::ptr::null(),
                windows_sys::Win32::System::Com::COINIT_MULTITHREADED as u32,
            );
        }

        // ── Encode frame → PNG → Windows in-memory stream ────────────────
        let mut png = Vec::new();
        frame
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| anyhow::anyhow!("PNG encode: {e}"))?;

        let stream = InMemoryRandomAccessStream::new()?;
        {
            let output = stream.GetOutputStreamAt(0)?;
            let writer = DataWriter::CreateDataWriter(&output)?;
            writer.WriteBytes(&png)?;
            writer.StoreAsync()?.get()?;
            writer.FlushAsync()?.get()?;
            writer.DetachStream()?;
        }
        stream.Seek(0)?;

        // ── Decode to SoftwareBitmap ──────────────────────────────────────
        let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
        let bitmap = decoder.GetSoftwareBitmapAsync()?.get()?;
        let bitmap = if bitmap.BitmapPixelFormat()? != BitmapPixelFormat::Bgra8 {
            SoftwareBitmap::Convert(&bitmap, BitmapPixelFormat::Bgra8)?
        } else {
            bitmap
        };

        // ── Acquire OcrEngine ────────────────────────────────────────────
        let lang_tag = HSTRING::from(language);
        let engine = Language::CreateLanguage(&lang_tag)
            .ok()
            .and_then(|l| {
                if OcrEngine::IsLanguageSupported(&l).unwrap_or(false) {
                    OcrEngine::TryCreateFromLanguage(&l).ok()
                } else {
                    None
                }
            })
            .or_else(|| OcrEngine::TryCreateFromUserProfileLanguages().ok());

        let engine = match engine {
            Some(e) => e,
            None => {
                log::warn!("OCR: no engine available for language '{language}'");
                return Ok(vec![]);
            }
        };

        // ── Run recognition ───────────────────────────────────────────────
        let result = engine.RecognizeAsync(&bitmap)?.get()?;

        // ── Extract (text, region) pairs ──────────────────────────────────
        let mut out = Vec::new();
        let lines = result.Lines()?;
        for i in 0..lines.Size()? {
            let line = lines.GetAt(i)?;
            let words = line.Words()?;
            for j in 0..words.Size()? {
                let word = words.GetAt(j)?;
                let text = word.Text()?.to_string();
                if text.trim().is_empty() {
                    continue;
                }
                let rect = word.BoundingRect()?;
                let text_hash = hex::encode(Sha256::digest(text.as_bytes()));
                out.push((
                    text,
                    OcrRegion {
                        x: rect.X as u32,
                        y: rect.Y as u32,
                        w: rect.Width as u32,
                        h: rect.Height as u32,
                        text_hash,
                    },
                ));
            }
        }

        log::debug!("OCR: {} word regions", out.len());
        Ok(out)
    }
}
