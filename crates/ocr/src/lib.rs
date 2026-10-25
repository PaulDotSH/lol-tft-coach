//! Read Mayhem augment offers off the League window: GDI capture of the
//! (borderless) game window → Windows.Media.Ocr → fuzzy match to augment names.
//! Blocking; call from a blocking thread. Never sends input or takes focus.

use model::AugmentId;
use std::collections::HashMap;

mod matching;
pub mod scoreboard;
#[cfg(windows)]
mod win;

pub use matching::match_augments;

/// BGRA8 pixels, top-down.
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// Capture the region of the League game window where augment cards appear.
/// None when the game window is missing or minimised.
pub fn capture_league() -> Option<Frame> {
    #[cfg(windows)]
    return win::capture_league();
    #[cfg(not(windows))]
    None
}

/// Size of the League window's client area.
pub fn league_client() -> Option<(u32, u32)> {
    #[cfg(windows)]
    return win::client_size();
    #[cfg(not(windows))]
    None
}

/// Capture a rectangle of the League window's client area.
pub fn capture_client(r: scoreboard::Rect) -> Option<Frame> {
    #[cfg(windows)]
    return win::capture_client(r);
    #[cfg(not(windows))]
    {
        let _ = r;
        None
    }
}

/// One OCR line and its box, in frame pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBox {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// OCR lines with their boxes.
pub fn recognize_boxes(frame: &Frame) -> anyhow::Result<Vec<TextBox>> {
    #[cfg(windows)]
    return win::recognize_boxes(frame);
    #[cfg(not(windows))]
    {
        let _ = frame;
        anyhow::bail!("OCR is only available on Windows")
    }
}

/// OCR text lines from a frame.
pub fn recognize(frame: &Frame) -> anyhow::Result<Vec<String>> {
    #[cfg(windows)]
    return win::recognize(frame);
    #[cfg(not(windows))]
    {
        let _ = frame;
        anyhow::bail!("OCR is only available on Windows")
    }
}

/// capture → recognize → match. Empty when nothing is offered.
pub fn scan(names: &HashMap<AugmentId, String>) -> anyhow::Result<Vec<AugmentId>> {
    match capture_league() {
        Some(f) => Ok(match_augments(&recognize(&f)?, names)),
        None => Ok(vec![]),
    }
}
