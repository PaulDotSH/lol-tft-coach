//! Windows.Media.Ocr on a captured frame.

use crate::Frame;
use anyhow::{Context, Result, anyhow};
use std::sync::{Mutex, OnceLock};
use windows::Globalization::Language;
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapBufferAccessMode, BitmapPixelFormat, SoftwareBitmap,
};
use windows::Media::Ocr::OcrEngine;
use windows::Win32::System::WinRT::IMemoryBufferByteAccess;
use windows::core::{HSTRING, Interface};

/// One engine per process; it rejects concurrent RecognizeAsync calls, hence the lock.
fn engine() -> Result<&'static Mutex<OcrEngine>> {
    static ENGINE: OnceLock<Result<Mutex<OcrEngine>, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            Language::CreateLanguage(&HSTRING::from("en-US"))
                .and_then(|l| OcrEngine::TryCreateFromLanguage(&l))
                .or_else(|_| OcrEngine::TryCreateFromUserProfileLanguages())
                .map(Mutex::new)
                .map_err(|e| format!("no OCR engine: {e}"))
        })
        .as_ref()
        .map_err(|e| anyhow!("{e}"))
}

/// Luma (max channel, keeps coloured text bright) into a Gray8 bitmap,
/// nearest-downsampled by `step`.
fn to_bitmap(frame: &Frame, step: u32) -> Result<SoftwareBitmap> {
    let (w, h) = (frame.width.div_ceil(step), frame.height.div_ceil(step));
    let bmp = SoftwareBitmap::CreateWithAlpha(
        BitmapPixelFormat::Gray8,
        w as i32,
        h as i32,
        BitmapAlphaMode::Ignore,
    )?;
    {
        let buf = bmp.LockBuffer(BitmapBufferAccessMode::Write)?;
        let plane = buf.GetPlaneDescription(0)?;
        let reference = buf.CreateReference()?;
        let access: IMemoryBufferByteAccess = reference.cast()?;
        let (mut ptr, mut cap) = (std::ptr::null_mut(), 0u32);
        unsafe { access.GetBuffer(&mut ptr, &mut cap)? };
        let dst = unsafe { std::slice::from_raw_parts_mut(ptr, cap as usize) };
        let (start, stride) = (plane.StartIndex as usize, plane.Stride as usize);
        let src_stride = frame.width as usize * 4;
        for y in 0..h as usize {
            let src = &frame.bgra[y * step as usize * src_stride..][..src_stride];
            let row = &mut dst[start + y * stride..][..w as usize];
            for (x, out) in row.iter_mut().enumerate() {
                let p = &src[x * step as usize * 4..][..3];
                *out = p[0].max(p[1]).max(p[2]);
            }
        }
    }
    Ok(bmp)
}

pub fn recognize(frame: &Frame) -> Result<Vec<String>> {
    anyhow::ensure!(
        frame.width > 0
            && frame.height > 0
            && frame.bgra.len() >= (frame.width * frame.height * 4) as usize,
        "bad frame"
    );
    let engine = engine()?;
    let max = OcrEngine::MaxImageDimension()?.max(1);
    let step = frame.width.max(frame.height).div_ceil(max);
    let bmp = to_bitmap(frame, step)?;
    let result = {
        let engine = engine.lock().unwrap_or_else(|e| e.into_inner());
        engine.RecognizeAsync(&bmp)?.join().context("OCR failed")?
    };
    result
        .Lines()?
        .into_iter()
        .map(|l| Ok(l.Text()?.to_string_lossy()))
        .collect()
}
